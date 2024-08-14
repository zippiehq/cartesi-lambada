use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::sleep;

use cid::Cid;
use ethers::prelude::*;
use ethers::types::{BlockId, BlockNumber};
use lambada::{handle_tx, setup_subscriber, ExecutorOptions};
use sqlite::State;
use std::str::FromStr;
#[async_std::main]
async fn main() {
    if let Some(subscribe_input) = setup_subscriber("evm-da") {
        subscribe_evm_da(
            subscribe_input.height,
            subscribe_input.opt,
            &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
            subscribe_input.genesis_cid_text,
            subscribe_input.chain_vm_id,
            subscribe_input.network_type,
        )
        .await;
    }
}

async fn subscribe_evm_da(
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
    chain_vm_id: String,
    network_type: String,
) {
    let sequencer_map = serde_json::from_str::<serde_json::Value>(&opt.sequencer_map)
        .expect("error getting sequencer url from sequencer map");
    let evm_da_client_endpoint = sequencer_map
        .get("evm-da")
        .unwrap()
        .get(network_type)
        .unwrap()
        .get("endpoint")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let eth_client = Arc::new(
        ethers::providers::Provider::<ethers::providers::Http>::try_from(&evm_da_client_endpoint)
            .expect("Could not instantiate Ethereum HTTP Provider"),
    );
    let namespace = chain_vm_id.clone();
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
        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();
        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, current_height as i64)).unwrap();

        for tx in &block.transactions {
            if let Some(to_address) = tx.to {
                if to_address == namespace_address {
                    let call_data = tx.input.clone();
                    let tx_hash = tx.hash;
                    tracing::info!("tx {:?}", tx);
                    if let Ok(state) = statement.next() {
                        // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
                        if state == State::Done {
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
                                opt.clone(),
                                Some(call_data.to_vec()),
                                current_cid,
                                metadata,
                                current_height,
                                genesis_cid_text.clone(),
                                hex::encode(block.hash.unwrap().0),
                            )
                            .await;
                        }
                    }
                }
            }
        }
        current_height += 1;
    }
}
