use hyper::client::conn::Connection;
use surf_disco::Url;
type HotShotClient = surf_disco::Client<hotshot_query_service::Error>;
use cartesi_lambda::execute;
use ethers::prelude::*;
use hotshot_query_service::availability::BlockQueryData;
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
    vm_id: u64,
    block_height: u64,
    db_dir: String,
    state_cid: Vec<u8>
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

    let mut block_query_stream = hotshot
        .socket(format!("stream/blocks/{}", block_height).as_str())
        .subscribe()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    let mut hash: Vec<u8> = vec![0; 32];
    let exception_err = std::io::Error::new(std::io::ErrorKind::Other, "exception");
    let mut current_cid: Vec<u8> = state_cid;

    while let Some(block_data) = block_query_stream.next().await {
        match block_data {
            Ok(block) => {
                let block: BlockQueryData<SeqTypes> = block;
                let height = block.height();
                tracing::info!("block height {}", height);
                let timestamp = block.timestamp().unix_timestamp() as u64;
                hash = block.hash().into_bits().into_vec();

                if block.block().transactions().len() != 0 {
                    hash = block.hash().into_bits().into_vec();
                }
                for (index, tx) in block.block().transactions().into_iter().enumerate() {
                    if u64::from(tx.vm()) as u64 == vm_id {
                        tracing::info!("found tx for our vm id");
                        tracing::info!("tx.payload().len: {:?}", tx.payload().len());
                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());
                        let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
                            .await
                            .unwrap();
                        let connection = sqlite::open(db_dir.clone()).unwrap();

                        let result = execute(&mut machine, ipfs_url, tx.payload().to_vec(), current_cid.clone()).await;
                        if let Ok(cid) = result {
                            current_cid = cid;
                        }
                        else if let Some(exception_err) = result.as_ref().err() {
                            current_cid = execute(&mut machine, ipfs_url, tx.payload().to_vec(), current_cid).await.unwrap();
                        }
                        else {
                            result.unwrap();
                        }
                    }
                }
                let mut statement = connection
                    .prepare("INSERT INTO blocks (hash, height) VALUES (?, ?)")
                    .unwrap();
                statement.bind((1, &hash as &[u8])).unwrap();
                statement.bind((2, height as i64)).unwrap();
                statement.next().unwrap();
            }
            Err(err) => {
                tracing::error!("Error in HotShot block stream, retrying: {err}");
                continue;
            }
        };
    }
}
