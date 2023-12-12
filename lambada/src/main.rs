use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use clap::Parser;
use ethers::prelude::*;
use futures::join;
use futures::TryStreamExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::{body::Buf, StatusCode};
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper_tls::HttpsConnector;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use lambada::executor::{subscribe, ExecutorOptions};
use lambada::Options;
use sequencer::L1BlockInfo;
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::process::Command;
use std::sync::Arc;
use std::{convert::Infallible, fmt::format};
#[async_std::main]
async fn main() {
    setup_logging();
    setup_backtrace();

    let opt = Options::parse();
    let connection = sqlite::open(opt.db_file.clone()).unwrap();
    let query = "
    CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
    height INTEGER NOT NULL);
";
    connection.execute(query).unwrap();
    let cartesi_machine_path = opt.machine_dir.clone();
    let cartesi_machine_url = opt.cartesi_machine_url.clone();
    let db_file = opt.db_file.clone();
    let compute_only = opt.compute_only.clone();
    let appchain = opt.appchain.clone();
    let ipfs_url = opt.ipfs_url.clone();

    let executor_options = ExecutorOptions {
        sequencer_url: opt.sequencer_url.clone(),
    };
    let addr = ([0, 0, 0, 0], 3033).into();

    let context = Arc::new(opt);
    let service = make_service_fn(|_| {
        let options = Arc::clone(&context);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let options = Arc::clone(&options);
                async move {
                    let path = req.uri().path().to_owned();
                    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                    Ok::<_, Infallible>(request_handler(req, &segments, options).await)
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(Box::new(service));
    let appchain = Cid::try_from(appchain).unwrap().to_bytes();
    if compute_only {
        tracing::info!("Compute only");
        server.await;
    } else {
        tracing::info!("Runs with subscribe");

        join!(
            subscribe(
                &executor_options,
                cartesi_machine_url,
                cartesi_machine_path.as_str(),
                ipfs_url.as_str(),
                db_file,
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
    options: Arc<lambada::Options>,
) -> Response<Body> {
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
                let cartesi_machine_url = options.cartesi_machine_url.clone();
                let ipfs_url = options.ipfs_url.as_str();
                let connection = sqlite::open(options.db_file.clone()).unwrap();
                let cartesi_machine_path = options.machine_dir.as_str();

                let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
                    .await
                    .unwrap();
                let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());
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
                        let json_response = serde_json::json!({
                            "cid": Cid::try_from(cid).unwrap().to_string(),
                        });
                        let json_response = serde_json::to_string(&json_response).unwrap();

                        let response = Response::new(Body::from(json_response));

                        return response;
                    }
                    Err(e) => {
                        let json_error = serde_json::json!({
                            "error": e.to_string(),
                        });
                        let json_error = serde_json::to_string(&json_error).unwrap();
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from(json_error))
                            .unwrap();
                    }
                }
            } else {
                let json_error = serde_json::json!({
                    "error": "request header should be application/octet-streams",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
        }
        (hyper::Method::GET, ["health"]) => {
            let json_request = r#"{"healthy": "true"}"#;
            let response = Response::new(Body::from(json_request));
            response
        }
        (hyper::Method::GET, ["latest"]) => {
            if options.compute_only {
                let json_error = serde_json::json!({
                    "error": "in compute only mode",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let connection = sqlite::open(options.db_file.clone()).unwrap();
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
                return response;
            }
            let json_error = serde_json::json!({
                "error": "failed to get last block",
            });
            let json_error = serde_json::to_string(&json_error).unwrap();
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(json_error))
                .unwrap();
        }
        (hyper::Method::GET, ["block", height_number]) => {
            if options.compute_only {
                let json_error = serde_json::json!({
                    "error": "in compute only mode",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let connection = sqlite::open(options.db_file.clone()).unwrap();
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
                return response;
            }
            let json_error = serde_json::json!({
                "error": "failed to receive block",
            });
            let json_error = serde_json::to_string(&json_error).unwrap();
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(json_error))
                .unwrap();
        }
        (hyper::Method::POST, ["submit"]) => {
            if options.compute_only {
                let json_error = serde_json::json!({
                    "error": "in compute only mode",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            if reqest.headers().get("content-type")
                == Some(&hyper::header::HeaderValue::from_static(
                    "application/octet-stream",
                ))
            {
                let https = HttpsConnector::new();
                let client = Client::builder().build::<_, hyper::Body>(https);

                let uri = format!("{}submit/submit", options.sequencer_url.clone())
                    .parse()
                    .unwrap();

                let data: Vec<u8> = hyper::body::to_bytes(reqest.into_body())
                    .await
                    .unwrap()
                    .to_vec();
                let appchain = options.appchain.clone();

                let ipfs_client = IpfsClient::from_str(options.ipfs_url.clone().as_str()).unwrap();
                let chain_info = ipfs_client
                    .cat(&(appchain + "/app/chain-info.json"))
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();

                let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
                    .expect("error reading chain-info.json file");

                let chain_vm_id: u64 = chain_info
                    .get("sequencer")
                    .unwrap()
                    .get("vm-id")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();

                tracing::info!("data for submitting {:?}", data.clone());

                let submit_data: Data = Data {
                    payload: data,
                    vm: chain_vm_id,
                };

                let mut req = Request::new(Body::from(bincode::serialize(&submit_data).unwrap()));
                *req.uri_mut() = uri;
                let mut map = HeaderMap::new();
                map.insert(
                    header::CONTENT_TYPE,
                    "application/octet-stream".parse().unwrap(),
                );
                *req.headers_mut() = map;
                *req.method_mut() = Method::POST;

                let response = client.request(req).await.unwrap();
                return response;
            } else {
                let json_error = serde_json::json!({
                    "error": "request header should be application/octet-streams",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
        }
        _ => {
            let json_error = serde_json::json!({
                "error": "Invalid request",
            });
            let json_error = serde_json::to_string(&json_error).unwrap();
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(json_error))
                .unwrap();
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Data {
    vm: u64,
    payload: Vec<u8>,
}