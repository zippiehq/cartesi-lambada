use hyper::client::conn::Connection;
use sequencer::{L1BlockInfo, VmId};
use surf_disco::Url;
use tide_disco::{error::ServerError, Error};

type HotShotClient = surf_disco::Client<ServerError>;

use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use ethers::prelude::*;
use futures_util::TryStreamExt;
use hotshot_query_service::availability::BlockQueryData;
use hyper::Request;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use jf_primitives::merkle_tree::namespaced_merkle_tree::NamespaceProof;
use sequencer::SeqTypes;
use sqlite::State;
use std::{time::{Duration, SystemTime}, sync::Arc};

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
#[derive(Clone, Debug)]
pub struct ExecutorOptions {
    pub sequencer_url: Url,
    pub ipfs_url: Url,
    pub db_path: String,
    pub base_cartesi_machine_path: String,
}

pub async fn subscribe(opt: ExecutorOptions, cartesi_machine_url: String, appchain: Cid) {
    let mut current_cid = appchain.clone();
    let genesis_cid_text = current_cid.to_string();
    let mut current_height: u64 = u64::MAX;
    let sequencer_url = opt.sequencer_url.clone();

    // Make sure database is set up
    let connection = sqlite::open(format!("{}/{}", opt.db_path, genesis_cid_text)).unwrap();
    let query = "
    CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
    height INTEGER NOT NULL);
";
    connection.execute(query).unwrap();

    let query_service_url = sequencer_url.join("availability").unwrap();

    let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();

    // Get the latest CID and height if one exists
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
    }

    let ipfs_client = IpfsClient::from_str(opt.ipfs_url.as_str()).unwrap();
    // Set what our current chain info is, so we can notice later on if it changes
    let mut current_chain_info_cid: Option<Cid> = get_chain_info_cid(&opt, current_cid).await;
    if current_chain_info_cid == None {
        tracing::debug!("not chain info found, leaving");
        return;
    }
    loop {
        // Set up subscription: read what sequencer and (if we don't know it already)
        let chain_info = ipfs_client
            .cat(&(current_cid.to_string() + "/app/chain-info.json"))
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

        let hotshot = HotShotClient::new(query_service_url.clone());
        hotshot.connect(None).await;

        let mut block_query_stream = hotshot
            .socket(format!("stream/blocks/{}", current_height).as_str())
            .subscribe()
            .await
            .expect("Unable to subscribe to HotShot block stream");

        let mut hash: Vec<u8> = vec![0; 32];
        let exception_err = std::io::Error::new(std::io::ErrorKind::Other, "exception");
        tracing::info!("iterating through blocks from height {:?}", current_height);

        while let Some(block_data) = block_query_stream.next().await {
            match block_data {
                Ok(block) => {
                    let new_chain_info = get_chain_info_cid(&opt, current_cid)
                        .await;
                    if new_chain_info == None {
                        tracing::error!("No chain info found, leaving");
                        return;
                    }
                    if new_chain_info != current_chain_info_cid {
                        current_chain_info_cid = new_chain_info;
                        tracing::error!("No support for changing chain info yet");
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
                    let timestamp = block.header().timestamp as u64;

                    hash = block.hash().into_bits().into_vec();

                    if transactions.len() != 0 {
                        hash = block.hash().into_bits().into_vec();
                    }
                    let mut statement = connection
                        .prepare("SELECT * FROM blocks WHERE height=?")
                        .unwrap();
                    statement.bind((1, height as i64)).unwrap();

                    if let Ok(state) = statement.next() {
                        // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
                        if state == State::Done {
                            for (index, tx) in transactions.into_iter().enumerate() {
                                // don't need this check anymore?
                                if u64::from(tx.vm()) as u64 == chain_vm_id {
                                    tracing::info!("found tx for our vm id");
                                    tracing::info!("tx.payload().len: {:?}", tx.payload().len());

                                    let forked_machine_url =
                                        format!("http://{}", machine.fork().await.unwrap());

                                    let time_before_execute = SystemTime::now();

                                    let result = execute(
                                        forked_machine_url,
                                        &opt.base_cartesi_machine_path,
                                        &opt.ipfs_url.to_string(),
                                        tx.payload().to_vec(),
                                        current_cid.clone(),
                                        &block_info,
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
                                        current_cid = cid;
                                        tracing::info!(
                                            "resulted current_cid {:?}",
                                            Cid::try_from(current_cid.clone()).unwrap().to_string()
                                        );
                                    } else {
                                        //TODO
                                        panic!("execute failed");
                                    }
                                    let mut statement = connection
                                        .prepare(
                                            "INSERT INTO blocks (state_cid, height) VALUES (?, ?)",
                                        )
                                        .unwrap();
                                    statement
                                        .bind((1, &current_cid.to_bytes() as &[u8]))
                                        .unwrap();
                                    statement.bind((2, height as i64)).unwrap();
                                    statement.next().unwrap();
                                }
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
    }
}

async fn get_chain_info_cid(opt: &ExecutorOptions, current_cid: Cid) -> Option<Cid> {
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}api/v0/dag/resolve?arg={}{}",
            opt.ipfs_url.as_str(),
            current_cid.to_string(),
            "/app/chain-info.json"
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
            ).unwrap();
            Some(response_cid_value)
        }
        Err(e) => { None }
    }
}
