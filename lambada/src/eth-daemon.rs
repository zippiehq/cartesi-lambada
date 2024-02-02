use ethers::prelude::*;
use serde::{Deserialize, Serialize};
use std::{env, error::Error};
use warp::Filter;

#[derive(Debug, Deserialize, Serialize)]
struct WebhookBody {
    appchain_cid: String,
    block_height: u64,
    new_state_cid: String,
    output_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();

    let provider = Provider::<Http>::try_from(env::var("INFURA_URL")?)?;
    let wallet = env::var("PRIVATE_KEY")?.parse::<LocalWallet>()?.with_chain_id(1u64);
    let client = SignerMiddleware::new(provider, wallet);

    let contract_address = env::var("CONTRACT_ADDRESS")?.parse::<Address>()?;
    let contract_abi = include_str!("");
    let contract = Contract::from_json(client, contract_address, contract_abi.as_bytes())?;

    let with_contract = warp::any().map(move || contract.clone());

    let post_state_route = warp::post()
        .and(warp::path("postState"))
        .and(warp::body::json())
        .and(with_contract)
        .and_then(|body: WebhookBody, contract: Contract<SignerMiddleware<Provider<Http>, LocalWallet>>| async move {
            let tx_hash = contract
                .method::<_, H256>(
                    "postBlock",
                    (
                        body.appchain_cid.parse::<[u8; 32]>()?,
                        body.block_height,
                        hex::decode(&body.new_state_cid)?,
                        body.output_hash.parse::<[u8; 32]>()?,
                    ),
                )?
                .send()
                .await?;

            Ok::<_, warp::Rejection>(warp::reply::json(&tx_hash))
        });

    warp::serve(post_state_route).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}
