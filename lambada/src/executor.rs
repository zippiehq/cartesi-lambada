use ark_serialize::CanonicalSerialize;
use ethers::prelude::*;
use hyper::client::HttpConnector;
use serde_json::json;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use surf_disco::Url;
use tide_disco::error::ServerError;

type HotShotClient = surf_disco::Client<ServerError>;

use async_std::stream::StreamExt;
use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::JsonRpcCartesiMachineClient;
use celestia_rpc::{BlobClient, HeaderClient};
use celestia_types::nmt::Namespace;
use cid::Cid;
use ethers::types::{BlockId, BlockNumber, H256, U256};
use futures_util::TryStreamExt;
use hotshot_query_service::availability::BlockQueryData;
use hyper::Uri;
use hyper::{header, Body, Client, Method, Request};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use jf_primitives::merkle_tree::namespaced_merkle_tree::NamespaceProof;
use sequencer::{SeqTypes, VmId};
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::time::sleep;
use crate::eip4844::TxEip4844;

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
#[derive(Clone, Debug)]
pub struct ExecutorOptions {
    pub espresso_testnet_sequencer_url: String,
    pub celestia_testnet_sequencer_url: String,
    pub ipfs_url: String,
    pub ipfs_write_url: String,
    pub db_path: String,
    pub callback_node_info: Option<(Uri, String, String)>,
    pub evm_da_url: String,
}

pub async fn subscribe(
    opt: ExecutorOptions,
    cartesi_machine_url: String,
    appchain: Cid,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
) {
    tracing::info!("starting subscribe() of {:?}", appchain.to_string());
    let mut current_cid = appchain.clone();
    let genesis_cid_text = current_cid.to_string();
    let mut current_height: u64 = u64::MAX;
    let machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();
    let ipfs_client = IpfsClient::from_str(&opt.ipfs_url).unwrap();

    // Set what our current chain info is, so we can notice later on if it changes
    let current_chain_info_cid: Arc<Mutex<Option<Cid>>> =
        Arc::new(Mutex::new(get_chain_info_cid(&opt, current_cid).await));
    if *current_chain_info_cid.lock().unwrap() == None {
        tracing::debug!("not chain info found, leaving");
        return;
    }
    tracing::info!("starting subscribe loop of {:?}", appchain.to_string());
    let mut chain_info = ipfs_client
        .cat(&format!("{}/gov/chain-info.json", current_cid.to_string()))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();

    // enter subscription loop
    loop {
        {
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/chains/{}",
                opt.db_path, genesis_cid_text
            ))
            .unwrap();
            let query = "
        CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
        height INTEGER NOT NULL);
    ";
            connection.execute(query).unwrap();

            let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
                .expect("error reading chain-info.json file");

            let starting_block_height: u64 = chain_info
                .get("sequencer")
                .unwrap()
                .get("height")
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let mut initial_block_height: i64 = starting_block_height as i64 - 1;
            if initial_block_height < 0 {
                initial_block_height = 0
            };

            let mut statement = connection
                .prepare("SELECT * FROM blocks ORDER BY height DESC LIMIT 1")
                .unwrap();

            if let Ok(State::Row) = statement.next() {
                let height = statement.read::<i64, _>("height").unwrap() as u64;
                let cid =
                    Cid::try_from(statement.read::<Vec<u8>, _>("state_cid").unwrap()).unwrap();
                tracing::info!(
                    "persisted state of chain {:?} is height {:?} = CID {:?}",
                    genesis_cid_text,
                    height,
                    cid.to_string()
                );
                current_cid = cid;
                current_height = height;
            } else {
                tracing::info!("new chain, not persisted: {:?}", genesis_cid_text);
                let mut statement = connection
                    .prepare("INSERT INTO blocks (state_cid, height) VALUES (?, ?)")
                    .unwrap();
                statement
                    .bind((1, &current_cid.to_bytes() as &[u8]))
                    .unwrap();
                statement.bind((2, initial_block_height as i64)).unwrap();
                statement.next().unwrap();
            }
        }
        // Set up subscription: read what sequencer and (if we don't know it already)
        chain_info = ipfs_client
            .cat(&format!("{}/gov/chain-info.json", current_cid.to_string()))
            .map_ok(|chunk| chunk.to_vec())
            .try_concat()
            .await
            .unwrap();

        tracing::info!(
            "chain info {}",
            String::from_utf8(chain_info.clone()).unwrap()
        );

        let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
            .expect("error reading chain-info.json file");

        let starting_block_height: u64 = chain_info
            .get("sequencer")
            .unwrap()
            .get("height")
            .unwrap()
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();

        let chain_vm_id = chain_info
            .get("sequencer")
            .unwrap()
            .get("vm-id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        if current_height == u64::MAX {
            current_height = starting_block_height;
        }

        if current_height < starting_block_height {
            tracing::error!("Current height less than starting block height in chain info, should not be possible");
            return;
        }

        tracing::info!("iterating through blocks from height {:?}", current_height);

        let r#type: String = chain_info
            .get("sequencer")
            .unwrap()
            .get("type")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        tracing::info!("chain type: {:?}", r#type);
        match r#type.as_str() {
            "espresso" => {
                let chain_info_cid = Arc::clone(&current_chain_info_cid);

                subscribe_espresso(
                    &machine,
                    current_height,
                    opt.clone(),
                    &mut current_cid,
                    chain_info_cid,
                    chain_vm_id,
                    genesis_cid_text.clone(),
                    rx.clone(),
                )
                .await;
            }
            "celestia" => {
                let chain_info_cid = Arc::clone(&current_chain_info_cid);

                subscribe_celestia(
                    &machine,
                    current_height,
                    chain_info_cid,
                    opt.clone(),
                    &mut current_cid,
                    chain_vm_id,
                    genesis_cid_text.clone(),
                    rx.clone(),
                )
                .await;
            }

            "evm-da" => {
                subscribe_evm_da(
                    &machine,
                    starting_block_height,
                    opt.clone(),
                    &mut current_cid,
                    genesis_cid_text.clone(), // recheck
                    rx.clone(),
                    chain_vm_id,
                )
                .await;
            }

            "evm-blocks" => {
                subscribe_evm_blocks(
                    &machine,
                    starting_block_height,
                    opt.clone(),
                    &mut current_cid,
                    genesis_cid_text.clone(),
                    rx.clone(),
                )
                .await;
            }
            _ => tracing::info!("Unknown sequencer type: {}", r#type),
        }
    }
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

