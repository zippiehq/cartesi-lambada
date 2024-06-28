use alloy_primitives::{address, U256};
use alloy_sol_types::{sol, SolCall};
use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use async_std::stream::StreamExt;
use async_std::task;
use cartesi_machine::{configuration::RuntimeConfig, Machine};
use cid::Cid;
use futures::TryStreamExt;
use hyper::{Body, Client, Request};
use hyper_tls::HttpsConnector;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use nix::libc;
use nix::sys::wait::waitpid;
use nix::unistd::{fork, setsid, ForkResult, Pid};
use os_pipe::{dup_stdin, dup_stdout};
use os_pipe::{PipeReader, PipeWriter};
use polling::{Event, Events, Poller};
use rand::{distributions::Alphanumeric, Rng};
use rs_car_ipfs::single_file::read_single_file_seek;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;
use std::{thread, time::SystemTime};
const HTIF_YIELD_MANUAL_REASON_RX_ACCEPTED_DEF: u16 = 0x1;
const HTIF_YIELD_MANUAL_REASON_RX_REJECTED_DEF: u16 = 0x2;

const EXCEPTION: u16 = 0x4;
const SPAWN_COMPUTE: u16 = 0x27;
const JOIN_COMPUTE: u16 = 0x28;
const CURRENT_STATE_CID: u16 = 0x20;
const SET_STATE_CID: u16 = 0x21;
const METADATA: u16 = 0x22;
const KECCAK256_NAMESPACE: u16 = 0x23;
const EXTERNALIZE_STATE: u16 = 0x24;
const IPFS_GET_BLOCK: u16 = 0x25;
const HINT: u16 = 0x26;

const PMA_CMIO_RX_BUFFER_START_DEF: u64 = 0x60000000;
const PMA_CMIO_TX_BUFFER_START_DEF: u64 = 0x60800000;
const PMA_CMIO_RX_BUFFER_LOG2_SIZE_DEF: u64 = 21;
const PMA_CMIO_TX_BUFFER_LOG2_SIZE_DEF: u64 = 21;
const HTIF_YIELD_REASON_ADVANCE_STATE_DEF: u16 = 0;

