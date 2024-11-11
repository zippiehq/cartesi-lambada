use avail_subxt::AvailClient;

use avail_subxt::api::data_availability::calls::types::SubmitData;
use avail_subxt::primitives::CheckAppId;
use cid::Cid;
use futures_util::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use lambada::Batch;
use lambada::{
    executor::calculate_sha256, handle_tx, setup_subscriber, trigger_callback_for_newblock,
    ExecutorOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlite::State;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
#[async_std::main]
async fn main() {
    if let Some(subscribe_input) = setup_subscriber("avail") {
        subscribe_avail(
            subscribe_input.height,
            subscribe_input.current_chain_info_cid,
            subscribe_input.opt,
            &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
            subscribe_input.chain_vm_id,
            subscribe_input.genesis_cid_text,
            subscribe_input.network_type,
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
    network_type: String,
) {
    let sequencer_map = serde_json::from_str::<serde_json::Value>(&opt.sequencer_map)
        .expect("error getting sequencer url from sequencer map");
    let avail_client_endpoint = sequencer_map
        .get("avail")
        .unwrap()
        .get(network_type)
        .unwrap()
        .get("endpoint")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let ipfs_client = IpfsClient::from_str(&opt.ipfs_url).unwrap();
    let mut chain_info = ipfs_client
        .cat(&format!("{}/gov/chain-info.json", current_cid.to_string()))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();

    let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
        .expect("error reading chain-info.json file");

    let activate_paio_batch: bool = chain_info
        .get("sequencer")
        .unwrap()
        .get("paio-batches")
        .unwrap_or(&Value::String("false".to_string()))
        .as_str()
        .unwrap()
        .parse::<bool>()
        .unwrap();

    println!("activate_paio_batch {:?}", activate_paio_batch);

    let client = AvailClient::new(&avail_client_endpoint).await.unwrap();
    let mut current_height = if current_height == 0 {
        1
    } else {
        current_height
    };
    let chain_vm_id_num: u64 = chain_vm_id.parse::<u64>().expect("VM ID as u64");
    let avail_tx_namespace = chain_vm_id_num.to_be_bytes().to_vec();
    let mut avail_tx_count: u64 = 0;
    loop {
        match client
            .legacy_rpc()
            .chain_get_block_hash(Some(current_height.into()))
            .await
            .unwrap()
        {
            None => {
                sleep(Duration::from_secs(2)).await;
            }
            Some(block_hash) => {
                let mut block = client.blocks().at(block_hash).await.unwrap();
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

                            if app_id
                                == parity_scale_codec::Compact(chain_vm_id_num.try_into().unwrap())
                            {
                                if activate_paio_batch {
                                    println!("receiving of new batch");
                                    let batch = Batch::from_bytes(tx_data).unwrap();
                                    for tx in batch.txs {
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
                                            Some(tx),
                                            current_cid,
                                            tx_metadata,
                                            current_height,
                                            genesis_cid_text.clone(),
                                            hex::encode(block_hash),
                                        )
                                        .await;
                                        avail_tx_count += 1;
                                    }
                                } else {
                                    tracing::info!(
                                        "app_id {:?}, tx_data.to_vec() {:?} block number {:?}",
                                        app_id,
                                        String::from_utf8(tx_data.to_vec()).unwrap(),
                                        block_number
                                    );
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
                current_height = current_height + 1
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SubscribeResponse {
    finished: bool,
}
