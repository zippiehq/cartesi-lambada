use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use hyper::Request;
use sequencer::L1BlockInfo;
use serde_json::Value;
use sqlite::State;
use std::io::Cursor;
use std::time::{Duration, SystemTime};

// How we communicate between host (this) and guest (the cartesi machine) is through read-writing the second flash drive
// which is placed in memory at MACHINE_IO_ADDRESSS. Special care is needed on machine side to flush/ignore OS caches until PMEM/DAX comes around.
//
// The guest writes certain operations along with serialized parameters into memory, does an automatic yield, and the host handles the operation
// and resumes its execution after clearing yield flag.
//
// The model we are assuming here: 
// - a base machine image, that'll boot up
// - signal LOAD_APP (give me an app CID), 
// - and the app will signal LOAD_TX (give me a payload and state CID + app CID (for double checking it's running the right image))
// - and result in FINISH (accept/reject) with a new state CID or EXCEPTION
// - halting or out of cycles is a failed tx

pub const MACHINE_IO_ADDRESSS: u64 = 0x90000000000000;

// IPFS 'de-hashing' - get a IPFS block based on a CID
const READ_BLOCK: u64 = 0x00001;

// an exception happened in the machine
const EXCEPTION: u64 = 0x00002;

// machine/app is expecting to be given a transaction payload, also current state CID and current app CID
const LOAD_TX: u64 = 0x00003;

// app is done processing the transaction succesfully or unsuccesfully but not with an exception
const FINISH: u64 = 0x00004;

// Write back to IPFS on host (extract new IPFS blocks created inside machine)
const WRITE_BLOCK: u64 = 0x000005;

// base image has booted up and expects to get the current app CID to initialize it into LOAD_TX state. Null-op in arbitration.
const LOAD_APP: u64 = 0x00006;

// make a hint that we're expecting certain IPFS hashes to be available (for example all hashes from ethereum block X), null-op in arbitration
const HINT: u64 = 0x00007;