fn main() {
    let log_directory_path: String =
        std::env::var("LAMBADA_LOGS_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let my_stderr = File::create(format!("{}/lambada-worker-stderr.log", log_directory_path))
        .expect("Failed to create stderr file");
    let stderr_fd = my_stderr.as_raw_fd();
    unsafe {
        libc::close(2);
        libc::dup2(stderr_fd, 2);
    }

    let stdin = dup_stdin().unwrap();
    let poller = Poller::new().unwrap();
    let key = &stdin.as_fd().as_raw_fd();
    unsafe {
        poller
            .add(&stdin.as_fd(), Event::readable(key.clone() as usize))
            .unwrap()
    };
    let mut children_hash_map: HashMap<
        usize,
        (Pid, Rc<PipeReader>, Option<Rc<PipeWriter>>, Rc<PipeWriter>),
    > = HashMap::new();
    let mut events = Events::new();
    loop {
        events.clear();
        poller
            .wait(&mut events, Some(Duration::from_millis(100)))
            .unwrap();

        for ev in events.iter() {
            if ev.key == key.clone() as usize {
                if let Ok(parameter) = read_message(&stdin) {
                    tracing::info!("Run forking");

                    run_forking(
                        parameter,
                        &mut children_hash_map,
                        &poller,
                        &stdin,
                        key,
                        None,
                    );
                }
            } else if children_hash_map.contains_key(&ev.key) {
                tracing::info!("Reading event came in");
                let child = children_hash_map.remove_entry(&ev.key).unwrap();
                if let Ok(execute_output) = read_message(&child.1 .1.clone()) {
                    if let Ok(execute_result) =
                        serde_json::from_slice::<ExecuteResult>(&execute_output)
                    {
                        tracing::info!("Sending Execute result");

                        if let Some(output_pipe) = child.1 .2 {
                            write_message(&output_pipe, &execute_result).unwrap();
                            drop(output_pipe);
                        } else {
                            let output_pipe = dup_stdout().unwrap();
                            write_message(&output_pipe, &execute_result).unwrap();
                            drop(output_pipe);
                        }
                        waitpid(child.1 .0, None).unwrap();
                    } else if let Ok(_) =
                        serde_json::from_slice::<ExecuteParameters>(&execute_output)
                    {
                        tracing::info!("JOIN_COMPUTE Forking");

                        run_forking(
                            execute_output,
                            &mut children_hash_map,
                            &poller,
                            &stdin,
                            key,
                            Some(child.1 .3),
                        );
                    }
                }
            }
        }
    }
}

fn run_forking(
    parameter: Vec<u8>,
    children_hash_map: &mut HashMap<
        usize,
        (Pid, Rc<PipeReader>, Option<Rc<PipeWriter>>, Rc<PipeWriter>),
    >,
    poller: &Poller,
    stdin: &PipeReader,
    key: &i32,
    requestor_pipe: Option<Rc<PipeWriter>>,
) {
    let (reader_for_parent, writer_for_parent) = os_pipe::pipe().unwrap();
    let (reader_for_child, writer_for_child) = os_pipe::pipe().unwrap();
    let input = serde_json::from_slice::<ExecuteParameters>(&parameter).unwrap();
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            drop(reader_for_parent);
            drop(writer_for_child);
            let reader_for_child = Rc::new(reader_for_child);
            let child_writer = Rc::new(writer_for_parent);

            children_hash_map.insert(
                reader_for_child.clone().as_fd().as_raw_fd() as usize,
                (
                    child,
                    reader_for_child.clone(),
                    requestor_pipe,
                    child_writer,
                ),
            );

            unsafe {
                poller
                    .add(
                        &reader_for_child.as_fd(),
                        Event::readable(reader_for_child.as_raw_fd() as usize),
                    )
                    .unwrap();
            }
            poller
                .modify(&stdin.as_fd(), Event::readable(key.clone() as usize))
                .unwrap();
        }
        Ok(ForkResult::Child) => {
            drop(reader_for_child);
            let log_directory_path: String =
                std::env::var("LAMBADA_LOGS_DIR").unwrap_or_else(|_| String::from("/tmp"));
            let my_stdout = File::create(format!(
                "{}/{}-stdout.log",
                log_directory_path, input.identifier
            ))
            .expect("Failed to create stdout file");
            let my_stderr = File::create(format!(
                "{}/{}-stderr.log",
                log_directory_path, input.identifier
            ))
            .expect("Failed to create stderr file");
            let stdout_fd = my_stdout.as_raw_fd();
            let stderr_fd = my_stderr.as_raw_fd();
            unsafe {
                libc::close(1);
                libc::close(2);
                libc::dup2(stdout_fd, 1);
                libc::dup2(stderr_fd, 2);
            }
            task::block_on(async {
                setup_logging();
                setup_backtrace();
                let result = execute(
                    input.ipfs_url.as_str(),
                    input.ipfs_write_url.as_str(),
                    input.payload,
                    Cid::try_from(input.state_cid).unwrap(),
                    input
                        .metadata
                        .iter()
                        .map(|(k, v)| (hex::decode(&k).unwrap(), hex::decode(&v).unwrap()))
                        .collect::<HashMap<Vec<u8>, Vec<u8>>>(),
                    input.max_cycles_input,
                    &writer_for_child,
                    &reader_for_parent,
                )
                .await;
                let response = ExecuteResult {
                    identifier: input.identifier,
                    result,
                };
                write_message(&writer_for_child, &response).unwrap();
            });
            drop(reader_for_parent);
            std::process::exit(0);
        }
        Err(_) => {}
    }
}

fn encode_evm_advance(payload: Vec<u8>) -> Vec<u8> {
    sol! { interface Inputs {
        function EvmAdvance(
            uint256 chainId,
            address appContract,
            address msgSender,
            uint256 blockNumber,
            uint256 blockTimestamp,
            uint256 index,
            bytes calldata payload
        ) external;
    } };
    let call = Inputs::EvmAdvanceCall {
        chainId: U256::from(0),
        appContract: address!(),
        msgSender: address!(),
        blockNumber: U256::from(0),
        blockTimestamp: U256::from(0),
        index: U256::from(0),
        payload: payload.into(),
    };
    call.abi_encode()
}

