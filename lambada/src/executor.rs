use hyper::client::conn::Connection;
use sequencer::L1BlockInfo;
use surf_disco::Url;
type HotShotClient = surf_disco::Client<hotshot_query_service::Error>;
use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use ethers::prelude::*;
use futures_util::TryStreamExt;
use hotshot_query_service::availability::BlockQueryData;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use sequencer::SeqTypes;
use sqlite::State;
use std::time::{Duration, SystemTime};

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
#[derive(Clone, Debug)]
pub struct ExecutorOptions {
    pub sequencer_url: Url,
}

pub async fn subscribe(
    opt: &ExecutorOptions,
    cartesi_machine_url: String,
    cartesi_machine_path: &str,
    ipfs_url: &str,
    db_file: String,
    appchain: Vec<u8>,
    height: u64,
) {
    let mut current_cid: Vec<u8> = appchain.clone();

    let sequencer_url = opt.sequencer_url.clone();
    let connection = sqlite::open(db_file.clone()).unwrap();

    let query_service_url = sequencer_url.join("availability").unwrap();
    let hotshot = HotShotClient::new(query_service_url.clone());
    hotshot.connect(None).await;

    let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();

    tracing::info!("getting chain info");
    let ipfs_client = IpfsClient::from_str(ipfs_url).unwrap();

    let chain_info = ipfs_client
        .cat(&(Cid::try_from(appchain.clone()).unwrap().to_string() + "/app/chain-info.json"))
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

    let block_height: u64 = chain_info
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

    let mut block_query_stream = hotshot
        .socket(format!("stream/blocks/{}", height).as_str())
        .subscribe()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    let mut hash: Vec<u8> = vec![0; 32];
    let exception_err = std::io::Error::new(std::io::ErrorKind::Other, "exception");
    tracing::info!("iterating through blocks");

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
                let timestamp = block.timestamp().unix_timestamp() as u64;
                hash = block.hash().into_bits().into_vec();

                if block.block().transactions().len() != 0 {
                    hash = block.hash().into_bits().into_vec();
                }
                let mut statement = connection
                    .prepare("SELECT * FROM blocks WHERE height=?")
                    .unwrap();
                statement.bind((1, height as i64)).unwrap();

                if let Ok(state) = statement.next() {
                    if state == State::Done {
                        for (index, tx) in block.block().transactions().into_iter().enumerate() {
                            if u64::from(tx.vm()) as u64 == chain_vm_id {
                                tracing::info!("found tx for our vm id");
                                tracing::info!("tx.payload().len: {:?}", tx.payload().len());

                                let forked_machine_url =
                                    format!("http://{}", machine.fork().await.unwrap());

                                let time_before_execute = SystemTime::now();

                                let result = execute(
                                    forked_machine_url,
                                    cartesi_machine_path,
                                    ipfs_url,
                                    tx.payload().to_vec(),
                                    current_cid.clone(),
                                    block_info,
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
                                    .prepare("INSERT INTO blocks (state_cid, height) VALUES (?, ?)")
                                    .unwrap();
                                statement.bind((1, &current_cid as &[u8])).unwrap();
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
