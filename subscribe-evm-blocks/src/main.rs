use cid::Cid;
use ethers::prelude::*;
use ethers::types::{BlockId, BlockNumber};
use lambada::{handle_tx, setup_subscriber, ExecutorOptions};
use sqlite::State;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
#[async_std::main]
async fn main() {
    if let Some(subscribe_input) = setup_subscriber("evm-blocks") {
        subscribe_evm_blocks(
            subscribe_input.height,
            subscribe_input.opt,
            &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
            subscribe_input.genesis_cid_text,
            subscribe_input.network_type,
        )
        .await;
    }
}

async fn subscribe_evm_blocks(
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
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
    let mut current_height = starting_block_height;
    println!("before current height");
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
        println!(
            "EVM-BLOCKS: latest block {} current height {}",
            latest_block, current_height
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

        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();

        let block = eth_client
            .get_block(BlockId::Number(BlockNumber::Number(current_height.into())))
            .await
            .expect("Failed to fetch block")
            .expect("Block not found");

        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, current_height as i64)).unwrap();

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
        if let Ok(state) = statement.next() {
            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
            if state == State::Done {
                handle_tx(
                    opt.clone(),
                    Some(Vec::new()),
                    current_cid,
                    metadata,
                    current_height,
                    genesis_cid_text.clone(),
                    hex::encode(block.hash.unwrap().0),
                )
                .await;

                current_height += 1;
            }
        }
    }
}
