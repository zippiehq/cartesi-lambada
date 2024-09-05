use celestia_rpc::{BlobClient, HeaderClient};
use celestia_types::nmt::Namespace;
use celestia_types::SyncState;
use cid::Cid;
use futures_util::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use lambada::Batch;
use lambada::{
    executor::calculate_sha256, handle_tx, is_chain_info_same, setup_subscriber,
    trigger_callback_for_newblock, ExecutorOptions,
};
use serde_json::Value;
use sqlite::State;
use std::thread;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[async_std::main]
async fn main() {
    if let Some(subscribe_input) = setup_subscriber("celestia") {
        subscribe_celestia(
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
async fn subscribe_celestia(
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
    let celestia_client_endpoint = sequencer_map
        .get("celestia")
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
    let token = match std::env::var("CELESTIA_TESTNET_NODE_AUTH_TOKEN_READ") {
        Ok(token) => token,
        Err(_) => return,
    };
    let client = celestia_rpc::Client::new(&celestia_client_endpoint, Some(token.as_str()))
        .await
        .unwrap();
    let current_height = if current_height == 0 {
        1
    } else {
        current_height
    };
    let chain_vm_id_num: u64 = chain_vm_id.parse::<u64>().expect("VM ID as u64");
    let celestia_tx_namespace = chain_vm_id_num.to_be_bytes().to_vec();
    let mut celestia_tx_count: u64 = 0;
    match client.header_wait_for_height(current_height).await {
        Ok(extended_header) => {
            let mut state = client.header_sync_state().await.unwrap();
            while client
                .header_wait_for_height(state.height + 1)
                .await
                .is_ok()
            {
                //let chain_info_cid = Arc::clone(&current_chain_info_cid);
                if !is_chain_info_same(
                    opt.clone(),
                    *current_cid,
                    Arc::new(Mutex::new(Some(
                        Cid::try_from(current_chain_info_cid.clone()).unwrap(),
                    ))),
                )
                .await
                {
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
                        let blobs = blobs.unwrap();
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
                                    if activate_paio_batch {
                                        println!("receiving of new batch");
                                        let batch = Batch::from_bytes(&blob.data).unwrap();
                                        for tx in batch.txs {
                                            let mut tx_metadata = metadata.clone();

                                            tx_metadata.insert(
                                                calculate_sha256("celestia-tx-count".as_bytes()),
                                                celestia_tx_count.to_be_bytes().to_vec(),
                                            );
                                            tx_metadata.insert(
                                                calculate_sha256(
                                                    "celestia-tx-namespace".as_bytes(),
                                                ),
                                                celestia_tx_namespace.clone(),
                                            );
                                            handle_tx(
                                                opt.clone(),
                                                Some(tx),
                                                current_cid,
                                                tx_metadata.clone(),
                                                state.height,
                                                genesis_cid_text.clone(),
                                                hex::encode(extended_header.commit.block_id.hash),
                                            )
                                            .await;
                                            celestia_tx_count += 1;
                                        }
                                    } else {
                                        let mut tx_metadata = metadata.clone();
                                        tx_metadata.insert(
                                            calculate_sha256("celestia-tx-count".as_bytes()),
                                            celestia_tx_count.to_be_bytes().to_vec(),
                                        );
                                        tx_metadata.insert(
                                            calculate_sha256("celestia-tx-namespace".as_bytes()),
                                            celestia_tx_namespace.clone(),
                                        );

                                        handle_tx(
                                            opt.clone(),
                                            Some(blob.data),
                                            current_cid,
                                            tx_metadata,
                                            state.height,
                                            genesis_cid_text.clone(),
                                            hex::encode(extended_header.commit.block_id.hash),
                                        )
                                        .await;
                                        celestia_tx_count += 1;
                                    }
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
