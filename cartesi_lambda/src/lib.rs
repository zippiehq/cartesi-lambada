use async_std::stream::StreamExt;
use futures::AsyncBufRead;
use futures::AsyncReadExt;
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use hyper::body::to_bytes;
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper::{StatusCode, Uri};
use hyper_tls::HttpsConnector;

use async_std::task;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use rs_car_ipfs::single_file::read_single_file_seek;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::{thread, time::SystemTime};

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

// get metadata by 32-byte hash
const GET_METADATA: u64 = 0x00008;

// retrieve from a data source

const GET_DATA: u64 = 0x00009;


const NAMESPACE_KECCAK256: u64 = 0x2;


// execute is the entry point for the computation to be done, we have a particular state CID with it's associated /app directory
// and a transaction payload + metadata and we want to do this computation and get the new state CID back or an error
pub async fn execute(
    machine_url: String,
    ipfs_url: &str,
    ipfs_write_url: &str,
    payload: Option<Vec<u8>>,
    state_cid: Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    max_cycles_input: Option<u64>,
) -> Result<Cid, std::io::Error> {
    tracing::info!("state cid {:?}", state_cid.to_string());

    // Resolve what the app CID is in this current state
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}/api/v0/dag/resolve?arg={}/gov/app",
            ipfs_url,
            state_cid.to_string()
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let mut app_cid;
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

    let read_client = IpfsClient::from_str(ipfs_url).unwrap();
    let write_client = IpfsClient::from_str(ipfs_write_url).unwrap();

    let app_info_raw = read_client
        .cat(&format!("{}/info.json", app_cid.to_string()))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();

    let app_info = serde_json::from_slice::<serde_json::Value>(&app_info_raw).unwrap();
    let base_image = String::from(
        app_info
            .clone()
            .get("base_image_cid")
            .unwrap()
            .as_str()
            .unwrap(),
    );

    let base_image_cid = Cid::try_from(base_image.clone()).unwrap();

    tracing::info!("execute");

    // connect to a Cartesi Machine - we expect this to be an forked, empty machine and we control when it's shut down
    let machine = JsonRpcCartesiMachineClient::new(machine_url).await.unwrap();

    // The current state of the machine (0 = base image loaded, base image initialized (LOAD_APP), 1 = app initialized (LOAD_TX), 2 = processing a tx (should end in FINISH/EXCEPTION))
    let mut machine_loaded_state = 0;

    // TODO: we will have multiple base images in future
    if std::path::Path::new(&format!(
        "/data/snapshot/{}_{}",
        base_image,
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    ))
    .is_dir()
    {
        while std::path::Path::new(&format!(
            "/data/snapshot/{}_{}.lock",
            base_image,
            Cid::try_from(app_cid.clone()).unwrap().to_string()
        ))
        .exists()
        {
            tracing::info!(
                "waiting for {}_{}.lock",
                base_image,
                Cid::try_from(app_cid.clone()).unwrap().to_string()
            );
            thread::sleep(std::time::Duration::from_millis(500));
        }
        // There is a snapshot of this base machine with this app initialized and waiting for a transaction (LOAD_TX)
        tracing::info!(
            "loading machine from /data/snapshot/{}_{}",
            base_image,
            Cid::try_from(app_cid.clone()).unwrap().to_string()
        );
        let before_load_machine = SystemTime::now();
        machine
            .load_machine(
                &format!(
                    "/data/snapshot/{}_{}",
                    base_image,
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
    } else if std::path::Path::new(&format!("/data/snapshot/{}_bootedup", base_image)).exists() {
        // There is a snapshot of this base machine initialized and waiting to be populated with an app that'll initialize (LOAD_APP)
        while std::path::Path::new(&format!("/data/snapshot/{}_bootedup.lock", base_image)).exists()
        {
            tracing::info!("waiting for {}_bootedup.lock", base_image);
            thread::sleep(std::time::Duration::from_millis(500));
        }
        let before_load_machine = SystemTime::now();
        machine
            .load_machine(
                format!("/data/snapshot/{}_bootedup", base_image).as_str(),
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
    } else if std::path::Path::new(&format!("/data/snapshot/{}", base_image)).exists() {
        // no sane snapshot for us, we need to start from start
        while std::path::Path::new(&format!("/data/snapshot/{}.lock", base_image)).exists() {
            tracing::info!("waiting for {}.lock", base_image);
            thread::sleep(std::time::Duration::from_millis(500));
        }
        let before_load_machine = SystemTime::now();
        machine
            .load_machine(
                format!("/data/snapshot/{}", base_image).as_str(),
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
    } else {
        tracing::info!("grabbing new base machine {}", base_image);
        if std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(format!("/data/snapshot/{}.lock", base_image))
            .is_ok()
        {
            dedup_download_directory(
                ipfs_url,
                base_image_cid,
                format!("/data/snapshot/{}", base_image),
            )
            .await;
            std::fs::remove_file(format!("/data/snapshot/{}.lock", base_image)).unwrap();
            let before_load_machine = SystemTime::now();
            // XXX we should really do root hash check at least on downloaded stuff that we've been pointed to?
            // maybe a compute_with_callback flag that we can trust the machine? but need to be careful about derived snapshots then too?
            machine
                .load_machine(
                    format!("/data/snapshot/{}", base_image).as_str(),
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
        }
    }
    let mut max_cycles = u64::MAX;
    if let Some(m_cycle) = max_cycles_input {
        max_cycles = m_cycle;
    }
    loop {
        let mut interpreter_break_reason = Value::Null;
        // Are we yielded? If not, continue machine execution
        if !machine.read_iflags_y().await.unwrap() {
            interpreter_break_reason = machine.run(max_cycles).await.unwrap();
        }

        // Command/opcode is 64-bit big endian value at MACHINE_IO_ADDRESSS
        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let opt = u64::from_be_bytes(read_opt_be_bytes.try_into().unwrap());

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

                let block = read_client
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
                if !std::path::Path::new(&format!(
                    "/data/snapshot/{}_{}.lock",
                    base_image.as_str(),
                    app_cid.clone().to_string()
                ))
                .exists()
                    && !std::path::Path::new(&format!(
                        "/data/snapshot/{}_{}.lock",
                        base_image.as_str(),
                        app_cid.clone().to_string()
                    ))
                    .exists()
                    && (machine_loaded_state == 0 || machine_loaded_state == 1)
                {
                    if std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .open(format!(
                            "/data/snapshot/{}_{}.lock",
                            base_image,
                            app_cid.clone().to_string()
                        ))
                        .is_ok()
                    {
                        let arc_app_cid = Arc::new(app_cid.clone());
                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());

                        let base_image = base_image.clone();

                        thread::spawn(move || {
                            let _ = task::block_on(async move {
                                let forked_machine =
                                    JsonRpcCartesiMachineClient::new(forked_machine_url)
                                        .await
                                        .unwrap();
                                let app_cid: cid::CidGeneric<64> =
                                    Cid::try_from(arc_app_cid.to_string()).unwrap();
                                tracing::info!(
                                    "snapshot stage load tx to dir: {}",
                                    format!(
                                        "/data/snapshot/{}_{}",
                                        base_image,
                                        app_cid.clone().to_string()
                                    )
                                );

                                forked_machine
                                    .store(&format!(
                                        "/data/snapshot/{}_{}",
                                        base_image,
                                        app_cid.clone().to_string(),
                                    ))
                                    .await
                                    .unwrap();
                                forked_machine.destroy().await.unwrap();
                                forked_machine.shutdown().await.unwrap();
                                std::fs::remove_file(format!(
                                    "/data/snapshot/{}_{}.lock",
                                    base_image,
                                    app_cid.clone().to_string()
                                ))
                                .unwrap();
                                tracing::info!("done snapshotting app {}", app_cid.clone());
                            });
                        });
                    } else {
                        tracing::info!(
                            "did not manage to create lock, snapshot might already be in progress"
                        );
                    }
                } else {
                    tracing::info!("snapshot of app already being stored or stored, skipping snapshot (lock file exists)")
                }
                if payload.is_none() {
                    tracing::info!("machine warmed up");
                    machine.destroy().await.unwrap();
                    machine.shutdown().await.unwrap();
                    return Ok(Cid::default());
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

                let payload_length = payload.clone().unwrap().len() as u64;

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
                        STANDARD.encode(payload.clone().unwrap()),
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
                let put_response = write_client.block_put(data).await.unwrap();
            }
            // op LOAD_APP is a signal from the base image that it's ready to be told which app CID to attempt to initialize
            // This results in length of app CID (BE u64) and the CID itself being written into the flash drive
            // If a snapshot doesn't exist for the base image being in LOAD_APP mode, it's done
            LOAD_APP => {
                tracing::info!("LOAD_APP");

                if !std::path::Path::new(&format!("/data/snapshot/{}_bootedup", base_image))
                    .exists()
                    && !std::path::Path::new(&format!(
                        "/data/snapshot/{}_bootedup.lock",
                        base_image
                    ))
                    .exists()
                    && machine_loaded_state == 0
                {
                    if std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .open(format!("/data/snapshot/{}_bootedup.lock", base_image))
                        .is_ok()
                    {
                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());
                        let base_image = base_image.clone();

                        thread::spawn(move || {
                            let _ = task::block_on(async {
                                let forked_machine =
                                    JsonRpcCartesiMachineClient::new(forked_machine_url)
                                        .await
                                        .unwrap();
                                tracing::info!(
                                    "snapshot stage load app to dir: /data/snapshot/{}",
                                    format!("{}_bootedup", base_image)
                                );
                                forked_machine
                                    .store(&format!("/data/snapshot/{}_bootedup", base_image))
                                    .await
                                    .unwrap();
                                forked_machine.destroy().await.unwrap();
                                forked_machine.shutdown().await.unwrap();
                                std::fs::remove_file(format!(
                                    "/data/snapshot/{}_bootedup.lock",
                                    base_image
                                ))
                                .unwrap();
                                tracing::info!("done snapshotting {}_bootedup", base_image);
                            });
                        });
                    } else {
                        tracing::info!(
                            "did not manage to create lock, snapshot might already be in progress"
                        );
                    }
                } else {
                    tracing::info!("snapshot of base already exists or lock file exists, skipping");
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

                let payload: Vec<u8> =
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, payload_length)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap();

                tracing::info!("hint payload {:?}", payload);
                // XXX we send hint to the KECCAK256_SOURCE for now
                let https = HttpsConnector::new();
                let client = Client::builder().build::<_, hyper::Body>(https);
    
                let uri: String = format!(
                    "{}/hint/{}",
                    std::env::var("KECCAK256_SOURCE").unwrap(),
                    hex::encode(payload)
                )
                .parse()
                .unwrap();
    
                let block_req = Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap();
                client.request(block_req).await.unwrap();
            }
            // Handles the GET_METADATA action:
            // 1. Reads a 64-bit value from the specified memory address (MACHINE_IO_ADDRESSS + 8) and converts it from big-endian to a u64, representing the length of a metadata key
            // 2. Reads the metadata key of the specified length from memory starting at MACHINE_IO_ADDRESSS + 16
            // 3. Fetches the metadata associated with the key
            // 4. Writes the retrieved metadata back into the machine's memory at MACHINE_IO_ADDRESSS + 16, encoding using the STANDARD base64 (due to jsonrpc).
            // 5. Writes the length of the metadata as a big-endian byte array into the machine's memory at MACHINE_IO_ADDRESSS, also using the STANDARD encoding.
            //
            // Unavailable metadata should kill the machine, data requested by machine is assumed to be available.
            GET_METADATA => {
                tracing::info!("GET_METADATA");
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let metadata_key = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                tracing::info!("read metadata: {:?}", hex::encode(metadata_key.clone()));

                let metadata_result = metadata.get(&metadata_key).unwrap(); // handle this a bit better
                tracing::info!("metadata len {:?}", metadata_result.len());

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16,
                        STANDARD.encode(metadata_result.clone()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(metadata_result.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                tracing::info!("metadata info was written");
            }
            GET_DATA => {
                tracing::info!("GET_DATA");
                let namespace = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let id_length =u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let id = machine
                .read_memory(MACHINE_IO_ADDRESSS + 16, id_length)
                .await
                .unwrap();

                if namespace == NAMESPACE_KECCAK256 {
                    let https = HttpsConnector::new();
                    let client = Client::builder().build::<_, hyper::Body>(https);
        
                    let uri: String = format!(
                        "{}/dehash/{}",
                        std::env::var("KECCAK256_SOURCE").unwrap(),
                        hex::encode(id)
                    )
                    .parse()
                    .unwrap();
        
                    let block_req = Request::builder()
                        .method("GET")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap();
                    let block_response = client.request(block_req).await.unwrap();
                    let body_bytes = hyper::body::to_bytes(block_response).await.unwrap();        
                    machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16,
                        STANDARD.encode(body_bytes.clone()),
                    )
                    .await
                    .unwrap();
                    machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(body_bytes.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                } else {
                    panic!("unknown namespace");
                }
            }
            _ => {
                // XXX this should be a fatal error
                tracing::info!("unknown opt {:?}", opt)
            }
        }
        // We should basically not get here in a well-behaved app, it should FINISH (accept/reject), EXCEPTION
        if interpreter_break_reason == Value::String("halted".to_string()) {
            tracing::info!("halted");
            machine.destroy().await.unwrap();
            machine.shutdown().await.unwrap();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "machine halted without finishing, exception",
            ));
        }
        if interpreter_break_reason == Value::String("reached_target_mcycle".to_string()) {
            tracing::info!("reached cycles limit before completion of execution");
            machine.destroy().await.unwrap();
            machine.shutdown().await.unwrap();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "reached cycles limit before completion of execution",
            ));
        }
        // After handling the operation, we clear the yield flag and loop back and continue execution of machine
        machine.reset_iflags_y().await.unwrap();
    }
}

