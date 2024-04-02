use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use async_std::stream::StreamExt;
use async_std::{sync::Mutex, task};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use futures::TryStreamExt;
use hyper::Request;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use nix::unistd::{fork, ForkResult};
use os_pipe::PipeReader;
use os_pipe::{dup_stdin, dup_stdout};
use polling::{Event, Events, Poller};
use rs_car_ipfs::single_file::read_single_file_seek;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::{thread, time::SystemTime};
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

const SPAWN_COMPUTE: u64 = 0x0000A;

const JOIN_COMPUTE: u64 = 0x0000B;
fn main() {
    let stdin = dup_stdin().unwrap();
    let poller = Poller::new().unwrap();
    let key = &stdin.as_fd().as_raw_fd();
    unsafe {
        poller
            .add(&stdin.as_fd(), Event::readable(key.clone() as usize))
            .unwrap()
    };
    let mut children_hash_map: HashMap<usize, (Pid, Rc<PipeReader>)> = HashMap::new();
    let mut events = Events::new();
    loop {
        events.clear();
        poller
            .wait(&mut events, Some(Duration::from_millis(100)))
            .unwrap();

        for ev in events.iter() {
            if ev.key == key.clone() as usize {
                if let Ok(parameter) = read_message(&stdin) {
                    let (reader_for_parent, writer_for_parent) = os_pipe::pipe().unwrap();
                    let (reader_for_child, writer_for_child) = os_pipe::pipe().unwrap();
                    let execute_parameter =
                        serde_json::from_slice::<ExecuteParameters>(&parameter).unwrap();
                    match unsafe { fork() } {
                        Ok(ForkResult::Parent { child, .. }) => {
                            tracing::info!(
                                "Continuing execution in parent process, new child has pid: {}",
                                child
                            );
                            drop(reader_for_parent);
                            drop(writer_for_child);

                            write_message(&writer_for_parent, &execute_parameter).unwrap();

                            drop(writer_for_parent);
                            let reader_for_child = Rc::new(reader_for_child);
                            children_hash_map.insert(
                                reader_for_child.clone().as_fd().as_raw_fd() as usize,
                                (child, reader_for_child.clone()),
                            );
                            unsafe {
                                poller
                                    .add(
                                        &reader_for_child.as_fd(),
                                        Event::readable(reader_for_child.as_raw_fd() as usize),
                                    )
                                    .unwrap();
                            }
                        }
                        Ok(ForkResult::Child) => {
                            drop(reader_for_child);
                            drop(writer_for_parent);
                            let random_number = rand::random::<u64>();
                            let my_stdout =
                                File::create(format!("/tmp/{}-stdout.log", random_number))
                                    .expect("Failed to create stdout file");
                            let my_stderr =
                                File::create(format!("/tmp/{}-stderr.log", random_number))
                                    .expect("Failed to create stderr file");
                            let stdout_fd = my_stdout.as_raw_fd();
                            let stderr_fd = my_stderr.as_raw_fd();
                            unsafe {
                                libc::close(1);
                                libc::close(2);
                                libc::dup2(stdout_fd, 1);
                                libc::dup2(stderr_fd, 2);
                            }
                            if let Ok(parent_input) = read_message(&reader_for_parent) {
                                let input =
                                    serde_json::from_slice::<ExecuteParameters>(&parent_input)
                                        .unwrap();

                                task::block_on(async {
                                    setup_logging();
                                    setup_backtrace();
                                    let result = execute(
                                        input.machine_url,
                                        input.ipfs_url.as_str(),
                                        input.ipfs_write_url.as_str(),
                                        input.payload,
                                        Cid::try_from(input.state_cid).unwrap(),
                                        input
                                            .metadata
                                            .iter()
                                            .map(|(k, v)| {
                                                (hex::decode(&k).unwrap(), hex::decode(&v).unwrap())
                                            })
                                            .collect::<HashMap<Vec<u8>, Vec<u8>>>(),
                                        input.max_cycles_input,
                                    )
                                    .await;
                                    let response = ExecuteResult {
                                        identifier: input.identifier,
                                        result,
                                    };
                                    write_message(&writer_for_child, &response).unwrap();
                                });
                            }
                            drop(reader_for_parent);
                            std::process::exit(0);
                        }
                        Err(_) => {}
                    }
                }
            } else if children_hash_map.contains_key(&ev.key) {
                tracing::info!("Reading event came in");
                let reader_for_child = children_hash_map.remove_entry(&ev.key).unwrap();
                if let Ok(execute_output) = read_message(&reader_for_child.1 .1.clone()) {
                    let execute_result = serde_json::from_slice::<ExecuteResult>(&execute_output)
                        .expect("error parse execute result");
                    let stdout = dup_stdout().unwrap();
                    write_message(&stdout, &execute_result).unwrap();
                    drop(stdout);
                }
                waitpid(reader_for_child.1 .0, None).unwrap();
                poller
                    .modify(&stdin.as_fd(), Event::readable(key.clone() as usize))
                    .unwrap();
            }
        }
    }
}
async fn execute(
    machine_url: String,
    ipfs_url: &str,
    ipfs_write_url: &str,
    payload: Option<Vec<u8>>,
    state_cid: Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    max_cycles_input: Option<u64>,
) -> Result<Vec<u8>, serde_error::Error> {
    let mut thread_execute: HashMap<
        Vec<u8>,
        thread::JoinHandle<Result<Vec<u8>, serde_error::Error>>,
    > = HashMap::<Vec<u8>, thread::JoinHandle<Result<Vec<u8>, serde_error::Error>>>::new();
    tracing::info!("state cid {:?}", state_cid.to_string());
    let time_before_receive_app_cid = SystemTime::now();
    let mut measure_execution_time = false;
    if cfg!(feature = "measure_execution_time") {
        measure_execution_time = true;
    }
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
    let time_after_receive_app_cid = SystemTime::now();
    if measure_execution_time {
        tracing::info!(
            "resolving current app CID took {} milliseconds",
            time_after_receive_app_cid
                .duration_since(time_before_receive_app_cid)
                .unwrap()
                .as_millis()
        );
    }
    tracing::info!(
        "app cid {:?}",
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    );

    let read_client = IpfsClient::from_str(ipfs_url).unwrap();
    let write_client = IpfsClient::from_str(ipfs_write_url).unwrap();

    let time_before_receive_info_file = SystemTime::now();

    let app_info_raw = read_client
        .cat(&format!("{}/info.json", app_cid.to_string()))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();

    let time_after_receive_info_file = SystemTime::now();

    if measure_execution_time {
        tracing::info!(
            "receiving {}/info.json took {} milliseconds",
            app_cid.to_string(),
            time_after_receive_info_file
                .duration_since(time_before_receive_info_file)
                .unwrap()
                .as_millis()
        );
    }

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
    let machine = JsonRpcCartesiMachineClient::new(machine_url.clone())
        .await
        .unwrap();

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
            let before_dedup_download_directory = SystemTime::now();
            dedup_download_directory(
                ipfs_url,
                base_image_cid,
                format!("/data/snapshot/{}", base_image),
            )
            .await;
            let after_dedup_download_directory = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "Full dedup_load_directory took {} milliseconds",
                    after_dedup_download_directory
                        .duration_since(before_dedup_download_directory)
                        .unwrap()
                        .as_millis()
                );
            }

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
        let time_before_read_iflags_y = SystemTime::now();
        // Are we yielded? If not, continue machine execution
        if !machine.read_iflags_y().await.unwrap() {
            let time_after_read_iflags_y = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "reading iflags y took {} milliseconds",
                    time_after_read_iflags_y
                        .duration_since(time_before_read_iflags_y)
                        .unwrap()
                        .as_millis()
                );
            }
            let time_before_run_machine = SystemTime::now();
            interpreter_break_reason = machine.run(max_cycles).await.unwrap();
            let time_after_run_machine = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "machine running to {} cycles y took {} milliseconds",
                    max_cycles,
                    time_after_run_machine
                        .duration_since(time_before_run_machine)
                        .unwrap()
                        .as_millis()
                );
            }
        }
        let time_before_read_opt = SystemTime::now();

        // Command/opcode is 64-bit big endian value at MACHINE_IO_ADDRESSS
        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let time_after_read_opt = SystemTime::now();
        if measure_execution_time {
            tracing::info!(
                "read_memory(MACHINE_IO_ADDRESSS, 8)(reading opt) took {} milliseconds",
                time_after_read_opt
                    .duration_since(time_before_read_opt)
                    .unwrap()
                    .as_millis()
            );
        }

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
                let time_before_read_cid_length = SystemTime::now();

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_read_cid_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8) (cid length) took {} milliseconds",
                        time_after_read_cid_length
                            .duration_since(time_before_read_cid_length)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_read_cid = SystemTime::now();

                let cid = Cid::try_from(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                        .await
                        .unwrap(),
                )
                .unwrap();
                let time_after_read_cid = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 16, length) (length) took {} milliseconds",
                        time_after_read_cid
                            .duration_since(time_before_read_cid)
                            .unwrap()
                            .as_millis()
                    );
                }

                tracing::info!("read cid {:?}", cid.to_string());
                let time_before_get_block = SystemTime::now();
                let block = read_client
                    .block_get(cid.to_string().as_str())
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();
                let time_after_get_block = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "block_get({}) took {} milliseconds",
                        cid.to_string(),
                        time_after_get_block
                            .duration_since(time_before_get_block)
                            .unwrap()
                            .as_millis()
                    );
                }

                tracing::info!("block len {:?}", block.len());
                let time_before_write_block = SystemTime::now();

                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 16, STANDARD.encode(block.clone()))
                    .await
                    .unwrap();
                let time_after_write_block = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS + 16, STANDARD.encode(block.clone())) (writing encoded block) took {} milliseconds",
                        time_after_write_block
                        .duration_since(time_before_write_block)
                        .unwrap()
                        .as_millis()
                    );
                }

                let time_before_write_block_len = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(block.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                let time_after_write_block_len = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS, STANDARD.encode(block.len().to_be_bytes().to_vec())) (writing block length) took {} milliseconds",
                        time_after_write_block_len
                        .duration_since(time_before_write_block_len)
                        .unwrap()
                        .as_millis()
                    );
                }

                tracing::info!("read_block info was written");
            }
            // Handles the EXCEPTION case coming from VM/guest:
            // 1. Destroys the current state of the machine, releasing any resources it was using.
            // 2. Shuts down the machine, ensuring all ongoing processes are terminated.
            // 3. Returns an error of type `std::io::Error` with the kind `std::io::ErrorKind::Other`, indicating a general error, and a message "exception" to signify that an exception occurred.
            EXCEPTION => {
                tracing::info!("HTIF_YIELD_REASON_TX_EXCEPTION");
                let before_destroying_machine = SystemTime::now();
                machine.destroy().await.unwrap();
                let after_destroying_machine = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "destroying machine took {} milliseconds",
                        after_destroying_machine
                            .duration_since(before_destroying_machine)
                            .unwrap()
                            .as_millis()
                    );
                }

                let before_shutting_down_machine = SystemTime::now();
                machine.shutdown().await.unwrap();
                let after_shutting_down_machine = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "shutting down machine took {} milliseconds",
                        after_shutting_down_machine
                            .duration_since(before_shutting_down_machine)
                            .unwrap()
                            .as_millis()
                    );
                }
                return Err(serde_error::Error::new(&std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "exception",
                )));
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
                        let time_before_forking_machine = SystemTime::now();

                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());

                        let time_after_forking_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "storing forked machine {} took {} milliseconds",
                                forked_machine_url,
                                time_after_forking_machine
                                    .duration_since(time_before_forking_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let base_image = base_image.clone();

                        //thread::spawn(move || {
                        //let _ = task::block_on(async move {
                        let forked_machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
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
                        let time_before_storing_forked_machine = SystemTime::now();

                        forked_machine
                            .store(&format!(
                                "/data/snapshot/{}_{}",
                                base_image,
                                app_cid.clone().to_string(),
                            ))
                            .await
                            .unwrap();
                        let time_after_storing_forked_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "storing forked machine /data/snapshot/{}_{} took {} milliseconds",
                                base_image,
                                app_cid.clone().to_string(),
                                time_after_storing_forked_machine
                                    .duration_since(time_before_storing_forked_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }
                        let before_destroying_machine = SystemTime::now();
                        forked_machine.destroy().await.unwrap();
                        let after_destroying_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "destroying forked machine took {} milliseconds",
                                after_destroying_machine
                                    .duration_since(before_destroying_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }
                        let before_shutting_down_machine = SystemTime::now();
                        forked_machine.shutdown().await.unwrap();
                        let after_shutting_down_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "shutting down forked machine took {} milliseconds",
                                after_shutting_down_machine
                                    .duration_since(before_shutting_down_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }
                        std::fs::remove_file(format!(
                            "/data/snapshot/{}_{}.lock",
                            base_image,
                            app_cid.clone().to_string()
                        ))
                        .unwrap();
                        tracing::info!("done snapshotting app {}", app_cid.clone());
                        //});
                        //});
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
                    let before_destroying_machine = SystemTime::now();
                    machine.destroy().await.unwrap();
                    let after_destroying_machine = SystemTime::now();
                    if measure_execution_time {
                        tracing::info!(
                            "destroying machine took {} milliseconds",
                            after_destroying_machine
                                .duration_since(before_destroying_machine)
                                .unwrap()
                                .as_millis()
                        );
                    }

                    let before_shutting_down_machine = SystemTime::now();
                    machine.shutdown().await.unwrap();
                    let after_shutting_down_machine = SystemTime::now();
                    if measure_execution_time {
                        tracing::info!(
                            "shutting down machine took {} milliseconds",
                            after_shutting_down_machine
                                .duration_since(before_shutting_down_machine)
                                .unwrap()
                                .as_millis()
                        );
                    }

                    return Ok(Vec::new());
                }
                let cid_length = state_cid.clone().to_bytes().len() as u64;
                let time_before_write_cid_length = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                let time_after_write_cid_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS, STANDARD.encode(cid_length.to_be_bytes().to_vec())) (writing encoded cid length) took {} milliseconds",
                        time_after_write_cid_length
                        .duration_since(time_before_write_cid_length)
                        .unwrap()
                        .as_millis()
                    );
                }

                let time_before_write_state_cid = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(state_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();
                let time_after_write_state_cid = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS + 8, STANDARD.encode(state_cid.clone().to_bytes()),)(writing encoded cid) took {} milliseconds",
                        time_after_write_state_cid
                        .duration_since(time_before_write_state_cid)
                        .unwrap()
                        .as_millis()
                    );
                }

                let payload_length = payload.clone().unwrap().len() as u64;

                let time_before_write_payload_length = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16 + cid_length,
                        STANDARD.encode(payload_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                let time_after_write_payload_length = SystemTime::now();

                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS + 16 + cid_length, STANDARD.encode(payload_length.to_be_bytes().to_vec()))(writing payload length) took {} milliseconds",
                        time_after_write_payload_length
                        .duration_since(time_before_write_payload_length)
                        .unwrap()
                        .as_millis()
                    );
                }

                let time_before_write_payload = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 24 + cid_length,
                        STANDARD.encode(payload.clone().unwrap()),
                    )
                    .await
                    .unwrap();

                let time_after_write_payload = SystemTime::now();

                if measure_execution_time {
                    tracing::info!(
                            "write_memory(MACHINE_IO_ADDRESSS + 24 + cid_length, STANDARD.encode(payload.clone().unwrap()))(writing payload) took {} milliseconds",
                            time_after_write_payload
                        .duration_since(time_before_write_payload)
                        .unwrap()
                        .as_millis()
                        );
                }

                tracing::info!("load_tx info was written");
            }
            // FINISH: The guest has finished execution and reported back a status code, accept or rejection of the transaction.
            // It also reports back the current IPFS CID of the state which we return to the consumer of this function.
            FINISH => {
                tracing::info!("FINISH");
                let time_before_read_status = SystemTime::now();

                let status = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_read_status = SystemTime::now();

                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8)(reading status) took {} milliseconds",
                        time_after_read_status
                        .duration_since(time_before_read_status)
                        .unwrap()
                        .as_millis()
                    );
                }

                let time_before_read_data_length = SystemTime::now();

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_read_data_length = SystemTime::now();

                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8)(reading status) took {} milliseconds",
                        time_after_read_data_length
                        .duration_since(time_before_read_data_length)
                        .unwrap()
                        .as_millis()
                    );
                }

                let time_before_read_data = SystemTime::now();

                let data = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 24, length)
                    .await
                    .unwrap();
                let time_after_read_data = SystemTime::now();

                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 24, length)(reading data) took {} milliseconds",
                        time_after_read_data
                        .duration_since(time_before_read_data)
                        .unwrap()
                        .as_millis()
                    );
                }

                match status {
                    0 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        let before_destroying_machine = SystemTime::now();
                        machine.destroy().await.unwrap();
                        let after_destroying_machine = SystemTime::now();

                        if measure_execution_time {
                            tracing::info!(
                                "destroying machine took {} milliseconds",
                                after_destroying_machine
                                    .duration_since(before_destroying_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let before_shutting_down_machine = SystemTime::now();
                        machine.shutdown().await.unwrap();
                        let after_shutting_down_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "shutting down machine took {} milliseconds",
                                after_shutting_down_machine
                                    .duration_since(before_shutting_down_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        tracing::info!(
                            "FINISH received cid {}",
                            Cid::try_from(data.clone()).unwrap()
                        );

                        return Ok(data);
                    }
                    1 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_REJECTED");
                        let before_destroying_machine = SystemTime::now();
                        machine.destroy().await.unwrap();
                        let after_destroying_machine = SystemTime::now();

                        if measure_execution_time {
                            tracing::info!(
                                "destroying machine took {} milliseconds",
                                after_destroying_machine
                                    .duration_since(before_destroying_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let before_shutting_down_machine = SystemTime::now();
                        machine.shutdown().await.unwrap();
                        let after_shutting_down_machine = SystemTime::now();

                        if measure_execution_time {
                            tracing::info!(
                                "shutting down machine took {} milliseconds",
                                after_shutting_down_machine
                                    .duration_since(before_shutting_down_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }
                        return Err(serde_error::Error::new(&std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "transaction was rejected",
                        )));
                    }
                    _ => {
                        let before_destroying_machine = SystemTime::now();
                        machine.destroy().await.unwrap();
                        let after_destroying_machine = SystemTime::now();

                        if measure_execution_time {
                            tracing::info!(
                                "destroying machine took {} milliseconds",
                                after_destroying_machine
                                    .duration_since(before_destroying_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let before_shutting_down_machine = SystemTime::now();
                        machine.shutdown().await.unwrap();
                        let after_shutting_down_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "shutting down machine took {} milliseconds",
                                after_shutting_down_machine
                                    .duration_since(before_shutting_down_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }
                        return Err(serde_error::Error::new(&std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "unknown status",
                        )));
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
                let time_before_read_data_length = SystemTime::now();

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_read_data_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8)(reading data length) took {} milliseconds",
                        time_after_read_data_length
                            .duration_since(time_before_read_data_length)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_read_data = SystemTime::now();

                let memory = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                let time_after_read_data = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                            "read_memory(MACHINE_IO_ADDRESSS + 16, length)(reading data ) took {} milliseconds",
                            time_after_read_data
                                .duration_since(time_before_read_data)
                                .unwrap()
                                .as_millis()
                        );
                }

                let data = Cursor::new(memory.clone());
                let time_before_putting_block = SystemTime::now();

                let put_response = write_client.block_put(data).await.unwrap();
                let time_after_putting_block = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "block_put(data)(putting data) took {} milliseconds",
                        time_after_putting_block
                            .duration_since(time_before_putting_block)
                            .unwrap()
                            .as_millis()
                    );
                }
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
                        let time_before_forking_machine = SystemTime::now();

                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());
                        let time_after_forking_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "storing forked machine {} took {} milliseconds",
                                forked_machine_url,
                                time_after_forking_machine
                                    .duration_since(time_before_forking_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let base_image = base_image.clone();

                        //thread::spawn(move || {
                        //let _ = task::block_on(async {
                        let forked_machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
                            .await
                            .unwrap();
                        tracing::info!(
                            "snapshot stage load app to dir: /data/snapshot/{}",
                            format!("{}_bootedup", base_image)
                        );
                        let time_before_storing_forked_machine = SystemTime::now();

                        forked_machine
                            .store(&format!("/data/snapshot/{}_bootedup", base_image))
                            .await
                            .unwrap();
                        let time_after_storing_forked_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                        "storing forked machine /data/snapshot/{}_bootedup took {} milliseconds",
                                        base_image,
                                        time_after_storing_forked_machine
                                            .duration_since(time_before_storing_forked_machine)
                                            .unwrap()
                                            .as_millis()
                                    );
                        }

                        let before_destroying_machine = SystemTime::now();
                        forked_machine.destroy().await.unwrap();
                        let after_destroying_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "destroying forked machine took {} milliseconds",
                                after_destroying_machine
                                    .duration_since(before_destroying_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        let before_shutting_down_machine = SystemTime::now();
                        forked_machine.shutdown().await.unwrap();
                        let after_shutting_down_machine = SystemTime::now();
                        if measure_execution_time {
                            tracing::info!(
                                "shutting down forked machine took {} milliseconds",
                                after_shutting_down_machine
                                    .duration_since(before_shutting_down_machine)
                                    .unwrap()
                                    .as_millis()
                            );
                        }

                        std::fs::remove_file(format!(
                            "/data/snapshot/{}_bootedup.lock",
                            base_image
                        ))
                        .unwrap();
                        tracing::info!("done snapshotting {}_bootedup", base_image);
                        //});
                        //});
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
                let time_before_writing_cid_length = SystemTime::now();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                let time_after_writing_cid_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS, STANDARD.encode(cid_length.to_be_bytes().to_vec()))(writing cid length) took {} milliseconds",
                        time_after_writing_cid_length
                            .duration_since(time_before_writing_cid_length)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_writing_app_cid = SystemTime::now();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(app_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();
                let time_after_writing_app_cid = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS + 8, STANDARD.encode(app_cid.clone().to_bytes()),)(writing cid length) took {} milliseconds",
                        time_after_writing_app_cid
                            .duration_since(time_before_writing_app_cid)
                            .unwrap()
                            .as_millis()
                    );
                }

                tracing::info!("load app info was written");
            }

            // This is currently a null-op, but HINT is meant to tell the host / dehashing database/provider that there's an expectation
            // that certain hashes/CIDs are available for dehashing/resolving
            // In memory this is a BE u64 value of length of payload
            HINT => {
                tracing::info!("HINT");
                let time_before_reading_payload_length = SystemTime::now();

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_reading_payload_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8)(reading payload length) took {} milliseconds",
                        time_after_reading_payload_length
                            .duration_since(time_before_reading_payload_length)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_reading_payload = SystemTime::now();

                let payload = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, payload_length)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let time_after_reading_payload = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 16, payload_length)(reading payload) took {} milliseconds",
                        time_after_reading_payload
                            .duration_since(time_before_reading_payload)
                            .unwrap()
                            .as_millis()
                    );
                }

                tracing::info!("hint payload {:?}", payload);
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
                let time_before_reading_metadata_key_length = SystemTime::now();

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let time_after_reading_metadata_key_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 8, 8)(reading metadata key length) took {} milliseconds",
                        time_after_reading_metadata_key_length
                            .duration_since(time_before_reading_metadata_key_length)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_reading_metadata_key = SystemTime::now();

                let metadata_key = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                let time_after_reading_metadata_key = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_memory(MACHINE_IO_ADDRESSS + 16, length)(reading metadata key) took {} milliseconds",
                        time_after_reading_metadata_key
                            .duration_since(time_before_reading_metadata_key)
                            .unwrap()
                            .as_millis()
                    );
                }

                tracing::info!("read metadata: {:?}", hex::encode(metadata_key.clone()));

                let metadata_result = metadata.get(&metadata_key).unwrap(); // handle this a bit better
                tracing::info!("metadata len {:?}", metadata_result.len());
                let time_before_writing_metadata = SystemTime::now();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16,
                        STANDARD.encode(metadata_result.clone()),
                    )
                    .await
                    .unwrap();
                let time_after_writing_metadata = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS + 16, STANDARD.encode(metadata_result.clone()))(writing encoded metadata) took {} milliseconds",
                        time_after_writing_metadata
                            .duration_since(time_before_writing_metadata)
                            .unwrap()
                            .as_millis()
                    );
                }

                let time_before_writing_metadata_length = SystemTime::now();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(metadata_result.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                let time_after_writing_metadata_length = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "write_memory(MACHINE_IO_ADDRESSS, STANDARD.encode(metadata_result.len().to_be_bytes().to_vec()))(writing encoded metadata length) took {} milliseconds",
                        time_after_writing_metadata_length
                            .duration_since(time_before_writing_metadata_length)
                            .unwrap()
                            .as_millis()
                    );
                }
                tracing::info!("metadata info was written");
            }
            SPAWN_COMPUTE => {
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid_bytes = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();
                let cid = Cid::try_from(cid_bytes.clone()).unwrap();

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16 + length, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let payload = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 24 + length, payload_length)
                    .await
                    .unwrap();
                let ipfs_url = Arc::new(Mutex::new(ipfs_url.to_string()));
                let ipfs_write_url = Arc::new(Mutex::new(ipfs_write_url.to_string()));
                let machine_url = Arc::new(Mutex::new(machine_url.clone()));

                let mut hasher = Sha256::new();
                hasher.update(cid_bytes);
                hasher.update(payload.clone());
                let thread_hash = hasher.finalize().to_vec();

                thread_execute.insert(
                    thread_hash,
                    thread::spawn(move || {
                        let ipfs_url = Arc::clone(&ipfs_url);
                        let ipfs_write_url = Arc::clone(&ipfs_write_url);
                        let machine_url = Arc::clone(&machine_url);

                        task::block_on(async {
                            let mut metadata: HashMap<Vec<u8>, Vec<u8>> =
                                HashMap::<Vec<u8>, Vec<u8>>::new();

                            metadata.insert(
                                calculate_sha256("sequencer".as_bytes()),
                                calculate_sha256("spawn".as_bytes()),
                            );
                            execute(
                                machine_url.lock().await.clone(),
                                ipfs_url.lock().await.clone().as_str(),
                                ipfs_write_url.lock().await.clone().as_str(),
                                Some(payload),
                                cid,
                                metadata,
                                max_cycles_input,
                            )
                            .await
                        })
                    }),
                );
            }
            JOIN_COMPUTE => {
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid_bytes = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();
                let cid = Cid::try_from(cid_bytes.clone()).unwrap();

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16 + length, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let payload = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 24 + length, payload_length)
                    .await
                    .unwrap();
                let ipfs_url = Arc::new(Mutex::new(ipfs_url.to_string()));
                let ipfs_write_url = Arc::new(Mutex::new(ipfs_write_url.to_string()));
                let machine_url = Arc::new(Mutex::new(machine_url.clone()));

                let mut hasher = Sha256::new();
                hasher.update(cid_bytes);
                hasher.update(payload.clone());
                let thread_hash = hasher.finalize().to_vec();

                let mut execute_result = None;
                if let Some(thread) = thread_execute.remove(&thread_hash) {
                    execute_result = Some(thread.join().unwrap());
                } else {
                    execute_result = Some(
                        thread::spawn(move || {
                            let ipfs_url = Arc::clone(&ipfs_url);
                            let ipfs_write_url = Arc::clone(&ipfs_write_url);
                            let machine_url = Arc::clone(&machine_url);

                            task::block_on(async {
                                let mut metadata: HashMap<Vec<u8>, Vec<u8>> =
                                    HashMap::<Vec<u8>, Vec<u8>>::new();

                                metadata.insert(
                                    calculate_sha256("sequencer".as_bytes()),
                                    calculate_sha256("spawn".as_bytes()),
                                );
                                execute(
                                    machine_url.lock().await.clone(),
                                    ipfs_url.lock().await.clone().as_str(),
                                    ipfs_write_url.lock().await.clone().as_str(),
                                    Some(payload),
                                    cid,
                                    metadata,
                                    max_cycles_input,
                                )
                                .await
                            })
                        })
                        .join()
                        .unwrap(),
                    )
                }

                match execute_result.unwrap() {
                    Ok(cid) => {
                        machine
                            .write_memory(
                                MACHINE_IO_ADDRESSS + 8,
                                STANDARD.encode(0_u64.to_be_bytes()),
                            )
                            .await
                            .unwrap();

                        machine
                            .write_memory(
                                MACHINE_IO_ADDRESSS + 16,
                                STANDARD.encode(cid.len().to_be_bytes()),
                            )
                            .await
                            .unwrap();

                        machine
                            .write_memory(MACHINE_IO_ADDRESSS + 24, STANDARD.encode(cid))
                            .await
                            .unwrap();
                    }
                    Err(_) => {
                        machine
                            .write_memory(
                                MACHINE_IO_ADDRESSS + 8,
                                STANDARD.encode(1_u64.to_be_bytes()),
                            )
                            .await
                            .unwrap();
                    }
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
            let before_destroying_machine = SystemTime::now();
            machine.destroy().await.unwrap();
            let after_destroying_machine = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "destroying machine took {} milliseconds",
                    after_destroying_machine
                        .duration_since(before_destroying_machine)
                        .unwrap()
                        .as_millis()
                );
            }

            let before_shutting_down_machine = SystemTime::now();
            machine.shutdown().await.unwrap();
            let after_shutting_down_machine = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "shutting down machine took {} milliseconds",
                    after_shutting_down_machine
                        .duration_since(before_shutting_down_machine)
                        .unwrap()
                        .as_millis()
                );
            }
            return Err(serde_error::Error::new(&std::io::Error::new(
                std::io::ErrorKind::Other,
                "machine halted without finishing, exception",
            )));
        }
        if interpreter_break_reason == Value::String("reached_target_mcycle".to_string()) {
            tracing::info!("reached cycles limit before completion of execution");
            let before_destroying_machine = SystemTime::now();
            machine.destroy().await.unwrap();
            let after_destroying_machine = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "destroying machine took {} milliseconds",
                    after_destroying_machine
                        .duration_since(before_destroying_machine)
                        .unwrap()
                        .as_millis()
                );
            }

            let before_shutting_down_machine = SystemTime::now();
            machine.shutdown().await.unwrap();
            let after_shutting_down_machine = SystemTime::now();
            if measure_execution_time {
                tracing::info!(
                    "shutting down machine took {} milliseconds",
                    after_shutting_down_machine
                        .duration_since(before_shutting_down_machine)
                        .unwrap()
                        .as_millis()
                );
            }
            return Err(serde_error::Error::new(&std::io::Error::new(
                std::io::ErrorKind::Other,
                "reached cycles limit before completion of execution",
            )));
        }
        let time_before_resetting_iflags_y = SystemTime::now();
        // After handling the operation, we clear the yield flag and loop back and continue execution of machine
        machine.reset_iflags_y().await.unwrap();
        let time_after_resetting_iflags_y = SystemTime::now();
        if measure_execution_time {
            tracing::info!(
                "iflags y resetting took {} milliseconds",
                time_after_resetting_iflags_y
                    .duration_since(time_before_resetting_iflags_y)
                    .unwrap()
                    .as_millis()
            );
        }
    }
}

