use sequencer::{L1BlockInfo, VmId};
use surf_disco::Url;
use tide_disco::error::ServerError;

type HotShotClient = surf_disco::Client<ServerError>;

use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::JsonRpcCartesiMachineClient;
use celestia_rpc::{BlobClient, HeaderClient};
use celestia_types::nmt::Namespace;
use cid::Cid;
use ethers::prelude::*;
use futures_util::TryStreamExt;
use hotshot_query_service::availability::BlockQueryData;
use hyper::Request;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use jf_primitives::merkle_tree::namespaced_merkle_tree::NamespaceProof;
use sequencer::SeqTypes;
use sqlite::State;
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
#[derive(Clone, Debug)]
pub struct ExecutorOptions {
    pub espresso_testnet_sequencer_url: String,
    pub celestia_testnet_sequencer_url: String,
    pub ipfs_url: String,
    pub ipfs_write_url: String,
    pub db_path: String,
    pub base_cartesi_machine_path: String,
}

pub async fn subscribe(opt: ExecutorOptions, cartesi_machine_url: String, appchain: Cid) {
    let mut chain_info_path = "/app";

    tracing::info!("starting subscribe() of {:?}", appchain.to_string());

    let mut current_cid = appchain.clone();
    let genesis_cid_text = current_cid.to_string();
    let mut current_height: u64 = u64::MAX;
    let espresso_testnet_sequencer_url = opt.espresso_testnet_sequencer_url.clone();
    let celestia_testnet_sequencer_url = opt.celestia_testnet_sequencer_url.clone();
    let machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();
    // Make sure database is set up
    let ipfs_client = IpfsClient::from_str(&opt.ipfs_url).unwrap();
    match ipfs_client
        .files_stat(&format!(
            "/{}{}",
            current_cid.to_string(),
            "/gov/chain-info.json"
        ))
        .await
    {
        Ok(_) => {
            chain_info_path = "/gov";
            tracing::info!("deprecated behaviour: directory /app/chain-info.json was moved to /gov/chain-info.json");
        }
        Err(_) => {}
    };
    {
        let connection =
            sqlite::Connection::open_thread_safe(format!("{}/{}", opt.db_path, genesis_cid_text))
                .unwrap();
        let query = "
    CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
    height INTEGER NOT NULL);
";
        connection.execute(query).unwrap();

        let chain_info = ipfs_client
            .cat(&format!(
                "{}{}/chain-info.json",
                current_cid.to_string(),
                chain_info_path.to_string()
            ))
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

        let mut initial_block_height: i64 = starting_block_height as i64 - 1;
        if initial_block_height < 0 {
            initial_block_height = 0
        };

        let mut statement = connection
            .prepare("SELECT * FROM blocks ORDER BY height DESC LIMIT 1")
            .unwrap();

        if let Ok(State::Row) = statement.next() {
            let height = statement.read::<i64, _>("height").unwrap() as u64;
            let cid = Cid::try_from(statement.read::<Vec<u8>, _>("state_cid").unwrap()).unwrap();
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
    // Set what our current chain info is, so we can notice later on if it changes
    let current_chain_info_cid: Arc<Mutex<Option<Cid>>> = Arc::new(Mutex::new(
        get_chain_info_cid(&opt, current_cid, chain_info_path).await,
    ));
    if *current_chain_info_cid.lock().unwrap() == None {
        tracing::debug!("not chain info found, leaving");
        return;
    }
    tracing::info!("starting subscribe loop of {:?}", appchain.to_string());
    loop {
        // Set up subscription: read what sequencer and (if we don't know it already)
        let chain_info = ipfs_client
            .cat(&format!(
                "{}{}/chain-info.json",
                current_cid.to_string(),
                chain_info_path.to_string()
            ))
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

        let chain_vm_id: u64 = chain_info
            .get("sequencer")
            .unwrap()
            .get("vm-id")
            .unwrap()
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();

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
                    espresso_testnet_sequencer_url.as_str(),
                    current_height,
                    opt.clone(),
                    &mut current_cid,
                    chain_info_cid,
                    chain_vm_id,
                    genesis_cid_text.clone(),
                    chain_info_path,
                )
                .await;
            }
            "celestia" => {
                let chain_info_cid = Arc::clone(&current_chain_info_cid);

                subscribe_celestia(
                    &machine,
                    celestia_testnet_sequencer_url.clone(),
                    current_height,
                    chain_info_cid,
                    opt.clone(),
                    &mut current_cid,
                    chain_vm_id,
                    genesis_cid_text.clone(),
                    chain_info_path,
                )
                .await;
            }
            _ => {
                tracing::info!("unknown sequencer type");
            }
        }
    }
}

async fn get_chain_info_cid(
    opt: &ExecutorOptions,
    current_cid: Cid,
    chain_info_path: &str,
) -> Option<Cid> {
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}/api/v0/dag/resolve?arg={}{}{}",
            opt.ipfs_url,
            current_cid.to_string(),
            chain_info_path,
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