async fn dedup_download_directory(ipfs_url: &str, directory_cid: Cid, out_file_path: String) {
    let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();
    let res = ipfs_client
        .ls(format!("/ipfs/{}", directory_cid.to_string()).as_str())
        .await
        .unwrap();
    let first_object = res.objects.first().unwrap();

    std::fs::create_dir_all(out_file_path.clone()).unwrap();

    for val in &first_object.links {
        let req = Request::builder()
            .method("POST")
            .uri(format!("{}/api/v0/dag/export?arg={}", ipfs_url, val.hash))
            .body(hyper::Body::empty())
            .unwrap();

        let client = hyper::Client::new();

        match client.request(req).await {
            Ok(res) => {
                let mut f = res
                    .into_body()
                    .map(|result| {
                        result.map_err(|error| {
                            std::io::Error::new(std::io::ErrorKind::Other, "Error!")
                        })
                    })
                    .into_async_read();

                let mut out = async_std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(format!("{}/{}", out_file_path, val.name.clone()))
                    .await
                    .unwrap();
                let root_cid = rs_car::Cid::try_from(val.hash.clone()).unwrap();
                tracing::info!(
                    "storing file from CAR of {} into {}/{}",
                    val.hash.clone(),
                    out_file_path,
                    val.name.clone()
                );

                read_single_file_seek(&mut f, &mut out, None).await.unwrap();
            }
            Err(er) => {
                println!("{}", er.to_string());
            }
        }
    }
}
