use async_compatibility_layer::logging::setup_backtrace;
use async_compatibility_layer::logging::setup_logging;
use async_std::task;
use celestia_rpc::{BlobClient, HeaderClient};
use celestia_types::nmt::Namespace;
use cid::Cid;
use hyper::Uri;
use hyper::{header, Body, Client, Method, Request};
use nix::libc;
use os_pipe::PipeReader;
use os_pipe::{dup_stdin, dup_stdout};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sqlite::State;
use std::env;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutorOptions {
    pub espresso_testnet_sequencer_url: String,
    pub celestia_testnet_sequencer_url: String,
    pub avail_testnet_sequencer_url: String,
    pub ipfs_url: String,
    pub ipfs_write_url: String,
    pub db_path: String,
    pub server_address: String,
    pub evm_da_url: String,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SubscribeInput {
    pub height: u64,
    pub opt: ExecutorOptions,
    pub current_cid: Vec<u8>,
    pub chain_vm_id: String,
    pub genesis_cid_text: String,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BincodedCompute {
    pub metadata: HashMap<Vec<u8>, Vec<u8>>,
    pub payload: Vec<u8>,
}
#[async_std::main]
async fn main() {
    let chain_cid = &env::args().collect::<Vec<_>>()[1];

    let my_stdout = File::create(format!("/tmp/{}-celestia-stdout.log", chain_cid))
        .expect("Failed to create stdout file");
    let my_stderr = File::create(format!("/tmp/{}-celestia-stderr.log", chain_cid))
        .expect("Failed to create stderr file");

    let stdout_fd = my_stdout.as_raw_fd();
    let stderr_fd = my_stderr.as_raw_fd();
    unsafe {
        libc::close(1);
        libc::close(2);
        libc::dup2(stdout_fd, 1);
        libc::dup2(stderr_fd, 2);
    }
    setup_logging();
    setup_backtrace();
    let stdin = dup_stdin().unwrap();

    if let Ok(parameter) = read_message(&stdin) {
        tracing::info!(" after second read");
        let time_after_execute = SystemTime::now();
        let subscribe_input = serde_json::from_slice::<SubscribeInput>(&parameter).unwrap();
        subscribe_celestia(
            subscribe_input.height,
            Cid::from_str(chain_cid).unwrap().to_bytes(),
            subscribe_input.opt,
            &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
            subscribe_input.chain_vm_id,
            subscribe_input.genesis_cid_text,
        )
        .await;
    }
}
async fn subscribe_celestia(
    current_height: u64,
    current_chain_info_cid: Vec<u8>,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    chain_vm_id: String,
    genesis_cid_text: String,
) {
    let token = match std::env::var("CELESTIA_TESTNET_NODE_AUTH_TOKEN_READ") {
        Ok(token) => token,
        Err(_) => return,
    };
    let client =
        celestia_rpc::Client::new(&opt.celestia_testnet_sequencer_url, Some(token.as_str()))
            .await
            .unwrap();
    let current_height = if current_height == 0 {
        1
    } else {
        current_height
    };
    let chain_vm_id_num: u64 = chain_vm_id.parse::<u64>().expect("VM ID as u64");
    let celestia_tx_namespace = chain_vm_id_num.to_be_bytes().to_vec();
    let mut celestia_tx_count: u64 = 0;
    match client.header_wait_for_height(current_height).await {
        Ok(extended_header) => {
            let mut state = client.header_sync_state().await.unwrap();
            while client
                .header_wait_for_height(state.height + 1)
                .await
                .is_ok()
            {
                //let chain_info_cid = Arc::clone(&current_chain_info_cid);
                if !is_chain_info_same(
                    opt.clone(),
                    *current_cid,
                    Arc::new(Mutex::new(Some(
                        Cid::try_from(current_chain_info_cid.clone()).unwrap(),
                    ))),
                )
                .await
                {
                    break;
                }

                match client
                    .blob_get_all(
                        state.height,
                        &[Namespace::new_v0(&chain_vm_id.as_bytes()).unwrap()],
                    )
                    .await
                {
                    Ok(blobs) => {
                        let connection = sqlite::Connection::open_thread_safe(format!(
                            "{}/chains/{}",
                            opt.db_path, genesis_cid_text
                        ))
                        .unwrap();
                        let mut statement = connection
                            .prepare("SELECT * FROM blocks WHERE height=?")
                            .unwrap();
                        statement.bind((1, state.height as i64)).unwrap();
                        let mut metadata: HashMap<Vec<u8>, Vec<u8>> =
                            HashMap::<Vec<u8>, Vec<u8>>::new();

                        metadata.insert(
                            calculate_sha256("sequencer".as_bytes()),
                            calculate_sha256("celestia".as_bytes()),
                        );

                        metadata.insert(
                            calculate_sha256("celestia-block-height".as_bytes()),
                            state.height.to_be_bytes().to_vec(),
                        );

                        if let Ok(statement_state) = statement.next() {
                            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
                            if statement_state == State::Done {
                                for blob in blobs {
                                    let mut tx_metadata = metadata.clone();
                                    tx_metadata.insert(
                                        calculate_sha256("celestia-tx-count".as_bytes()),
                                        celestia_tx_count.to_be_bytes().to_vec(),
                                    );
                                    tx_metadata.insert(
                                        calculate_sha256("celestia-tx-namespace".as_bytes()),
                                        celestia_tx_namespace.clone(),
                                    );

                                    handle_tx(
                                        opt.clone(),
                                        Some(blob.data),
                                        current_cid,
                                        tx_metadata,
                                        state.height,
                                        genesis_cid_text.clone(),
                                        hex::encode(extended_header.commit.block_id.hash),
                                    )
                                    .await;

                                    celestia_tx_count += 1;
                                }
                            }

                            let new_state_cid = current_cid.to_string();
                            let new_block_height = state.height;

                            let options_clone = Arc::new(opt.clone());
                            let genesis_block_cid = genesis_cid_text.clone();

                            thread::spawn(move || {
                                let runtime = tokio::runtime::Runtime::new().unwrap();
                                runtime.block_on(async {
                                    trigger_callback_for_newblock(
                                        options_clone,
                                        &genesis_block_cid,
                                        new_block_height,
                                        &new_state_cid,
                                    )
                                    .await;
                                });
                            });
                        }
                    }
                    Err(_) => {}
                }
                state = client.header_sync_state().await.unwrap();
            }
        }
        Err(e) => {
            tracing::info!("Error: {:?}", e);
        }
    }
}

async fn handle_tx(
    opt: ExecutorOptions,
    data: Option<Vec<u8>>,
    current_cid: &mut Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    height: u64,
    genesis_cid_text: String,
    block_hash: String,
) {
    let mut bincoded_compute_data: BincodedCompute = BincodedCompute {
        metadata: metadata.clone(),
        payload: vec![],
    };
    if let Some(payload) = data.clone() {
        bincoded_compute_data.payload = payload;
    }
    let req = Request::builder()
        .method("POST")
        .header("Content-Type", "application/octet-stream")
        .uri(format!(
            "http://{}/compute/{}?bincoded=true",
            opt.server_address,
            current_cid.to_string()
        ))
        .body(Body::from(
            bincode::serialize(&bincoded_compute_data).unwrap(),
        ))
        .unwrap();
    let client = hyper::Client::new();
    match client.request(req).await {
        Ok(result) => {
            let cid = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(result)
                    .await
                    .expect("/compute failed with no response")
                    .to_vec(),
            )
            .expect("/compute failed with no response");
            let cid = Cid::try_from(cid.get("cid").unwrap().as_str().unwrap()).unwrap();
            tracing::info!("old current_cid {:?}", cid.clone());
            *current_cid = cid;
            tracing::info!("resulted current_cid {:?}", current_cid.clone());
        }
        Err(e) => {
            tracing::info!("no output: {:?}", e);
        }
    }

    // XXX Is this right? Shouldn't this be after processing all tx'es?
    let connection = sqlite::Connection::open_thread_safe(format!(
        "{}/chains/{}",
        opt.db_path, genesis_cid_text
    ))
    .unwrap();

    let mut statement = connection
        .prepare(
            "INSERT INTO blocks (state_cid, height, sequencer_block_reference, finalized) VALUES (?, ?, ?, ?)",
        )
        .unwrap();
    statement
        .bind((1, &current_cid.to_bytes() as &[u8]))
        .unwrap();
    statement.bind((2, height as i64)).unwrap();
    statement.bind((3, &block_hash as &str)).unwrap();
    statement.bind((4, 1)).unwrap();
    statement.next().unwrap();
}

pub fn calculate_sha256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

async fn is_chain_info_same(
    opt: ExecutorOptions,
    current_cid: Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
) -> bool {
    let new_chain_info = get_chain_info_cid(&opt, current_cid).await;
    if new_chain_info == None {
        tracing::error!("No chain info found, leaving");
        return false;
    }
    if new_chain_info != *current_chain_info_cid.lock().unwrap() {
        *current_chain_info_cid.lock().unwrap() = new_chain_info;
        tracing::error!("No support for changing chain info yet");
        return false;
    }

    return true;
}

async fn get_chain_info_cid(opt: &ExecutorOptions, current_cid: Cid) -> Option<Cid> {
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}/api/v0/dag/resolve?arg={}/gov/{}",
            opt.ipfs_url,
            current_cid.to_string(),
            "/chain-info.json"
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let client = Arc::new(hyper::Client::new());
    match client.request(req).await {
        Ok(res) => {
            let response_cid_value = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(res).await.expect("no cid").to_vec(),
            )
            .unwrap();

            let response_cid_value = Cid::try_from(
                response_cid_value
                    .get("Cid")
                    .unwrap()
                    .get("/")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            Some(response_cid_value)
        }
        Err(_) => None,
    }
}

async fn trigger_callback_for_newblock(
    options: Arc<ExecutorOptions>,
    genesis_block_cid: &str,
    block_height: u64,
    state_cid: &str,
) {
    let connection =
        sqlite::Connection::open_thread_safe(format!("{}/subscriptions.db", options.db_path))
            .unwrap();
    let mut statement = connection
        .prepare("SELECT callback_url FROM block_callbacks WHERE genesis_block_cid = ?")
        .unwrap();
    statement.bind((1, genesis_block_cid)).unwrap();

    while let Ok(State::Row) = statement.next() {
        let callback_url = statement.read::<String, _>(0).unwrap();
        let payload = serde_json::json!({
            "appchain": genesis_block_cid,
            "block_height": block_height,
            "state_cid": state_cid
        });

        let client = Client::new();
        let req = Request::builder()
            .method(Method::POST)
            .uri(callback_url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let _ = client.request(req).await;
    }
}
pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}