async fn handle_tx(
    machine: &JsonRpcCartesiMachineClient,
    opt: ExecutorOptions,
    data: Option<Vec<u8>>,
    current_cid: &mut Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    height: u64,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
) {
    let mut bincoded_compute_data: BincodedCompute = BincodedCompute {
        metadata: metadata.clone(),
        payload: vec![],
    };
    if let Some(payload) = data.clone() {
        bincoded_compute_data.payload = payload;
    }
    if let Some(callback_node_info) = opt.callback_node_info {
        let req = Request::builder()
            .method("POST")
            .header("Content-Type", "application/octet-stream")
            .uri(format!(
                "{}compute_with_callback/{}?callback=http://{}/return_callback/{}&bincoded=true",
                callback_node_info.0, callback_node_info.1, callback_node_info.2, height
            ))
            .body(Body::from(
                bincode::serialize(&bincoded_compute_data).unwrap(),
            ))
            .unwrap();
        let client = hyper::Client::new();
        match client.request(req).await {
            Ok(_) => {}
            Err(e) => {
                tracing::info!("EXECUTE FAILED: {:?}", e);
            }
        }

        tracing::info!("wait to tx be send to rx");
        let id: Result<(u64, Option<String>), std::sync::mpsc::RecvError> =
            rx.unwrap().lock().await.recv();
        if let Ok(new_state_data) = id {
            if let Some(new_cid) = new_state_data.1 {
                let resulted_cid = Cid::try_from(new_cid.clone()).unwrap();
                *current_cid = resulted_cid;
            }
        }
    } else {
        let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());

        let time_before_execute = SystemTime::now();

        let result = execute(
            forked_machine_url,
            opt.ipfs_url.as_str(),
            opt.ipfs_write_url.as_str(),
            data,
            current_cid.clone(),
            metadata.clone(),
            None,
        )
        .await;
        let time_after_execute = SystemTime::now();

        tracing::info!(
            "executing time {}",
            time_after_execute
                .duration_since(time_before_execute)
                .unwrap()
                .as_millis()
        );

        match result {
            Ok(resulted_cid) => {
                tracing::info!(
                    "old current_cid {:?}",
                    Cid::try_from(current_cid.clone()).unwrap().to_string()
                );
                *current_cid = resulted_cid;
                tracing::info!(
                    "resulted current_cid {:?}",
                    Cid::try_from(current_cid.clone()).unwrap().to_string()
                );
            }
            Err(_) => {
                tracing::info!("EXECUTE FAILED: reusing current_cid, as transaction failed");
            }
        }
    }

    // XXX Is this right? Shouldn't this be after processing all tx'es?
    let connection = sqlite::Connection::open_thread_safe(format!(
        "{}/chains/{}",
        opt.db_path, genesis_cid_text
    ))
    .unwrap();

    let mut statement = connection
        .prepare("INSERT INTO blocks (state_cid, height) VALUES (?, ?)")
        .unwrap();
    statement
        .bind((1, &current_cid.to_bytes() as &[u8]))
        .unwrap();
    statement.bind((2, height as i64)).unwrap();
    statement.next().unwrap();
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

