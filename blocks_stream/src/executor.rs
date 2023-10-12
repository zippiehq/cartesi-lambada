use surf_disco::Url;
type HotShotClient = surf_disco::Client<hotshot_query_service::Error>;
use cartesi_lambda::execute;
use ethers::prelude::*;
use hotshot_query_service::availability::BlockQueryData;
use jsonrpc_cartesi_machine::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use sequencer::SeqTypes;

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
    block_height: u64
) {
    let ExecutorOptions {
        sequencer_url,
        l1_provider: _,
        hotshot_address: _,
    } = opt;

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

    while let Some(block_data) = block_query_stream.next().await {
        match block_data {
            Ok(block) => {
                let block: BlockQueryData<SeqTypes> = block;
                tracing::info!("block height {}", block.height());
                for tx in block.block().transactions() {
                    if u64::from(tx.vm()) as u64 == vm_id {
                        tracing::info!("found tx for our vm id");
                        tracing::info!("tx.payload().len: {:?}", tx.payload().len());
                        let forked_machine_url =
                            format!("http://{}", machine.fork().await.unwrap());
                        let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
                            .await
                            .unwrap();

                        execute(&mut machine, ipfs_url, tx.payload().to_vec()).await;
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
