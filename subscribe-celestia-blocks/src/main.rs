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
) -> Result<(), Box<dyn Error>> {
    let sequencer_map = serde_json::from_str::<serde_json::Value>(&opt.sequencer_map)
        .expect("error getting sequencer url from sequencer map");
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

    let celestia_client = CelestiaClient::new(&celestia_url_client_endpoint, None).await?;
    let eth_provider = Arc::new(
        ethers::providers::Provider::<Http>::try_from(&ethereum_url_client_endpoint)
            .expect("Could not instantiate Ethereum node url"),
    );

    let mut current_eth_block = eth_provider
        .get_block_number()
        .await
        .expect("Failed to fetch the current Ethereum block number");
    println!("Current Ethereum block number is: {}", current_eth_block);

    let target_address = H160::from_str("0xff00000000000000000000000000000000000010")
        .expect("Invalid Ethereum address");

    while current_celestia_height < u64::MAX {
        match eth_provider
            .get_block_with_txs(BlockId::Number(BlockNumber::from(current_eth_block)))
            .await
        {
            Ok(Some(block_with_txs)) => {
                for tx in block_with_txs.transactions {
                    if let Some(to_address) = tx.to {
                        if to_address == target_address {
                            let decoded = decode_calldata(&tx.input)?;
                            if decoded.block_number == current_celestia_height + 1 {
                                match celestia_client
                                    .header_wait_for_height(current_celestia_height + 1)
                                    .await
                                {
                                    Ok(header) => {
                                        println!(
                                            "Reached cthe current celestia block height {}: {}",
                                            current_celestia_height + 1,
                                            header.hash()
                                        );
                                        let valid = handle_potential_celestia_reference(
                                            &decoded,
                                            &celestia_client,
                                            decoded.block_number,
                                        )
                                        .await?;
                                        if valid {
                                            let blobs = celestia_client
                                                .blob_get_all(
                                                    decoded.block_number,
                                                    &[Namespace::new_v0(&chain_vm_id.as_bytes())
                                                        .unwrap()],
                                                )
                                                .await?;
                                            let connection =
                                                sqlite::Connection::open_thread_safe(format!(
                                                    "{}/chains/{}",
                                                    opt.db_path, genesis_cid_text
                                                ))
                                                .unwrap();

                                            let mut statement = connection
                                                .prepare("SELECT * FROM blocks WHERE height=?")
                                                .unwrap();
                                            statement
                                                .bind((1, decoded.block_number as i64))
                                                .unwrap();

                                            for blob in blobs {
                                                let mut metadata: HashMap<Vec<u8>, Vec<u8>> =
                                                    HashMap::new();
                                                metadata.insert(
                                                    calculate_sha256("sequencer".as_bytes()),
                                                    calculate_sha256("celestia".as_bytes()),
                                                );

                                                metadata.insert(
                                                    calculate_sha256(
                                                        "celestia-block-height".as_bytes(),
                                                    ),
                                                    decoded.block_number.to_be_bytes().to_vec(),
                                                );

                                                handle_tx(
                                                    opt.clone(),
                                                    Some(blob.data),
                                                    current_cid,
                                                    metadata,
                                                    current_celestia_height,
                                                    genesis_cid_text.clone(),
                                                    hex::encode(
                                                        &decoded.block_number.to_be_bytes(),
                                                    ),
                                                )
                                                .await;

                                                println!("Processed transaction at Celestia block {}: {}", decoded.block_number, tx.hash);
                                            }
                                            current_celestia_height = decoded.block_number;
                                        } else {
                                            println!("Validation failed for Celestia block at height {}: incorrect hash", decoded.block_number);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to wait for current celestia block height {}: {}", current_celestia_height + 1, e);
                                        return Err(e.into());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(None) => println!(
                "No transactions found in Ethereum block {}",
                current_eth_block
            ),
            Err(e) => {
                eprintln!("Error fetching Ethereum block {}: {}", current_eth_block, e);
                sleep(Duration::from_secs(10)).await;
            }
        }
        current_eth_block += U64::from(1);

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