async fn handle_tx(
    machine: &JsonRpcCartesiMachineClient,
    opt: ExecutorOptions,
    data: Option<Vec<u8>>,
    current_cid: &mut Cid,
    block_info: &L1BlockInfo,
    height: u64,
    genesis_cid_text: String,
    app_path: Option<&str>,
) {
    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());

    let time_before_execute = SystemTime::now();

    let result = execute(
        forked_machine_url,
        &opt.base_cartesi_machine_path,
        opt.ipfs_url.as_str(),
        opt.ipfs_write_url.as_str(),
        data,
        current_cid.clone(),
        block_info,
        None,
        app_path,
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
    if let Ok(cid) = result {
        tracing::info!(
            "old current_cid {:?}",
            Cid::try_from(current_cid.clone()).unwrap().to_string()
        );
        *current_cid = cid;
        tracing::info!(
            "resulted current_cid {:?}",
            Cid::try_from(current_cid.clone()).unwrap().to_string()
        );
    } else {
        tracing::info!("EXECUTE FAILED: reusing current_cid, as transaction failed");
    }
    // XXX Is this right? Shouldn't this be after processing all tx'es?
    let connection =
        sqlite::Connection::open_thread_safe(format!("{}/{}", opt.db_path, genesis_cid_text))
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
    app_path: &str,
) -> bool {
    let new_chain_info = get_chain_info_cid(&opt, current_cid, app_path).await;
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
    sequencer_url: &str,
    current_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
    chain_vm_id: u64,
    genesis_cid_text: String,
    chain_info_path: &str,
) {
    let query_service_url = Url::parse(&sequencer_url)
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

                if !is_chain_info_same(opt.clone(), *current_cid, chain_info_cid, chain_info_path)
                    .await
                {
                    return;
                }

                let block: BlockQueryData<SeqTypes> = block;
                let payload = block.payload();

                let proof = payload.get_namespace_proof(VmId::from(chain_vm_id));
                let transactions = proof.get_namespace_leaves();

                let mut block_info: L1BlockInfo = L1BlockInfo {
                    number: 0,
                    timestamp: U256([0; 4]),
                    hash: H256([0; 32]),
                };

                if let Some(info) = block.header().l1_finalized {
                    block_info = info;
                }

                let height = block.height();

                let connection = sqlite::Connection::open_thread_safe(format!(
                    "{}/{}",
                    opt.db_path, genesis_cid_text
                ))
                .unwrap();
                let mut statement = connection
                    .prepare("SELECT * FROM blocks WHERE height=?")
                    .unwrap();
                statement.bind((1, height as i64)).unwrap();
                let mut app_path = None;
                if chain_info_path.eq("/gov") {
                    app_path = Some(chain_info_path);
                }
                if let Ok(state) = statement.next() {
                    // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
                    if state == State::Done {
                        for (_, tx) in transactions.into_iter().enumerate() {
                            tracing::info!("tx.payload().len: {:?}", tx.payload().len());

                            handle_tx(
                                &machine,
                                opt.clone(),
                                Some(tx.payload().to_vec()),
                                current_cid,
                                &block_info,
                                height,
                                genesis_cid_text.clone(),
                                app_path,
                            )
                            .await;
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!("Error in HotShot block stream, retrying: {err}");
                continue;
            }
        };
    }
    tracing::info!("finished");
}
async fn subscribe_celestia(
    machine: &JsonRpcCartesiMachineClient,
    sequencer_url: String,
    current_height: u64,
    current_chain_info_cid: Arc<Mutex<Option<cid::CidGeneric<64>>>>,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    chain_vm_id: u64,
    genesis_cid_text: String,
    chain_info_path: &str,
) {
    let token = match std::env::var("CELESTIA_TESTNET_NODE_AUTH_TOKEN_READ") {
        Ok(token) => token,
        Err(_) => return,
    };
    let client = celestia_rpc::Client::new(sequencer_url.as_str(), Some(token.as_str()))
        .await
        .unwrap();
    let current_height = if current_height == 0 {
        1
    } else {
        current_height
    };
    match client.header_wait_for_height(current_height).await {
        Ok(_) => {
            let mut state = client.header_sync_state().await.unwrap();
            while client
                .header_wait_for_height(state.height + 1)
                .await
                .is_ok()
            {
                let chain_info_cid = Arc::clone(&current_chain_info_cid);
                if !is_chain_info_same(opt.clone(), *current_cid, chain_info_cid, chain_info_path)
                    .await
                {
                    break;
                }
                let block_info: &L1BlockInfo = &L1BlockInfo {
                    number: 0,
                    timestamp: U256([0; 4]),
                    hash: H256([0; 32]),
                };
                match client
                    .blob_get_all(
                        state.height,
                        &[Namespace::new_v0(&chain_vm_id.to_be_bytes()).unwrap()],
                    )
                    .await
                {
                    Ok(blobs) => {
                        let connection = sqlite::Connection::open_thread_safe(format!(
                            "{}/{}",
                            opt.db_path, genesis_cid_text
                        ))
                        .unwrap();
                        let mut statement = connection
                            .prepare("SELECT * FROM blocks WHERE height=?")
                            .unwrap();
                        statement.bind((1, state.height as i64)).unwrap();
                        let mut app_path = None;
                        if chain_info_path.eq("/gov") {
                            app_path = Some(chain_info_path);
                        }
                        if let Ok(statement_state) = statement.next() {
                            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
                            if statement_state == State::Done {
                                for blob in blobs {
                                    tracing::info!("new blob {:?}", blob);
                                    handle_tx(
                                        &machine,
                                        opt.clone(),
                                        Some(blob.data),
                                        current_cid,
                                        block_info,
                                        state.height,
                                        genesis_cid_text.clone(),
                                        app_path,
                                    )
                                    .await;
                                }
                            }
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
