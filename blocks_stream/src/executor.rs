use hyper::client::conn::Connection;
use sequencer::L1BlockInfo;
use surf_disco::Url;
type HotShotClient = surf_disco::Client<hotshot_query_service::Error>;
use cartesi_lambda::execute;
use cid::Cid;
use ethers::prelude::*;
use futures_util::TryStreamExt;
use hotshot_query_service::availability::BlockQueryData;
use hyper::Request;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use jsonrpc_cartesi_machine::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use sequencer::SeqTypes;
use std::sync::Arc;
use std::sync::Mutex;
pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
#[derive(Clone, Debug)]
pub struct ExecutorOptions {
    pub sequencer_url: Url,
    pub l1_provider: Url,
    pub hotshot_address: Address,
}

pub async fn subscribe(
    opt: &ExecutorOptions,
    cartesi_machine_url: String,
    cartesi_machine_path: &str,
    ipfs_url: &str,
    db_dir: String,
    appchain: Vec<u8>,
) {
    let ExecutorOptions {
        sequencer_url,
        l1_provider: _,
        hotshot_address: _,
    } = opt;
    let connection = sqlite::open(db_dir.clone()).unwrap();

    let query_service_url = sequencer_url.join("availability").unwrap();
    let hotshot = HotShotClient::new(query_service_url.clone());
    hotshot.connect(None).await;

    let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();
    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());

    let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
        .await
        .unwrap();
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

    let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();

    let chain_info = ipfs_client
        .cat(&(Cid::try_from(appchain.clone()).unwrap().to_string() + "/app/chain-info.json"))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();
    tracing::info!("chain info len {}", chain_info.len());
    tracing::info!(
        "chain info {}",
        String::from_utf8(chain_info.clone()).unwrap()
    );

    let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
        .expect("error reading chain-info.json file");

    let block_height: u64 = chain_info
        .get("sequencer")
        .unwrap()
        .get("height")
        .unwrap()
        .as_str().unwrap()
        .parse::<u64>()
        .unwrap();

    let chain_vm_id: u64 = chain_info
        .get("sequencer")
        .unwrap()
        .get("vm-id")
        .unwrap()
        .as_str().unwrap()
        .parse::<u64>()
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "http://127.0.0.1:5001/api/v0/dag/resolve?arg={}/app",
            Cid::try_from(appchain.clone()).unwrap().to_string()
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let client = hyper::Client::new();

    let mut app_cid = String::new();

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
                .unwrap().as_str().unwrap().to_string();
        }
        Err(e) => {
            panic!("{}", e)
        }
    }

    let mut block_query_stream = hotshot
        .socket(format!("stream/blocks/{}", block_height).as_str())
        .subscribe()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    let mut hash: Vec<u8> = vec![0; 32];
    let exception_err = std::io::Error::new(std::io::ErrorKind::Other, "exception");
    let mut current_cid: Vec<u8> = appchain;

    while let Some(block_data) = block_query_stream.next().await {
        match block_data {
            Ok(block) => {
                let block: BlockQueryData<SeqTypes> = block;

                let mut block_info: &L1BlockInfo = &L1BlockInfo {
                    number: 0,
                    timestamp: U256([0; 4]),
                    hash: H256([0; 32]),
                };
                if let Some(info) = block.block().l1_finalized() {
                    block_info = info;
                }

                let height = block.height();
                //tracing::info!("block height {}", height);
                let timestamp = block.timestamp().unix_timestamp() as u64;
                hash = block.hash().into_bits().into_vec();

                if block.block().transactions().len() != 0 {
                    hash = block.hash().into_bits().into_vec();
                }
                for (index, tx) in block.block().transactions().into_iter().enumerate() {
                    //if u64::from(tx.vm()) as u64 == vm_id {
                    tracing::info!("found tx for our vm id");
                    tracing::info!("tx.payload().len: {:?}", tx.payload().len());
                    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());
                    let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
                        .await
                        .unwrap();
                    let connection = sqlite::open(db_dir.clone()).unwrap();

                    let result = execute(
                        &mut machine,
                        ipfs_url,
                        tx.payload().to_vec(),
                        current_cid.clone(),
                        app_cid.clone(),
                        block_info,
                    )
                    .await;

                    if let Ok(cid) = result {
                        current_cid = cid;
                    } else {
                        result.unwrap();
                    }
                    //}
                }
                /*let mut statement = connection
                    .prepare("INSERT INTO blocks (hash, height) VALUES (?, ?)")
                    .unwrap();
                statement.bind((1, &hash as &[u8])).unwrap();
                statement.bind((2, height as i64)).unwrap();
                statement.next().unwrap();*/
            }
            Err(err) => {
                tracing::error!("Error in HotShot block stream, retrying: {err}");
                continue;
            }
        };
    }
}