async fn subscribe_espresso(
    machine: &JsonRpcCartesiMachineClient,
    current_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
    chain_vm_id: String,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
) {
    let query_service_url = Url::parse(&opt.espresso_testnet_sequencer_url)
        .unwrap()
        .join("availability")
        .unwrap();

    let hotshot = HotShotClient::new(query_service_url);
    hotshot.connect(None).await;

    let mut block_query_stream = hotshot
        .socket(format!("stream/blocks/{}", current_height).as_str())
        .subscribe()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    while let Some(block_data) = block_query_stream.next().await {
        match block_data {
            Ok(block) => {
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
                let proof = payload.get_namespace_proof(VmId::from(vm_id));
                let transactions = proof.get_namespace_leaves();

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
                metadata.insert(calculate_sha256("espresso-block-hash".as_bytes()), bytes);
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

                            tracing::info!("tx.payload().len: {:?}", tx.payload().len());

                            handle_tx(
                                &machine,
                                opt.clone(),
                                Some(tx.payload().to_vec()),
                                current_cid,
                                tx_metadata,
                                height,
                                genesis_cid_text.clone(),
                                rx.clone(),
                            )
                            .await;

                            espresso_tx_number += 1;
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
            Err(err) => {
                tracing::error!("Error in HotShot block stream, retrying: {err}");
                continue;
            }
        };
    }
}
async fn subscribe_celestia(
    machine: &JsonRpcCartesiMachineClient,
    current_height: u64,
    current_chain_info_cid: Arc<Mutex<Option<cid::CidGeneric<64>>>>,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    chain_vm_id: String,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
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
    let celestia_tx_count: u64 = 0;
    match client.header_wait_for_height(current_height).await {
        Ok(_) => {
            let mut state = client.header_sync_state().await.unwrap();
            while client
                .header_wait_for_height(state.height + 1)
                .await
                .is_ok()
            {
                let chain_info_cid = Arc::clone(&current_chain_info_cid);
                if !is_chain_info_same(opt.clone(), *current_cid, chain_info_cid).await {
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
                                        calculate_sha256("ecelestia-tx-count".as_bytes()),
                                        celestia_tx_count.to_be_bytes().to_vec(),
                                    );
                                    tx_metadata.insert(
                                        calculate_sha256("celestia-tx-namespace".as_bytes()),
                                        celestia_tx_namespace.clone(),
                                    );
                                    tracing::info!("new blob {:?}", blob);
                                    handle_tx(
                                        &machine,
                                        opt.clone(),
                                        Some(blob.data),
                                        current_cid,
                                        tx_metadata,
                                        state.height,
                                        genesis_cid_text.clone(),
                                        rx.clone(),
                                    )
                                    .await;
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
async fn subscribe_evm_da(
    machine: &JsonRpcCartesiMachineClient,
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
    chain_vm_id: String,
) {
    let eth_rpc_url = opt.evm_da_url.clone();
    let eth_client = Arc::new(
        ethers::providers::Provider::<ethers::providers::Http>::try_from(&eth_rpc_url)
            .expect("Could not instantiate Ethereum HTTP Provider"),
    );
    let namespace = chain_vm_id;
    let namespace_address =
        ethers::types::Address::from_str(&namespace).expect("Invalid namespace address");
    let mut current_height = starting_block_height;

    while current_height < u64::MAX {
        let latest_block = eth_client
            .get_block_number()
            .await
            .expect("Failed to fetch the latest block number");
        tracing::info!(
            "EVM-DA: latest block {} current height {}",
            latest_block,
            current_height
        );

        if latest_block < current_height.into() {
            tracing::info!(
                "Waiting for block number {} to be available. Current latest block number: {}",
                current_height,
                latest_block
            );
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        let block = eth_client
            .get_block_with_txs(BlockId::Number(BlockNumber::Number(current_height.into())))
            .await
            .expect("Failed to fetch block")
            .expect("Block not found");

        for tx in &block.transactions {
            if let Some(to_address) = tx.to {
                if to_address == namespace_address {
                    let call_data = tx.input.clone();
                    let tx_hash = tx.hash;
                    tracing::info!("tx {:?}", tx);
                    let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
                    metadata.insert(b"sequencer".to_vec(), b"evm-da".to_vec());
                    metadata.insert(
                        b"ethereum-block-height".to_vec(),
                        current_height.to_string().as_bytes().to_vec(),
                    );
                    metadata.insert(
                        b"ethereum-block-hash".to_vec(),
                        format!("{:?}", block.hash).as_bytes().to_vec(),
                    );
                    metadata.insert(
                        b"ethereum-tx-hash".to_vec(),
                        format!("{:?}", tx_hash).as_bytes().to_vec(),
                    );

                    handle_tx(
                        machine,
                        opt.clone(),
                        Some(call_data.to_vec()),
                        current_cid,
                        metadata,
                        current_height,
                        genesis_cid_text.clone(),
                        rx.clone(),
                    )
                    .await;
                }
            }
        }

        current_height += 1;
    }
}

async fn subscribe_evm_blocks(
    machine: &JsonRpcCartesiMachineClient,
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
) {
    let eth_rpc_url = opt.evm_da_url.clone();
    let eth_client = Arc::new(
        ethers::providers::Provider::<ethers::providers::Http>::try_from(&eth_rpc_url)
            .expect("Could not instantiate Ethereum HTTP Provider"),
    );
    let mut current_height = starting_block_height;
    while current_height < u64::MAX {
        let latest_block = eth_client
            .get_block_number()
            .await
            .expect("Failed to fetch the latest block number");
        tracing::info!(
            "EVM-BLOCKS: latest block {} current height {}",
            latest_block,
            current_height
        );

        if latest_block < current_height.into() {
            tracing::info!(
                "Waiting for block number {} to be available. Current latest block number: {}",
                current_height,
                latest_block
            );
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        let block = eth_client
            .get_block(BlockId::Number(BlockNumber::Number(current_height.into())))
            .await
            .expect("Failed to fetch block")
            .expect("Block not found");

        tracing::info!("Processing block at height {}", current_height);
        let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        metadata.insert(b"sequencer".to_vec(), b"evm-blocks".to_vec());
        metadata.insert(
            b"ethereum-block-height".to_vec(),
            current_height.to_string().as_bytes().to_vec(),
        );
        metadata.insert(
            b"ethereum-block-hash".to_vec(),
            format!("{:?}", block.hash).as_bytes().to_vec(),
        );
        handle_tx(
            machine,
            opt.clone(),
            Some(Vec::new()),
            current_cid,
            metadata,
            current_height,
            genesis_cid_text.clone(),
            rx.clone(),
        )
        .await;

        current_height += 1;
    }
}
async fn subscribe_evm_eip4844(
    machine: &JsonRpcCartesiMachineClient,
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
    rx: Option<Arc<async_std::sync::Mutex<Receiver<(u64, Option<String>)>>>>,
) {
    let eth_rpc_url = opt.evm_da_url.clone();
    let eth_client = Arc::new(
        Provider::<Http>::try_from(eth_rpc_url)
            .expect("Could not instantiate Ethereum HTTP Provider"),
    );

    let mut current_height = starting_block_height;

    while current_height < u64::MAX {
        let latest_block = eth_client.get_block_number().await.expect("Failed to fetch the latest block number");

        if latest_block < current_height.into() {
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        let block = eth_client
            .get_block_with_txs(BlockId::Number(BlockNumber::Number(current_height.into())))
            .await
            .expect("Failed to fetch block")
            .expect("Block not found");

        for tx in &block.transactions {
            let tx_bytes = tx.rlp();
            match TxEip4844::decode_enveloped(&mut &tx_bytes[..]) {
                Ok(eip4844_tx) => {
                    println!("Detected an EIP-4844 transaction: {:?}", eip4844_tx);
                },
                Err(_) => {
                    println!("Failed to decode EIP-4844 transaction: {:?}", tx_bytes);
                }
            }
        }

        current_height += 1;
    }
}

pub fn calculate_sha256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BincodedCompute {
    pub metadata: HashMap<Vec<u8>, Vec<u8>>,
    pub payload: Vec<u8>,
}
