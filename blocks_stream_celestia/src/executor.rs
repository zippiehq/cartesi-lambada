use hyper::client::conn::Connection;
use surf_disco::Url;
type HotShotClient = surf_disco::Client<hotshot_query_service::Error>;
use cartesi_lambda::execute;
use ethers::prelude::*;
use hotshot_query_service::availability::BlockQueryData;
use jsonrpc_cartesi_machine::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use jsonrpsee::http_client::{HeaderMap, HttpClientBuilder};
use sequencer::SeqTypes;
use http::{header, HeaderValue};
use celestia::index::CelestiaNodeAPI;
use std::env;
pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;

pub async fn subscribe(
    namespaces: Vec<String>,
    cartesi_machine_url: String,
    cartesi_machine_path: &str,
    ipfs_url: &str,
    db_dir: String,
    celestia_client_url: &str,
) {
    let connection = sqlite::open(db_dir.clone()).unwrap();

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

    let mut hash: Vec<u8> = vec![0; 32];

    let token = env::var("CELESTIA_NODE_AUTH_TOKEN_WRITE").unwrap();
    let mut headers = HeaderMap::new();
    let val = HeaderValue::from_str(&format!("Bearer {token}")).unwrap();
    headers.insert(header::AUTHORIZATION, val);
    let transport = HttpClientBuilder::default()
        .set_headers(headers)
        .build(celestia_client_url)
        .unwrap();
    let client = CelestiaNodeAPI::new(transport);
    let mut state = client.HeaderSyncState().await.unwrap();
    while client
        .HeaderWaitForHeight(state.height.unwrap() + 1)
        .await
        .is_ok()
    {
        state = client.HeaderSyncState().await.unwrap();
        tracing::info!("block height {}", state.height.unwrap());

        match client
            .BlobGetAll(
                state.height.unwrap(),
                vec![String::from("AAAAAAAAAAAAAAAAAAAAAAAAAEJpDCBNOWAP3dM=")],
            )
            .await
        {
            Ok(blobs) => {
                println!("received blobs vector {:?}", blobs);
                for (index, tx) in blobs.into_iter().enumerate() {
                    tracing::info!("blob data length: {:?}", tx.data.clone().unwrap().len());
                    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());
                    let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
                        .await
                        .unwrap();
                    let connection = sqlite::open(db_dir.clone()).unwrap();
                    let header = client.HeaderGetByHeight(state.height.unwrap()).await.unwrap().commit.unwrap();
                    /*execute(
                        &mut machine,
                        ipfs_url,
                        base64::decode(tx.data.unwrap()).unwrap(),
                        chrono::DateTime::parse_from_rfc3339(header.signatures.unwrap().last().unwrap().timestamp.as_ref().unwrap()).unwrap().timestamp() as u64,
                        state.height.unwrap(),
                        index as u64,
                        connection,
                    )
                    .await;*/
                }
                let mut statement = connection
                    .prepare("INSERT INTO blocks (hash, height) VALUES (?, ?)")
                    .unwrap();
                statement.bind((1, &hash as &[u8])).unwrap();
                statement.bind((2, state.height.unwrap() as i64)).unwrap();
                statement.next().unwrap();
            }
            Err(e) => {}
        }
    }

    tracing::error!("Error getting HeaderWaitForHeight");
}