async fn execute(
    ipfs_url: &str,
    ipfs_write_url: &str,
    payload: Option<Vec<u8>>,
    state_cid: Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    max_cycles_input: Option<u64>,
    writer_for_child: &PipeWriter,
    reader_for_parent: &PipeReader,
) -> Result<Vec<u8>, serde_error::Error> {
    let mut thread_execute: HashMap<Vec<u8>, Rc<PipeReader>> =
        HashMap::<Vec<u8>, Rc<PipeReader>>::new();
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

    // XXX this feels a bit like a layering violation
    let mut metadata = metadata.clone();
    metadata.insert(
        calculate_sha256("lambada-app".as_bytes()),
        Cid::try_from(app_cid.clone()).unwrap().to_bytes(),
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

    let mut machine: Option<Machine> = None;

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
        machine = Some(
            Machine::load(
                std::path::Path::new(&format!(
                    "/data/snapshot/{}_{}",
                    base_image,
                    Cid::try_from(app_cid.clone()).unwrap().to_string()
                )),
                RuntimeConfig {
                    skip_root_hash_check: true,
                    skip_root_hash_store: true,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let after_load_machine = SystemTime::now();
        tracing::info!(
            "It took {} milliseconds to load snapshot",
            after_load_machine
                .duration_since(before_load_machine)
                .unwrap()
                .as_millis()
        );

        machine_loaded_state = 2;
    } else if std::path::Path::new(&format!("/data/snapshot/{}_get_app", base_image)).exists() {
        // There is a snapshot of this base machine initialized and waiting to be populated with an app that'll initialize (LOAD_APP)
        while std::path::Path::new(&format!("/data/snapshot/{}_get_app.lock", base_image)).exists()
        {
            tracing::info!("waiting for {}_get_app.lock", base_image);
            thread::sleep(std::time::Duration::from_millis(500));
        }
        let before_load_machine = SystemTime::now();
        machine = Some(
            Machine::load(
                std::path::Path::new(&format!("/data/snapshot/{}_get_app", base_image).as_str()),
                RuntimeConfig {
                    skip_root_hash_check: true,
                    skip_root_hash_store: true,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        machine = Some(
            Machine::load(
                std::path::Path::new(&format!("/data/snapshot/{}", base_image).as_str()),
                RuntimeConfig {
                    skip_root_hash_check: true,
                    skip_root_hash_store: true,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
            machine = Some(
                Machine::load(
                    std::path::Path::new(&format!("/data/snapshot/{}", base_image)),
                    RuntimeConfig {
                        skip_root_hash_check: true,
                        skip_root_hash_store: true,
                        ..Default::default()
                    },
                )
                .unwrap(),
            );
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

    let mut machine = machine.unwrap();

    let mut max_cycles = u64::MAX;
    if let Some(m_cycle) = max_cycles_input {
        max_cycles = m_cycle;
    }
    loop {
        let mut interpreter_break_reason: Option<cartesi_machine::BreakReason> = None;
        let time_before_read_iflags_y = SystemTime::now();
        // Are we yielded? If not, continue machine execution
        if !machine.read_iflags_y().unwrap() {
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
            interpreter_break_reason = Some(machine.run(max_cycles).unwrap());
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

        const M16: u64 = ((1 << 16) - 1);
        const M32: u64 = ((1 << 32) - 1);
        let cmd = machine.read_htif_tohost_cmd().unwrap();
        let data = machine.read_htif_tohost_data().unwrap();
        let reason = ((data >> 32) & M16) as u16;
        let length = data & M32; // length

        tracing::info!("got reason {} cmd {} data {}", reason, cmd, length);
        match reason {
            SET_STATE_CID => {
                let cid_raw = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();
                let cid = Cid::try_from(cid_raw).unwrap();
                tracing::info!("SET_STATE_CID, ending succesfully");
                drop(machine);
                tracing::info!("SET_STATE_CID received cid {}", cid);
                return Ok(cid.to_bytes());
            }
            HTIF_YIELD_MANUAL_REASON_RX_ACCEPTED_DEF => {
                tracing::info!("HTIF_YIELD_MANUAL_REASON_RX_ACCEPTED_DEF");
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
                        match unsafe { fork() } {
                            Ok(ForkResult::Parent { child, .. }) => {}
                            Ok(ForkResult::Child) => {
                                let _ = setsid().unwrap();
                                machine
                                    .store(Path::new(&format!(
                                        "/data/snapshot/{}_{}",
                                        base_image,
                                        app_cid.clone().to_string(),
                                    )))
                                    .unwrap();
                                std::fs::remove_file(format!(
                                    "/data/snapshot/{}_{}.lock",
                                    base_image,
                                    app_cid.clone().to_string()
                                ))
                                .unwrap();
                                tracing::info!("done snapshotting app {}", app_cid.clone());
                                std::process::exit(0);
                            }
                            Err(_) => {}
                        }
                    } else {
                        tracing::info!(
                            "did not manage to create lock, snapshot might already be in progress"
                        );
                    }
                } else {
                    tracing::info!("snapshot of app already being stored or stored, skipping snapshot (lock file exists)")
                }

                let payload = payload.clone().unwrap_or_default();
                let encoded = encode_evm_advance(payload);
                tracing::info!("evm encoded {}", hex::encode(encoded.clone()));
                machine
                    .send_cmio_response(HTIF_YIELD_REASON_ADVANCE_STATE_DEF, &encoded)
                    .unwrap();
                tracing::info!("evm advance was written");
            }
            CURRENT_STATE_CID => {
                tracing::info!("CURRENT_STATE_CID");
                machine
                    .send_cmio_response(0, &state_cid.to_bytes().clone())
                    .unwrap();
                tracing::info!("current state info was written");
            }
            EXTERNALIZE_STATE => {
                tracing::info!("EXTERNALIZE_STATE");

                let memory = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();

                // XXX background this in future
                let data = Cursor::new(memory.clone());
                let time_before_putting_block = SystemTime::now();
                let _ = write_client.block_put(data).await.unwrap();
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
                machine.send_cmio_response(0, &[]).unwrap();
            }
            IPFS_GET_BLOCK => {
                tracing::info!("IPFS_GET_BLOCK");

                let memory = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();
                let cid = Cid::try_from(memory).unwrap();

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
                machine.send_cmio_response(0, &block.clone()).unwrap();
                tracing::info!("ipfs_get_block info was written");
            }
            EXCEPTION => {
                tracing::info!("EXCEPTION");
                drop(machine);
                return Err(serde_error::Error::new(&std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "exception",
                )));
            }
            METADATA => {
                tracing::info!("METADATA");
                let time_before_reading_metadata_key_length = SystemTime::now();

                let metadata_key = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();

                tracing::info!("read metadata: {:?}", hex::encode(metadata_key.clone()));

                let metadata_result = metadata.get(&metadata_key).unwrap(); // handle this a bit better
                tracing::info!("metadata len {:?}", metadata_result.len());
                let time_before_writing_metadata = SystemTime::now();
                machine
                    .send_cmio_response(0, &metadata_result.clone())
                    .unwrap();
                tracing::info!("metadata info was written");
            }
            /*SPAWN_COMPUTE => {
                let cid_bytes = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();
                let payload = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();

                let mut metadata: HashMap<String, String> = HashMap::<String, String>::new();

                metadata.insert(String::from("sequencer"), String::from("spawn"));

                let mut hasher = Sha256::new();
                hasher.update(cid_bytes.clone());
                hasher.update(payload.clone());
                let spawn_hash = hasher.finalize().to_vec();
                tracing::info!("spawning new execute thread");

                thread_execute.insert(spawn_hash, compute_output_reader.clone());
                let execute_parameter = ExecuteParameters {
                    ipfs_url: ipfs_url.to_string(),
                    ipfs_write_url: ipfs_write_url.to_string(),
                    payload: Some(payload),
                    state_cid: cid_bytes,
                    metadata,
                    max_cycles_input,
                    identifier: String::new(),
                };

                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&execute_parameter).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                write_message(&compute_writer, &execute_parameter_bytes).unwrap();
            }*/
            JOIN_COMPUTE => {
                let cid_length = u64::from_be_bytes(
                    machine
                        .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, 8)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid_bytes = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF + 8, cid_length)
                    .unwrap();

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(PMA_CMIO_TX_BUFFER_START_DEF + 8 + cid_length, 8)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let payload = machine
                    .read_memory(
                        PMA_CMIO_TX_BUFFER_START_DEF + 16 + cid_length,
                        payload_length,
                    )
                    .unwrap();

                let mut hasher = Sha256::new();
                hasher.update(cid_bytes.clone());
                hasher.update(payload.clone());
                let spawn_hash = hasher.finalize().to_vec();
                let mut execute_result = None;
                if let Some(reader) = thread_execute.remove(&spawn_hash) {
                    execute_result = Some(read_message(&reader));
                    tracing::info!("JOIN_COMPUTE execute result {:?}", execute_result);
                } else {
                    let mut metadata: HashMap<String, String> = HashMap::<String, String>::new();

                    metadata.insert(String::from("sequencer"), String::from("spawn"));

                    let execute_parameter = ExecuteParameters {
                        ipfs_url: ipfs_url.to_string(),
                        ipfs_write_url: ipfs_write_url.to_string(),
                        payload: Some(payload),
                        state_cid: cid_bytes,
                        metadata,
                        max_cycles_input,
                        identifier: rand::thread_rng()
                            .sample_iter(&Alphanumeric)
                            .take(10)
                            .map(char::from)
                            .collect(),
                    };
                    let mut execute_parameter_bytes = Vec::new();
                    let data_json = serde_json::to_string(&execute_parameter).unwrap();
                    execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                    execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                    write_message(&writer_for_child, &execute_parameter_bytes).unwrap();
                    tracing::info!("JOIN_COMPUTE waiting for result");
                    let execute_result = read_message(&reader_for_parent);
                    tracing::info!("JOIN_COMPUTE result {:?}", execute_result);
                }

                match execute_result.unwrap() {
                    Ok(cid) => {
                        machine.send_cmio_response(0, &cid).unwrap();
                    }
                    Err(_) => {
                        machine.send_cmio_response(1, &[]).unwrap();
                    }
                }
            }
            HINT => {
                let hint = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();

                let hint_str = String::from_utf8(hint).unwrap();
                if hint_str == "get_app" {
                    if !std::path::Path::new(&format!("/data/snapshot/{}_get_app", base_image))
                        .exists()
                        && !std::path::Path::new(&format!(
                            "/data/snapshot/{}_get_app.lock",
                            base_image
                        ))
                        .exists()
                        && machine_loaded_state == 0
                    {
                        if std::fs::OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create_new(true)
                            .open(format!("/data/snapshot/{}_get_app.lock", base_image))
                            .is_ok()
                        {
                            match unsafe { fork() } {
                                Ok(ForkResult::Parent { child, .. }) => {}
                                Ok(ForkResult::Child) => {
                                    let _ = setsid().unwrap();
                                    machine
                                        .store(Path::new(&format!(
                                            "/data/snapshot/{}_get_app",
                                            base_image
                                        )))
                                        .unwrap();
                                    std::fs::remove_file(format!(
                                        "/data/snapshot/{}_get_app.lock",
                                        base_image
                                    ))
                                    .unwrap();
                                    tracing::info!("done snapshotting {}_get_app", base_image);
                                    std::process::exit(0);
                                }
                                Err(_) => {}
                            }
                        }
                    } else {
                        tracing::info!(
                            "snapshot of base already exists or lock file exists, skipping"
                        );
                    }
                    machine.send_cmio_response(0, &[]).unwrap();
                }
            }
            KECCAK256_NAMESPACE => {
                let id = machine
                    .read_memory(PMA_CMIO_TX_BUFFER_START_DEF, length)
                    .unwrap();
                // XXX this id is a string, it should really be bytes
                let https = HttpsConnector::new();
                let client = Client::builder().build::<_, hyper::Body>(https);

                // XXX this is bad
                let uri: String = format!(
                    "{}/dehash/{}",
                    std::env::var("KECCAK256_SOURCE").unwrap(),
                    std::str::from_utf8(id.as_slice()).unwrap()
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
                machine.send_cmio_response(0, &body_bytes).unwrap();
            }
            _ => {
                // XXX this should be a fatal error
                tracing::info!("unknown reason {:?}", reason)
            }
        }
        // We should basically not get here in a well-behaved app, it should FINISH (accept/reject), EXCEPTION
        if matches!(
            interpreter_break_reason,
            Some(cartesi_machine::BreakReason::Halted)
        ) {
            tracing::info!("halted");
            drop(machine);
            return Err(serde_error::Error::new(&std::io::Error::new(
                std::io::ErrorKind::Other,
                "machine halted without finishing, exception",
            )));
        }
        if matches!(
            interpreter_break_reason,
            Some(cartesi_machine::BreakReason::ReachedTargetMcycle)
        ) {
            tracing::info!("reached cycles limit before completion of execution");
            drop(machine);
            return Err(serde_error::Error::new(&std::io::Error::new(
                std::io::ErrorKind::Other,
                "reached cycles limit before completion of execution",
            )));
        }
        let time_before_resetting_iflags_y = SystemTime::now();
        // After handling the operation, we clear the yield flag and loop back and continue execution of machine
        machine.reset_iflags_y().unwrap();
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
    let mut hasher = Sha3_256::new();
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
    let mut data_vector = Vec::new();
    let data_json = serde_json::to_string(&data).unwrap();
    data_vector.extend(data_json.as_bytes().len().to_le_bytes());
    data_vector.extend(data_json.as_bytes().to_vec());
    pipe.write_all(&data_vector).unwrap();
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteParameters {
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
