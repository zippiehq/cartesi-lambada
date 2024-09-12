use celestia_rpc::{BlobClient, Client as CelestiaClient, HeaderClient};
use celestia_types::nmt::Namespace;
use cid::Cid;
use ethers::prelude::*;
use ethers::types::{BlockId, BlockNumber};
use lambada::{executor::calculate_sha256, handle_tx, setup_subscriber, ExecutorOptions};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};
use tokio::time::{sleep, Duration};

#[derive(Serialize, Deserialize, Debug)]
pub struct DecodedTx {
    pub block_number: u64,
    pub block_hash: String,
}

#[tokio::main]
pub async fn main() {
    if let Some(subscribe_input) = setup_subscriber("celestia-blocks") {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let results = subscribe_celestia_blocks(
                subscribe_input.height,
                subscribe_input.opt,
                &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
                subscribe_input.genesis_cid_text,
                subscribe_input.chain_vm_id,
                subscribe_input.network_type,
                subscribe_input.chain_info,
            )
            .await;
            if let Err(e) = results {
                eprintln!("Error subscribing to Celestia blocks: {:?}", e);
            } else {
                println!("Successfully subscribed to Celestia blocks.");
            }
        });
    }
}

pub async fn subscribe_celestia_blocks(
    mut current_celestia_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
    chain_vm_id: String,
    network_type: String,
    chain_info: String,
) -> Result<(), Box<dyn Error>> {
    let sequencer_map = serde_json::from_str::<serde_json::Value>(&opt.sequencer_map)
        .expect("error getting sequencer url from sequencer map");
    let chain_info_value: serde_json::Value =
        serde_json::from_str(&chain_info).expect("Failed to parse chain_info JSON");
    let ethereum_url_client_endpoint = sequencer_map
        .get("evm-da")
        .unwrap()
        .get(network_type.clone())
        .unwrap()
        .get("endpoint")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let celestia_url_client_endpoint = sequencer_map
        .get("celestia")
        .unwrap()
        .get(&network_type)
        .unwrap()
        .get("endpoint")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let system_starting_celestia_height: u64 = chain_info_value
        .get("sequencer")
        .unwrap()
        .get("system-celestia-starting-height")
        .unwrap()
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();

    // starting_celestia_height
    let system_starting_ethereum_height: u64 = chain_info_value
        .get("sequencer")
        .unwrap()
        .get("system-ethereum-starting-height")
        .unwrap()
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();

    let celestia_client = CelestiaClient::new(&celestia_url_client_endpoint, None).await?;
    let eth_provider = Arc::new(
        ethers::providers::Provider::<Http>::try_from(&ethereum_url_client_endpoint)
            .expect("Could not instantiate Ethereum node url"),
    );

    let mut system_current_eth_height = system_starting_ethereum_height;
    let mut system_current_celestia_height = system_starting_celestia_height;

    // loop through Ethereum blocks and check for Celestia references
    while system_current_celestia_height < u64::MAX {
        // get the block with transactions
        let block_with_txs = eth_provider
            .get_block_with_txs(BlockId::Number(BlockNumber::from(
                system_current_eth_height,
            )))
            .await?;

        // check if the block has any transactions
        if let Some(block_with_txs) = block_with_txs {
            for tx in block_with_txs.transactions {
                // decode the calldata
                let decoded = decode_calldata(&tx.input)?;
                // check if the decoded block number is the next Celestia block
                if decoded.block_number == system_current_celestia_height + 1 {
                    let header = celestia_client
                        .header_get_by_height(system_current_celestia_height + 1)
                        .await?;

                    println!(
                        "reached the current celestia height {}: {}",
                        system_current_celestia_height + 1,
                        header.hash()
                    );
                    // check if the Celestia block hash matches the decoded hash
                    let valid = handle_potential_celestia_reference(
                        &decoded,
                        &celestia_client,
                        decoded.block_number,
                    )
                    .await?;
                    // if the Celestia block hash matches the decoded hash, process the transactions
                    if valid && decoded.block_number >= current_celestia_height {
                        println!(
                            "Valid transaction at Celestia block {}: {}",
                            decoded.block_number, tx.hash
                        );
                        // get all blobs for the Celestia block
                        match celestia_client
                            .blob_get_all(
                                decoded.block_number,
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
                                statement.bind((1, decoded.block_number as i64)).unwrap();
                                for blob in blobs {
                                    let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
                                    metadata.insert(
                                        calculate_sha256("sequencer".as_bytes()),
                                        calculate_sha256("celestia".as_bytes()),
                                    );
                                    metadata.insert(
                                        calculate_sha256("celestia-block-height".as_bytes()),
                                        decoded.block_number.to_be_bytes().to_vec(),
                                    );

                                    handle_tx(
                                        opt.clone(),
                                        Some(blob.data),
                                        current_cid,
                                        metadata,
                                        current_celestia_height,
                                        genesis_cid_text.clone(),
                                        hex::encode(&decoded.block_number.to_be_bytes()),
                                    )
                                    .await;

                                    println!(
                                        "Processed blob at Celestia block {}: {}",
                                        decoded.block_number, tx.hash
                                    );
                                }
                                system_current_celestia_height = decoded.block_number;
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to fetch blobs for Celestia block {}: {}",
                                    decoded.block_number, e
                                );
                            }
                        }
                    }
                }
            }
        } else {
            println!(
                "No transactions found in Ethereum block {}",
                system_current_eth_height
            );
        }
        // increment the Ethereum block number and wait for 1 second
        system_current_eth_height += 1;
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

pub fn decode_calldata(calldata: &Bytes) -> Result<DecodedTx, Box<dyn Error>> {
    if calldata.len() != 40 {
        return Err("Calldata too short".into());
    }

    let block_number = u64::from_be_bytes(
        calldata[0..8]
            .try_into()
            .map_err(|_| "Failed to parse block number")?,
    );
    let block_hash = hex::encode(&calldata[8..40]);

    Ok(DecodedTx {
        block_number,
        block_hash,
    })
}

async fn handle_potential_celestia_reference(
    decoded_tx: &DecodedTx,
    celestia_client: &CelestiaClient,
    block_number: u64,
) -> Result<bool, Box<dyn Error>> {
    let celestia_block_result = celestia_client
        .header_get_by_height(decoded_tx.block_number)
        .await;

    match celestia_block_result {
        Ok(celestia_block) => {
            let actual_block_hash = celestia_block.header.hash().to_string().to_lowercase();
            let expected_block_hash = decoded_tx.block_hash.to_lowercase();

            if actual_block_hash != expected_block_hash {
                println!("Validation criteria not met at block {}:", block_number);
                println!("Expected block hash: {}", expected_block_hash);
                println!("Actual block hash from Celestia: {}", actual_block_hash);
                return Ok(false);
            } else {
                println!(
                    "Valid transaction at block {}: {}",
                    block_number, actual_block_hash
                );
                return Ok(true);
            }
        }
        Err(e) => {
            eprintln!(
                "Failed to fetch block data from Celestia for block number {}: {}",
                decoded_tx.block_number, e
            );
            return Err(e.into());
        }
    }
}