// execute is the entry point for the computation to be done, we have a particular state CID with it's associated /app directory
// and a transaction payload + block_info and we want to do this computation and get the new state CID back or an error
pub async fn execute(
    machine_url: String,
    cartesi_machine_path: &str,
    ipfs_url: &str,
    payload: Vec<u8>,
    state_cid: Cid,
    block_info: &L1BlockInfo,
) -> Result<Cid, std::io::Error> {
    tracing::info!(
        "state cid {:?}",
        state_cid.to_string()
    );

    // Resolve what the app CID is in this current state
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}/api/v0/dag/resolve?arg={}/app",
            ipfs_url,
            state_cid.to_string()
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let mut app_cid = String::new();
    let client = hyper::Client::new();

    match client.request(req).await {
        Ok(res) => {
            let app_cid_value = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(res).await.expect("no cid").to_vec(),
            )
            .unwrap();

            app_cid = app_cid_value
                .get("Cid")
                .unwrap()
                .get("/")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
        }
        Err(e) => {
            panic!("{}", e)
        }
    }
    tracing::info!(
        "app cid {:?}",
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    );

    let client = IpfsClient::from_str(ipfs_url).unwrap();
    tracing::info!("execute");

    // connect to a Cartesi Machine - we expect this to be an forked, empty machine and we control when it's shut down
    let mut machine = JsonRpcCartesiMachineClient::new(machine_url).await.unwrap();

    // The current state of the machine (0 = base image loaded, base image initialized (LOAD_APP), 1 = app initialized (LOAD_TX), 2 = processing a tx (should end in FINISH/EXCEPTION))
    let mut machine_loaded_state = 0;

    // TODO: we will have multiple base images in future
    if std::path::Path::new(&format!(
        "/data/snapshot/base_{}",
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    ))
    .is_dir()
    {
        // There is a snapshot of this base machine with this app initialized and waiting for a transaction (LOAD_TX)
        tracing::info!(
            "loading machine from /data/snapshot/base_{}",
            Cid::try_from(app_cid.clone()).unwrap().to_string()
        );
        let before_load_machine = SystemTime::now();
        machine
            .load_machine(
                &format!(
                    "/data/snapshot/base_{}",
                    Cid::try_from(app_cid.clone()).unwrap().to_string()
                ),
                &MachineRuntimeConfig {
                    skip_root_hash_check: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let after_load_machine = SystemTime::now();
        tracing::info!(
            "It took {} milliseconds to load snapshot",
            after_load_machine
                .duration_since(before_load_machine)
                .unwrap()
                .as_millis()
        );
        
        machine_loaded_state = 2;
        tracing::info!("read iflag y {:?}", machine.read_iflags_y().await.unwrap());
    } else if std::path::Path::new(&format!("/data/snapshot/base",)).exists() {
        // There is a snapshot of this base machine initialized and waiting to be populated with an app that'll initialize (LOAD_APP)
        let before_load_machine = SystemTime::now();
        machine
            .load_machine(
                "/data/snapshot/base",
                &MachineRuntimeConfig {
                    skip_root_hash_check: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let after_load_machine = SystemTime::now();
        tracing::info!(
            "It took {} milliseconds to load snapshot",
            after_load_machine
                .duration_since(before_load_machine)
                .unwrap()
                .as_millis()
        );

        machine_loaded_state = 1;
    } else {
        // We need to load the base machine and run it, snapshotting LOAD_APP and LOAD_TX stages
        machine
            .load_machine(
                cartesi_machine_path,
                &MachineRuntimeConfig {
                    skip_root_hash_check: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    loop {
        let mut interpreter_break_reason = Value::Null;
        // Are we yielded? If not, continue machine execution
        // TODO: limit cycles if in compute mode
        if !machine.read_iflags_y().await.unwrap() {
            interpreter_break_reason = machine.run(u64::MAX).await.unwrap();
        }

        // XXX remove, for debugging
        let hex_encoded = hex::encode(
            machine
                .read_memory(MACHINE_IO_ADDRESSS, 1024)
                .await
                .unwrap(),
        );

        // Command/opcode is 64-bit big endian value at MACHINE_IO_ADDRESSS
        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let opt = u64::from_be_bytes(read_opt_be_bytes.try_into().unwrap());

        // XXX remove, for debugging
        tracing::info!(
            "before handling iflag y {:?}",
            machine.read_iflags_y().await.unwrap()
        );

        match opt {
            // Handles the READ_BLOCK action: [IPFS "dehashing"]
            // 1. Reads a 64-bit value from the specified memory address (MACHINE_IO_ADDRESSS + 8) and converts it from big-endian to a u64, representing the length of a content identifier (cid).
            // 2. Reads the content identifier (cid) of the specified length from memory starting at MACHINE_IO_ADDRESSS + 16 and converts it into a Cid object.
            // 3. Fetches the block associated with the cid from IPFS, concatenating all chunks of data received into a single vector.
            // 4. Writes the retrieved block back into the machine's memory at MACHINE_IO_ADDRESSS + 16, encoding using the STANDARD base64 (due to jsonrpc).
            // 5. Writes the length of the block as a big-endian byte array into the machine's memory at MACHINE_IO_ADDRESSS, also using the STANDARD encoding.
            //
            // Unavailable IPFS CID should kill the machine, data requested by machine is assumed to be available.
            READ_BLOCK => {
                tracing::info!("READ_BLOCK");
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid = Cid::try_from(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                        .await
                        .unwrap(),
                )
                .unwrap();

                tracing::info!("read cid {:?}", cid.to_string());

                let block = client
                    .block_get(cid.to_string().as_str())
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();
                tracing::info!("block len {:?}", block.len());

                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 16, STANDARD.encode(block.clone()))
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(block.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                tracing::info!("read_block info was written");
            }
            // Handles the EXCEPTION case coming from VM/guest:
            // 1. Destroys the current state of the machine, releasing any resources it was using.
            // 2. Shuts down the machine, ensuring all ongoing processes are terminated.
            // 3. Returns an error of type `std::io::Error` with the kind `std::io::ErrorKind::Other`, indicating a general error, and a message "exception" to signify that an exception occurred.
            EXCEPTION => {
                tracing::info!("HTIF_YIELD_REASON_TX_EXCEPTION");
                machine.destroy().await.unwrap();
                machine.shutdown().await.unwrap();
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "exception"));
            }
            // Handles the LOAD_TX action coming from VM/guest:
            // 1. Checks if the `machine_loaded_state` is either 0 or 1 [TODO: explain this better]:. If true, performs the following:
            //    a. Converts `app_cid` to a `Cid' so we can get it in text form.
            //    b. Stores a snapshot of the current state to a directory named after `app_cid` under `/data/snapshot/`, prefixed with base_.
            // 2. Converts `state_cid` to a `Cid` type and calculates its byte length as `cid_length`.
            // 3. Writes `cid_length` as big-endian bytes into the machine's memory at `MACHINE_IO_ADDRESSS`.
            // 4. Writes the bytes of `current_cid` to the machine's memory at `MACHINE_IO_ADDRESSS + 8`.
            // 5. Calculates the length of `payload` as `payload_length` and writes it as big-endian bytes into the machine's memory at `MACHINE_IO_ADDRESSS + 16 + cid_length`.
            // 6. Writes the `payload` bytes to the machine's memory at `MACHINE_IO_ADDRESSS + 24 + cid_length`.
            // 7. Converts the block number from `block_info` to big-endian bytes and writes it to the machine's memory at `MACHINE_IO_ADDRESSS + 24 + cid_length + payload_length`.
            // 8. Initializes a `block_timestamp` vector of 32 bytes and fills it with the big-endian representation of the timestamp from `block_info`.
            // 9. Writes the `block_timestamp` bytes to the machine's memory at `MACHINE_IO_ADDRESSS + 32 + cid_length + payload_length`.
            // 10. Extracts the `hash` from `block_info` and writes it to the machine's memory at `MACHINE_IO_ADDRESSS + 32 + block_timestamp.len() + cid_length + payload_length`.
            LOAD_TX => {
                tracing::info!("LOAD_TX");
                if machine_loaded_state == 0 || machine_loaded_state == 1 {
                    let app_cid: cid::CidGeneric<64> = Cid::try_from(app_cid.clone()).unwrap();
                    tracing::info!(
                        "snapshot stage load tx to dir: {} and read iflag : {}",
                        format!("/data/snapshot/base_{}", app_cid.clone().to_string()),
                        machine.read_iflags_y().await.unwrap()
                    );

                    machine
                        .store(&format!(
                            "/data/snapshot/base_{}",
                            app_cid.clone().to_string(),
                        ))
                        .await
                        .unwrap();
                    tracing::info!("done snapshotting");
                }
                let cid_length = state_cid.clone().to_bytes().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(state_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();

                let payload_length = payload.clone().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16 + cid_length,
                        STANDARD.encode(payload_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 24 + cid_length,
                        STANDARD.encode(payload.clone()),
                    )
                    .await
                    .unwrap();

                let block_number = block_info.number.to_be_bytes();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 24 + cid_length + payload_length,
                        STANDARD.encode(block_number.to_vec()),
                    )
                    .await
                    .unwrap();

                let mut block_timestamp = vec![0; 32];

                block_info.timestamp.to_big_endian(&mut block_timestamp);

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 32 + cid_length + payload_length,
                        STANDARD.encode(block_timestamp.to_vec()),
                    )
                    .await
                    .unwrap();

                let hash = block_info.hash.0;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS
                            + 32
                            + block_timestamp.len() as u64
                            + cid_length
                            + payload_length,
                        STANDARD.encode(hash.to_vec()),
                    )
                    .await
                    .unwrap();

                tracing::info!("load_tx info was written");
            }
            // FINISH: The guest has finished execution and reported back a status code, accept or rejection of the transaction.
            // It also reports back the current IPFS CID of the state which we return to the consumer of this function.
            FINISH => {
                tracing::info!("FINISH");

                let status = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let data = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 24, length)
                    .await
                    .unwrap();

                match status {
                    0 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        println!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        tracing::info!(
                            "FINISH received cid {}",
                            Cid::try_from(data.clone()).unwrap()
                        );

                        return Ok(Cid::try_from(data).unwrap());
                    }
                    1 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_REJECTED");
                        println!("HTIF_YIELD_REASON_RX_REJECTED");
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "transaction was rejected",
                        ));
                    }
                    _ => {
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "unknown status",
                        ));
                    }
                }
            }
            // WRITE_BLOCK:
            // 1. Reads an 8-byte value from the machine's memory at a specified address offset (MACHINE_IO_ADDRESSS + 8).
            // 2. Converts this 8-byte value from big-endian format to a u64 representing the length of the data to be written.
            // 3. Reads a block of memory from the machine, starting at a specified address offset (MACHINE_IO_ADDRESSS + 16) and of the previously determined length.
            // 4. Creates a Cursor wrapped around the cloned memory block. This Cursor is used to facilitate reading the data as a stream.
            // 5. Invokes the 'block_put' method of a IPFS client with the Cursor as an argument, which stores the block using the data stream.
            WRITE_BLOCK => {
                tracing::info!("WRITE_BLOCK");
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let memory = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                let data = Cursor::new(memory.clone());
                let put_response = client.block_put(data).await.unwrap();
            }
            // op LOAD_APP is a signal from the base image that it's ready to be told which app CID to attempt to initialize
            // This results in length of app CID (BE u64) and the CID itself being written into the flash drive
            // If a snapshot doesn't exist for the base image being in LOAD_APP mode, it's done
            LOAD_APP => {
                tracing::info!("LOAD_APP");
                if machine_loaded_state == 0 {
                    tracing::info!(
                        "snapshot stage load app to dir: /data/snapshot/base and read iflag : {}",
                        machine.read_iflags_y().await.unwrap()
                    );
                    machine.store("/data/snapshot/base").await.unwrap();
                    tracing::info!("done snapshotting");
                }
                let app_cid: cid::CidGeneric<64> = Cid::try_from(app_cid.clone()).unwrap();

                tracing::info!("app cid {:?}", Cid::try_from(app_cid.clone()).unwrap());
                let cid_length = app_cid.clone().to_bytes().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(app_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();
                tracing::info!("load app info was written");
            }

            // This is currently a null-op, but HINT is meant to tell the host / dehashing database/provider that there's an expectation
            // that certain hashes/CIDs are available for dehashing/resolving
            // In memory this is a BE u64 value of length of payload 
            HINT => {
                tracing::info!("HINT");

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let payload = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, payload_length)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                tracing::info!("hint payload {:?}", payload);
            }
            _ => {
                // XXX this should be a fatal error
                tracing::info!("unknown opt {:?}", opt)
            }
        }
        // XXX We should basically not get here in a well-behaved app, it should FINISH (accept/reject), EXCEPTION
        if interpreter_break_reason == Value::String("halted".to_string()) {
            tracing::info!("halted");
            machine.destroy().await.unwrap();
            machine.shutdown().await.unwrap();
            // XXX we should return here
        }
        // After handling the operation, we clear the yield flag and loop back and continue execution of machine
        machine.reset_iflags_y().await.unwrap();
    }
}
