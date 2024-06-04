use async_std::task;
use es_version::SequencerVersion;
use sequencer::SeqTypes;
use std::collections::HashMap;
use std::thread;
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tide_disco::error::ServerError;
type HotShotClient = surf_disco::Client<ServerError, SequencerVersion>;
use ark_serialize::CanonicalSerialize;
use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use async_std::stream::StreamExt;
use cid::Cid;
use hotshot_query_service::availability;
use hotshot_query_service::availability::BlockQueryData;
use hotshot_query_service::availability::VidCommonQueryData;
use hotshot_query_service::types::HeightIndexed;
use hyper::Uri;
use hyper::{header, Body, Client, Method, Request};
use nix::libc;
use os_pipe::PipeReader;
use os_pipe::{dup_stdin, dup_stdout};
use polling::{Event, Events, Poller};
use sequencer::block::payload::{parse_ns_payload, NamespaceProof};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sqlite::State;
use std::env;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;
use surf_disco::Url;
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
pub struct BincodedCompute {
    pub metadata: HashMap<Vec<u8>, Vec<u8>>,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SubscribeInput {
    pub height: u64,
    pub opt: ExecutorOptions,
    pub current_cid: Vec<u8>,
    pub chain_vm_id: String,
    pub genesis_cid_text: String,
}

#[async_std::main]
async fn main() {
    let chain_cid = &env::args().collect::<Vec<_>>()[1];

    let my_stdout = File::create(format!("/tmp/{}-espresso-stdout.log", chain_cid))
        .expect("Failed to create stdout file");
    let my_stderr = File::create(format!("/tmp/{}-espresso-stderr.log", chain_cid))
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
    let poller = Poller::new().unwrap();
    let key = &stdin.as_fd().as_raw_fd();
    unsafe {
        poller
            .add(&stdin.as_fd(), Event::readable(key.clone() as usize))
            .unwrap()
    };
    let mut events = Events::new();
    loop {
        events.clear();
        poller
            .wait(&mut events, Some(Duration::from_millis(100)))
            .unwrap();

        for ev in events.iter() {
            if ev.key == key.clone() as usize {
                if let Ok(parameter) = read_message(&stdin) {
                    let time_after_execute = SystemTime::now();
                    let subscribe_input =
                        serde_json::from_slice::<SubscribeInput>(&parameter).unwrap();
                    subscribe_espresso(
                        subscribe_input.height,
                        subscribe_input.opt,
                        &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
                        Arc::new(Mutex::new(Some(Cid::from_str(chain_cid).unwrap()))),
                        subscribe_input.chain_vm_id,
                        subscribe_input.genesis_cid_text,
                    )
                    .await;
                }
            }
        }
    }
}
async fn subscribe_espresso(
    current_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
    chain_vm_id: String,
    genesis_cid_text: String,
) {
    let query_service_url = Url::parse(&opt.espresso_testnet_sequencer_url)
        .unwrap()
        .join("v0/availability")
        .unwrap();

    let hotshot = HotShotClient::new(query_service_url);
    hotshot.connect(None).await;

    let mut block_query_stream = hotshot
        .socket(&format!("stream/blocks/{}", current_height))
        .subscribe::<availability::BlockQueryData<SeqTypes>>()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    let mut vid_common = hotshot
        .socket(&format!("stream/vid/common/{}", current_height))
        .subscribe::<VidCommonQueryData<SeqTypes>>()
        .await
        .unwrap();
    let mut chain = block_query_stream.zip(vid_common).enumerate();
    while let Some((i, (block, common))) = chain.next().await {
        let block = block.unwrap();
        let common = common.unwrap();
        let chain_info_cid = Arc::clone(&current_chain_info_cid);

        if !is_chain_info_same(opt.clone(), *current_cid, chain_info_cid).await {
            return;
        }

        let block: BlockQueryData<SeqTypes> = block;

        let payload = block.payload();

        let block_timestamp: u64 = block.header().timestamp;
        let espresso_block_timestamp = block_timestamp.to_be_bytes().to_vec();

        let espresso_tx_namespace = chain_vm_id.to_string();
        let mut espresso_tx_number: u64 = 0;
        let vm_id: u64 = chain_vm_id.parse().expect("vm-id should be a valid u64");

        let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::<Vec<u8>, Vec<u8>>::new();
        metadata.insert(
            calculate_sha256("sequencer".as_bytes()),
            calculate_sha256("espresso".as_bytes()),
        );
        metadata.insert(
            calculate_sha256("espresso-block-height".as_bytes()),
            block.height().to_be_bytes().to_vec(),
        );
        metadata.insert(
            calculate_sha256("espresso-block-timestamp".as_bytes()),
            espresso_block_timestamp,
        );

        let mut bytes = Vec::new();
        block.hash().serialize_uncompressed(&mut bytes).unwrap();
        metadata.insert(
            calculate_sha256("espresso-block-hash".as_bytes()),
            bytes.clone(),
        );
        if let Some(info) = block.header().l1_finalized {
            metadata.insert(
                calculate_sha256("espresso-l1-block-height".as_bytes()),
                info.number.to_be_bytes().to_vec(),
            );
            let mut block_timestamp = vec![0; 32];
            info.timestamp.to_big_endian(&mut block_timestamp);
            metadata.insert(
                calculate_sha256("espresso-l1-block-timestamp".as_bytes()),
                block_timestamp,
            );
            metadata.insert(
                calculate_sha256("espresso-l1-block-hash".as_bytes()),
                info.hash.as_bytes().to_vec(),
            );
        }

        let height = block.height();

        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();
        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, height as i64)).unwrap();

        if let Ok(state) = statement.next() {
            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
            if state == State::Done {
                drop(statement);
                drop(connection);

                let common = common.common();

                let proof = block.payload().namespace_with_proof(
                    block.payload().get_ns_table(),
                    vm_id.into(),
                    common.clone(),
                );

                if let Some(p) = proof {
                    match p {
                        NamespaceProof::Existence {
                            ns_payload_flat, ..
                        } => {
                            let transactions = parse_ns_payload(&ns_payload_flat, vm_id.into());
                            for (_, tx) in transactions.into_iter().enumerate() {
                                let mut tx_metadata = metadata.clone();
                                tx_metadata.insert(
                                    calculate_sha256("espresso-tx-number".as_bytes()),
                                    espresso_tx_number.to_be_bytes().to_vec(),
                                );
                                tx_metadata.insert(
                                    calculate_sha256("espresso-tx-namespace".as_bytes()),
                                    espresso_tx_namespace.clone().into(),
                                );

                                tracing::info!("new tx, call handle_tx");
                                handle_tx(
                                    opt.clone(),
                                    Some(tx.payload().to_vec()),
                                    current_cid,
                                    tx_metadata,
                                    height,
                                    genesis_cid_text.clone(),
                                    hex::encode(bytes.clone()),
                                )
                                .await;

                                espresso_tx_number += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let new_state_cid = current_cid.to_string();
            let new_block_height = height;

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

pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}
