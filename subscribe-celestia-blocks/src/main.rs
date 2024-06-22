use celestia_rpc::{Client as CelestiaClient, HeaderClient};
use ethers::prelude::*;
use ethers::types::{Address, BlockId, BlockNumber, Transaction};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{env, error::Error};
use tokio::time::sleep;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExecutorOptions {
    ethereum_node_url: String,
    celestia_node_url: String,
    db_path: String,
    starting_block: u64,
    target_address: Address,
    starting_celestia_height: u64,
}

struct DecodedTx {
    block_number: u64,
    block_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let ethereum_node_url = env::var("ETHEREUM_NODE_URL")?;
    let celestia_node_url = env::var("CELESTIA_NODE_URL")?;
    let db_path = env::var("DB_PATH")?;

    let celestia_client = CelestiaClient::new(&celestia_node_url, None).await?;
    let (starting_block, target_address, starting_celestia_height) =
        get_initial_settings(&celestia_client).await?;

    let opts = ExecutorOptions {
        ethereum_node_url,
        celestia_node_url,
        db_path,
        starting_block,
        target_address,
        starting_celestia_height,
    };

    let eth_provider = Provider::<Http>::try_from(opts.ethereum_node_url.as_str())?;

    subscribe_and_process(eth_provider, &celestia_client, opts).await
}

async fn get_initial_settings(
    client: &CelestiaClient,
) -> Result<(u64, Address, u64), Box<dyn Error>> {
    let config_block_number = env::var("CONFIG_BLOCK_NUMBER")?.parse::<u64>()?;
    let extended_header = client.header_get_by_height(config_block_number).await?;
    let header = &extended_header.header;
    let settings_data = &header.app_hash.as_bytes();

    let settings_str = String::from_utf8(settings_data.to_vec())?;
    let settings: Vec<&str> = settings_str.split(",").collect();
    let starting_block = settings[0].parse::<u64>()?;
    let target_address = settings[1].parse::<Address>()?;
    let starting_celestia_height = header.height.into();

    Ok((starting_block, target_address, starting_celestia_height))
}

async fn subscribe_and_process(
    provider: Provider<Http>,
    celestia_client: &CelestiaClient,
    opts: ExecutorOptions,
) -> Result<(), Box<dyn Error>> {
    let mut current_celestia_height = opts.starting_celestia_height;
    let mut current_eth_height = opts.starting_block;

    while current_eth_height < u64::MAX {
        let latest_block = provider.get_block_number().await?;
        if latest_block < U64::from(current_eth_height) {
            sleep(Duration::from_secs(10)).await;
            continue;
        }

        let block_with_txs = provider
            .get_block_with_txs(BlockId::Number(BlockNumber::Number(
                current_eth_height.into(),
            )))
            .await?
            .ok_or("Block not found")?;

        let ethereum_block_number = block_with_txs.number.unwrap().as_u64();
        let ethereum_block_hash = block_with_txs.hash.unwrap().to_string();

        for tx in block_with_txs.transactions {
            if let Some(to_address) = tx.to {
                if to_address == opts.target_address {
                    match decode_calldata(&tx.input) {
                        Ok(decoded) => {
                            let result = handle_potential_celestia_reference(
                                &decoded,
                                celestia_client,
                                current_celestia_height,
                                ethereum_block_number,
                                ethereum_block_hash.clone(),
                            )
                            .await?;
                            current_celestia_height = result;
                        }
                        Err(e) => {
                            println!("Error decoding calldata: {}", e);
                        }
                    }
                }
            }
        }
        current_eth_height += 1;
    }
    Ok(())
}

fn decode_calldata(calldata: &Bytes) -> Result<DecodedTx, Box<dyn Error>> {
    if calldata.len() < 40 {
        return Err("Calldata too short to contain necessary information".into());
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
    current_celestia_height: u64,
    ethereum_block_number: u64,
    ethereum_block_hash: String,
) -> Result<u64, Box<dyn Error>> {
    if decoded_tx.block_number == current_celestia_height + 1 {
        println!(
            "Waiting for Celestia block number: {}",
            decoded_tx.block_number
        );
        celestia_client
            .header_wait_for_height(decoded_tx.block_number)
            .await?;

        let celestia_block = celestia_client
            .header_get_by_height(decoded_tx.block_number)
            .await?;

        if celestia_block.header.hash().to_string() != decoded_tx.block_hash {
            println!("Celestia block hash does not match decoded hash.");
            return Ok(current_celestia_height);
        }

        println!(
            "Processing Ethereum block number: {}",
            ethereum_block_number
        );
        println!("Ethereum block hash: {}", ethereum_block_hash);

        return Ok(decoded_tx.block_number);
    }

    Ok(current_celestia_height)
}
