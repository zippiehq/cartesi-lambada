use crate::executor::{trigger_callback_for_newblock, ExecutorOptions};
use ethers::{abi::Abi, prelude::*};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::{env, error::Error};

#[derive(Debug, Deserialize, Serialize)]
struct WebhookBody {
    appchain_cid: String,
    block_height: u64,
    new_state_cid: String,
    output_hash: String,
}

async fn handle_request(
    req: Request<Body>,
    contract: Arc<Contract<SignerMiddleware<Provider<Http>, LocalWallet>>>,
    executor_options: Arc<ExecutorOptions>,
) -> Result<Response<Body>, Infallible> {
    if req.method() == Method::POST && req.uri().path() == "/postState" {
        let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
        let parsed_body: WebhookBody = serde_json::from_slice(&whole_body).unwrap();

        let params = (
            parsed_body.appchain_cid.clone(),
            parsed_body.block_height,
            parsed_body.new_state_cid.clone(),
            parsed_body.output_hash,
        );

        match contract
            .method::<_, H256>("postBlock", params)
            .expect("Contract method call setup failed")
            .send()
            .await
        {
            Ok(tx_receipt) => {
                // Instead of awaiting here, spawn a new thread
                let executor_options_clone = executor_options.clone();
                let appchain_cid = parsed_body.appchain_cid.clone();
                let block_height = parsed_body.block_height;
                let new_state_cid = parsed_body.new_state_cid.clone();
                std::thread::spawn(move || {
                    // Create a new Tokio runtime or use an existing async runtime if applicable
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let _ = trigger_callback_for_newblock(
                            executor_options_clone,
                            &appchain_cid,
                            block_height,
                            &new_state_cid,
                        )
                        .await;
                    });
                });

                Ok(Response::new(Body::from(format!(
                    "{{\"tx_hash\":\"{:?}\"}}",
                    tx_receipt.tx_hash()
                ))))
            }
            Err(_) => Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to call contract method"))
                .unwrap()),
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();

    let provider = Provider::<Http>::try_from(env::var("INFURA_URL")?)?;
    let wallet = env::var("PRIVATE_KEY")?
        .parse::<LocalWallet>()?
        .with_chain_id(1u64);
    let client = SignerMiddleware::new(provider, wallet);
    let contract_address: Address = env::var("CONTRACT_ADDRESS")?.parse()?;
    let contract_abi_str = include_str!("../../build/AppChainState.abi");
    let contract_abi: Abi =
        serde_json::from_str(contract_abi_str).expect("Failed to parse contract ABI");

    let contract = Arc::new(Contract::new(
        contract_address,
        contract_abi,
        Arc::new(client),
    ));

    let executor_options = Arc::new(ExecutorOptions {
        espresso_testnet_sequencer_url: env::var("ESPRESSO_TESTNET_SEQUENCER_URL").unwrap(),
        celestia_testnet_sequencer_url: env::var("CELESTIA_TESTNET_SEQUENCER_URL").unwrap(),
        ipfs_url: env::var("IPFS_URL").unwrap(),
        ipfs_write_url: env::var("IPFS_WRITE_URL").unwrap(),
        db_path: env::var("DB_PATH").unwrap(),
    });

    let service = make_service_fn(move |_conn| {
        let contract_clone = Arc::clone(&contract);
        let executor_options_clone = Arc::clone(&executor_options);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(
                    req,
                    Arc::clone(&contract_clone),
                    Arc::clone(&executor_options_clone),
                )
            }))
        }
    });

    let addr = ([127, 0, 0, 1], 3030).into();
    let server = Server::bind(&addr).serve(service);
    tracing::info!("Server running on http://{}", addr);

    let _ = server.await;

    Ok(())
}