async fn dedup_download_directory(ipfs_url: &str, directory_cid: Cid, out_file_path: String) {
    let mut measure_execution_time = false;
    if cfg!(feature = "measure_execution_time") {
        measure_execution_time = true;
    }
    let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();

    let before_ls_cid = SystemTime::now();
    let res = ipfs_client
        .ls(format!("/ipfs/{}", directory_cid.to_string()).as_str())
        .await
        .unwrap();
    let after_ls_cid = SystemTime::now();
    if measure_execution_time {
        tracing::info!(
            "ls(/ipfs/{}) took {} milliseconds",
            directory_cid.to_string(),
            after_ls_cid
                .duration_since(before_ls_cid)
                .unwrap()
                .as_millis()
        );
    }

    let first_object = res.objects.first().unwrap();

    std::fs::create_dir_all(out_file_path.clone()).unwrap();

    for val in &first_object.links {
        let before_dag_request = SystemTime::now();

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
                let before_reading_single_file_seek = SystemTime::now();

                read_single_file_seek(&mut f, &mut out, None).await.unwrap();
                let after_reading_single_file_seek = SystemTime::now();
                if measure_execution_time {
                    tracing::info!(
                        "read_single_file_seek took {} milliseconds",
                        after_reading_single_file_seek
                            .duration_since(before_reading_single_file_seek)
                            .unwrap()
                            .as_millis()
                    );
                }
            }
            Err(er) => {}
        }
        let after_dag_request = SystemTime::now();
        if measure_execution_time {
            tracing::info!(
                "{}/api/v0/dag/export?arg={} request handling took {} milliseconds",
                ipfs_url,
                val.hash,
                after_dag_request
                    .duration_since(before_dag_request)
                    .unwrap()
                    .as_millis()
            );
        }
    }
}

pub fn calculate_sha256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}

pub fn write_message<T>(mut pipe: &os_pipe::PipeWriter, data: &T) -> Result<(), std::io::Error>
where
    T: ?Sized + Serialize,
{
    let data_json = serde_json::to_string(&data).unwrap();
    pipe.write(&mut data_json.as_bytes().len().to_le_bytes())
        .unwrap();
    pipe.write(&mut data_json.as_bytes()).unwrap();
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteParameters {
    machine_url: String,
    ipfs_url: String,
    ipfs_write_url: String,
    payload: Option<Vec<u8>>,
    state_cid: Vec<u8>,
    metadata: HashMap<String, String>,
    max_cycles_input: Option<u64>,
    identifier: String,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteResult {
    result: Result<Vec<u8>, serde_error::Error>,
    identifier: String,
}
