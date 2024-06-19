use anyhow::Result;
use async_compatibility_layer::logging::setup_backtrace;
use async_compatibility_layer::logging::setup_logging;
use avail_subxt::api::vector::calls::types::FailedSendMessageTxs;
use avail_subxt::{AvailClient, Opts};

use cid::Cid;
use core::mem::swap;
use futures::future::{join_all, TryFutureExt};
use hyper::{header, Body, Client, Method, Request};
use nix::libc;
use os_pipe::PipeReader;
use os_pipe::{dup_stdin, dup_stdout};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sqlite::State;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use structopt::StructOpt;
//use subxt::{config::Header as XtHeader, utils::H256};
use avail_subxt::api::data_availability::calls::types::SubmitData;
use avail_subxt::primitives::CheckAppId;
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

    let my_stdout = File::create(format!("/tmp/{}-avail-stdout.log", chain_cid))
        .expect("Failed to create stdout file");
    let my_stderr = File::create(format!("/tmp/{}-avail-stderr.log", chain_cid))
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
        let time_after_execute = SystemTime::now();
        let subscribe_input = serde_json::from_slice::<SubscribeInput>(&parameter).unwrap();
        subscribe_avail(
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

pub async fn subscribe_avail(
    current_height: u64,
    current_chain_info_cid: Vec<u8>,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    chain_vm_id: String,
    genesis_cid_text: String,
) {
    let client = AvailClient::new(&opt.avail_testnet_sequencer_url)
        .await
        .unwrap();

    let chain_vm_id_num: u64 = chain_vm_id.parse::<u64>().expect("VM ID as u64");
    let avail_tx_namespace = chain_vm_id_num.to_be_bytes().to_vec();
    let mut avail_tx_count: u64 = 0;

    let mut best_stream = client.blocks().subscribe_finalized().await.unwrap();
    while let Some(block) = best_stream.next().await {
        let block = block.unwrap();

        let block_number = block.header().number;
        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();
        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, block_number as i64)).unwrap();
        let block_hash = block.hash();
        let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        metadata.insert(b"sequencer".to_vec(), b"avail".to_vec());
        metadata.insert(
            b"avail-block-height".to_vec(),
            block_number.to_string().as_bytes().to_vec(),
        );
        metadata.insert(
            b"avail-block-hash".to_vec(),
            format!("{:?}", block_hash).as_bytes().to_vec(),
        );
        if let Ok(state) = statement.next() {
            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
            if state == State::Done {
                let extrinsics = block.extrinsics().await.unwrap();
                let da_submissions = extrinsics.find::<SubmitData>();
                for da_submission in da_submissions {
                    let da_submission = da_submission.unwrap();

                    let tx_data = da_submission.value.data.0.as_slice();

                    let app_id = da_submission
                        .details
                        .signed_extensions()
                        .unwrap()
                        .find::<CheckAppId>()
                        .unwrap()
                        .unwrap();

                    if app_id == parity_scale_codec::Compact(chain_vm_id_num.try_into().unwrap()) {
                        let mut tx_metadata = metadata.clone();

                        tx_metadata.insert(
                            calculate_sha256("avail-tx-count".as_bytes()),
                            avail_tx_count.to_be_bytes().to_vec(),
                        );
                        tx_metadata.insert(
                            calculate_sha256("avail-tx-namespace".as_bytes()),
                            avail_tx_namespace.clone(),
                        );

                        handle_tx(
                            opt.clone(),
                            Some(tx_data.to_vec()),
                            current_cid,
                            tx_metadata,
                            current_height,
                            genesis_cid_text.clone(),
                            hex::encode(block_hash),
                        )
                        .await;

                        avail_tx_count += 1;
                    }
                }
            }
        }
        let new_state_cid = current_cid.to_string();
        let new_block_height = current_height;

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
    let time_before_execute = SystemTime::now();
    tracing::info!("current_cid {:?}", current_cid);

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
pub struct SubscribeResponse {
    finished: bool,
}
