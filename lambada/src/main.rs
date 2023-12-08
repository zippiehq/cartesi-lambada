use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use clap::Parser;
use ethers::prelude::*;
use futures::join;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper_tls::HttpsConnector;
use lambada::executor::{subscribe, ExecutorOptions};
use lambada::Options;
use sequencer::L1BlockInfo;
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::fmt::format;
use std::process::Command;

#[async_std::main]
async fn main() {
    setup_logging();
    setup_backtrace();

    let opt = Options::parse();
    let connection = sqlite::open(opt.db_dir.clone()).unwrap();
    let query = "
    CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
    height INTEGER NOT NULL);
";
    connection.execute(query).unwrap();
    let cartesi_machine_path = opt.machine_dir.as_str();
    let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
    let ipfs_url = "http://127.0.0.1:5001";

    let executor_options = ExecutorOptions {
        hotshot_address: opt.hotshot_address,
        l1_provider: opt.l1_provider.clone(),
        sequencer_url: opt.sequencer_url.clone(),
    };
    let addr = ([0, 0, 0, 0], 3033).into();

    let service = make_service_fn(|_| async move {
        Ok::<_, hyper::Error>(service_fn(|req| async {
            let path = req.uri().path().to_owned();
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let output = request_handler(req, &segments).await;
            match output {
                Ok(res) => Ok(res),
                Err(e) => Err(e.to_string()),
            }
        }))
    });

    let server = Server::bind(&addr).serve(Box::new(service));
    let appchain = Cid::try_from(opt.appchain).unwrap().to_bytes();
    let compute_only = opt.compute_only;
    if compute_only {
        tracing::info!("Compute only");
        server.await;
    } else {
        tracing::info!("With subscribe");

        join!(
            subscribe(
                &executor_options,
                cartesi_machine_url,
                cartesi_machine_path,
                ipfs_url,
                opt.db_dir,
                appchain,
                0,
            ),
            server,
        );
    }
}

async fn request_handler(
    reqest: Request<Body>,
    segments: &[&str],
) -> Result<Response<Body>, Box<dyn std::error::Error + 'static>> {
    match (reqest.method().clone(), segments) {
        (hyper::Method::POST, ["compute", cid]) => {
            if reqest.headers().get("content-type")
                == Some(&hyper::header::HeaderValue::from_static(
                    "application/octet-stream",
                ))
            {
                let data = hyper::body::to_bytes(reqest.into_body())
                    .await
                    .unwrap()
                    .to_vec();
                let connection = sqlite::open("sequencer_db").unwrap();
                let cartesi_machine_path = "/machines/ipfs-using2";

                let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
                let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
                    .await
                    .unwrap();
                let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());
                let ipfs_url = "http://127.0.0.1:5001";
                let state_cid = Cid::try_from(cid.to_string()).unwrap().to_bytes();
                let mut block_info: &L1BlockInfo = &L1BlockInfo {
                    number: 0,
                    timestamp: U256([0; 4]),
                    hash: H256([0; 32]),
                };
                let new_state_cid = execute(
                    forked_machine_url,
                    cartesi_machine_path,
                    ipfs_url,
                    data,
                    state_cid,
                    block_info,
                )
                .await;

                match new_state_cid {
                    Ok(cid) => {
                        let response =
                            Response::new(Body::from(Cid::try_from(cid).unwrap().to_string()));
                        return Ok(response);
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            } else {
                return Err("Request header should be application/octet-streams".into());
            }
        }
        (hyper::Method::GET, ["health"]) => {
            let json_request = r#"{"healthy": "true"}"#;
            let response = Response::new(Body::from(json_request));
            Ok(response)
        }
        (hyper::Method::GET, ["latest"]) => {
            let connection = sqlite::open("sequencer_db").unwrap();
            let mut statement = connection
                .prepare("SELECT * FROM blocks ORDER BY height DESC LIMIT 1")
                .unwrap();

            if let Ok(State::Row) = statement.next() {
                let height = statement.read::<i64, _>("height").unwrap();
                let cid = Cid::try_from(statement.read::<Vec<u8>, _>("state_cid").unwrap())
                    .unwrap()
                    .to_string();
                let json_request_value = serde_json::json!({
                    "height": height,
                    "state_cid": cid
                });

                let json_request = serde_json::to_string(&json_request_value).unwrap();
                let response = Response::new(Body::from(json_request));
                return Ok(response);
            }
            return Err("Failed to get last block".into());
        }
        (hyper::Method::GET, ["block", height_number]) => {
            let connection = sqlite::open("sequencer_db").unwrap();
            let mut statement = connection
                .prepare("SELECT * FROM blocks WHERE height=?")
                .unwrap();
            statement
                .bind((1, height_number.to_owned().parse::<i64>().unwrap()))
                .unwrap();

            if let Ok(State::Row) = statement.next() {
                let height = statement.read::<i64, _>("height").unwrap();
                let cid = Cid::try_from(statement.read::<Vec<u8>, _>("state_cid").unwrap())
                    .unwrap()
                    .to_string();
                let json_request_value: serde_json::Value = serde_json::json!({
                    "height": height,
                    "state_cid": cid
                });

                let json_request = serde_json::to_string(&json_request_value).unwrap();
                let response = Response::new(Body::from(json_request));
                return Ok(response);
            }
            return Err("Failed to receive block".into());
        }
        (hyper::Method::POST, ["submit"]) => {
            let https = HttpsConnector::new();
            let client = Client::builder().build::<_, hyper::Body>(https);
            let uri = "https://query.cortado.espresso.network/submit/submit"
                .parse()
                .unwrap();

            let data: Data = serde_json::from_slice(
                &hyper::body::to_bytes(reqest.into_body())
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let mut req = Request::new(Body::from(bincode::serialize(&data).unwrap()));
            *req.uri_mut() = uri;
            let mut map = HeaderMap::new();
            map.insert(
                header::CONTENT_TYPE,
                "application/octet-stream".parse().unwrap(),
            );
            *req.headers_mut() = map;
            *req.method_mut() = Method::POST;

            let response = client.request(req).await?;
            Ok(response)
        }
        _ => {
            panic!("Invalid request")
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Data {
    vm: u64,
    payload: Vec<u64>,
}
