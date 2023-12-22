use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use async_std::sync::Mutex;
use async_std::task;
use cartesi_lambda::execute;
use cartesi_machine_json_rpc::client::JsonRpcCartesiMachineClient;
use celestia_rpc::BlobClient;
use celestia_types::blob::SubmitOptions;
use celestia_types::nmt::Namespace;
use celestia_types::Blob;
use cid::Cid;
use clap::Parser;
use ethers::prelude::*;
use futures::TryStreamExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::StatusCode;
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper_tls::HttpsConnector;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use lambada::executor::{subscribe, ExecutorOptions};
use lambada::Options;
use rand::Rng;
use sequencer::L1BlockInfo;
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::convert::Infallible;
use std::io::Cursor;
use std::sync::Arc;

async fn start_subscriber(options: Arc<lambada::Options>, cid: Cid) {
    let executor_options = ExecutorOptions {
        espresso_testnet_sequencer_url: options.espresso_testnet_sequencer_url.clone(),
        celestia_testnet_sequencer_url: options.celestia_testnet_sequencer_url.clone(),
        ipfs_url: options.ipfs_url.clone(),
        db_path: options.db_path.clone(),
        base_cartesi_machine_path: options.machine_dir.clone(),
    };
    let cartesi_machine_url = options.cartesi_machine_url.to_string().clone();

    tracing::info!("Subscribing to appchain {:?}", cid.to_string());
    task::spawn(async move {
        let _ = task::block_on(subscribe(
            executor_options,
            cartesi_machine_url.clone(),
            Cid::try_from(cid).unwrap(),
        ));
    });
}

#[async_std::main]
async fn main() {
    setup_logging();
    setup_backtrace();

    let opt = Options::parse();
    let subscriptions = Vec::<Cid>::new();
    let addr = ([0, 0, 0, 0], 3033).into();

    let context = Arc::new(opt);
    let subscriptions = Arc::new(Mutex::new(subscriptions));
    // Make sure database is initalized and then load subscriptions from database
    {
        let connection =
            sqlite::Connection::open_thread_safe(format!("{}/subscriptions.db", context.db_path))
                .unwrap();
        let query = "
            CREATE TABLE IF NOT EXISTS subscriptions (appchain_cid BLOB(48) NOT NULL);";
        connection.execute(query).unwrap();

        let mut statement = connection.prepare("SELECT * FROM subscriptions").unwrap();

        while let Ok(State::Row) = statement.next() {
            let cid = Cid::try_from(statement.read::<Vec<u8>, _>("appchain_cid").unwrap()).unwrap();
            subscriptions.lock().await.push(cid);
            start_subscriber(Arc::clone(&context), cid).await;
        }
    }

    let service = make_service_fn(|_| {
        let options = Arc::clone(&context);
        let subscriptions = Arc::clone(&subscriptions);
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let options = Arc::clone(&options);
                let subscriptions = Arc::clone(&subscriptions);

                async move {
                    let path = req.uri().path().to_owned();
                    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                    Ok::<_, Infallible>(
                        request_handler(req, &segments, options, subscriptions).await,
                    )
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(Box::new(service));
    tracing::info!("Cartesi Lambada listening on {}", addr);
    server.await.unwrap();
}

async fn request_handler(
    reqest: Request<Body>,
    segments: &[&str],
    options: Arc<lambada::Options>,
    subscriptions: Arc<Mutex<Vec<Cid>>>,
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

                let cartesi_machine_path = options.machine_dir.as_str();

                let machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url.to_string())
                    .await
                    .unwrap();
                let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());
                let state_cid = Cid::try_from(cid.to_string()).unwrap();
                let block_info: &L1BlockInfo = &L1BlockInfo {
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
        (hyper::Method::GET, ["subscribe", appchain]) => {
            if subscriptions
                .lock()
                .await
                .contains(&Cid::try_from(appchain.to_string().clone()).unwrap())
            {
                let json_error = serde_json::json!({
                    "error": "subscription already exists",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let cid = Cid::try_from(appchain.to_string().clone()).unwrap();
            subscriptions.lock().await.push(cid);
            {
                let connection = sqlite::Connection::open_thread_safe(format!(
                    "{}/subscriptions.db",
                    options.db_path
                ))
                .unwrap();
                let mut statement = connection
                    .prepare("INSERT INTO subscriptions (appchain_cid) VALUES (?)")
                    .unwrap();
                statement.bind((1, &cid.to_bytes() as &[u8])).unwrap();
                statement.next().unwrap();
            }
            start_subscriber(options, cid).await;
            let json_request = r#"{"ok": "true"}"#;
            let response = Response::new(Body::from(json_request));
            response
        }
        (hyper::Method::GET, ["latest", appchain]) => {
            if !subscriptions
                .lock()
                .await
                .contains(&Cid::try_from(appchain.to_string().clone()).unwrap())
            {
                let json_error = serde_json::json!({
                    "error": "no such subscription",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let connection =
                sqlite::Connection::open_thread_safe(format!("{}/{}", options.db_path, appchain))
                    .unwrap();
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
        // XXX temporary for RWA demo
        (hyper::Method::GET, ["new", app_cid]) => {
            let random_number: u64 = rand::thread_rng().gen();

            let ipfs_client = IpfsClient::from_str(options.ipfs_url.clone().as_str()).unwrap();
            ipfs_client
                .files_mkdir(&format!("/new-{}", random_number), false)
                .await
                .unwrap();
            ipfs_client
                .files_cp(
                    &format!("/ipfs/{}", app_cid),
                    &format!("/new-{}/app", random_number),
                )
                .await
                .unwrap();
            let chain_info = ipfs_client
                .files_read(&format!("/new-{}/app/chain-info.json", random_number))
                .map_ok(|chunk| chunk.to_vec())
                .try_concat()
                .await
                .unwrap();

            let https = HttpsConnector::new();
            let client = Client::builder().build::<_, hyper::Body>(https);

            let uri: String = format!(
                "{}/status/latest_block_height",
                options.espresso_testnet_sequencer_url.clone()
            )
            .parse()
            .unwrap();

            let block_req = Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap();
            let block_response = client.request(block_req).await.unwrap();
            let body_bytes = hyper::body::to_bytes(block_response).await.unwrap();
            let height = String::from_utf8_lossy(&body_bytes)
                .trim()
                .parse::<u64>()
                .unwrap();

            let mut chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
                .expect("error reading chain-info.json file");

            chain_info.get_mut("sequencer").unwrap()["vm-id"] =
                serde_json::Value::from(random_number.to_string());

            chain_info.get_mut("sequencer").unwrap()["height"] =
                serde_json::Value::from(height.to_string());

            let chain_info = serde_json::to_vec(&chain_info).unwrap();
            ipfs_client
                .files_rm(
                    &format!("/new-{}/app/chain-info.json", random_number),
                    false,
                )
                .await
                .unwrap();
            ipfs_client
                .files_write(
                    &format!("/new-{}/app/chain-info.json", random_number),
                    true,
                    true,
                    Cursor::new(chain_info),
                )
                .await
                .unwrap();

            let app_cid = ipfs_client
                .files_stat(&format!("/new-{}", random_number))
                .await
                .unwrap()
                .hash;
            let json_response = serde_json::json!({
                "cid": app_cid
            });

            let json_response = serde_json::to_string(&json_response).unwrap();
            let response = Response::new(Body::from(json_response));
            return response;
        }
        (hyper::Method::GET, ["block", appchain, height_number]) => {
            if !subscriptions
                .lock()
                .await
                .contains(&Cid::try_from(appchain.to_string().clone()).unwrap())
            {
                let json_error = serde_json::json!({
                    "error": "no such subscription",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let connection =
                sqlite::Connection::open_thread_safe(format!("{}/{}", options.db_path, appchain))
                    .unwrap();
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
        (hyper::Method::POST, ["submit", appchain]) => {
            if !subscriptions
                .lock()
                .await
                .contains(&Cid::try_from(appchain.to_string().clone()).unwrap())
            {
                let json_error = serde_json::json!({
                    "error": "no such subscription",
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
                let ipfs_client = IpfsClient::from_str(&options.ipfs_url).unwrap();

                let chain_info = ipfs_client
                    .cat(&(appchain.to_string() + "/app/chain-info.json"))
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();

                tracing::info!(
                    "chain info {}",
                    String::from_utf8(chain_info.clone()).unwrap()
                );

                let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
                    .expect("error reading chain-info.json file");
                let r#type: String = chain_info
                    .get("sequencer")
                    .unwrap()
                    .get("type")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let data: Vec<u8> = hyper::body::to_bytes(reqest.into_body())
                    .await
                    .unwrap()
                    .to_vec();
                tracing::info!("chain type: {:?}", r#type);
                match r#type.as_str() {
                    "espresso" => {
                        let https = HttpsConnector::new();
                        let client = Client::builder().build::<_, hyper::Body>(https);

                        let uri = format!(
                            "{}/submit/submit",
                            options.espresso_testnet_sequencer_url.clone()
                        )
                        .parse()
                        .unwrap();

                        let ipfs_client =
                            IpfsClient::from_str(options.ipfs_url.clone().as_str()).unwrap();
                        let chain_info = ipfs_client
                            .cat(&(appchain.to_string() + "/app/chain-info.json"))
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

                        let mut req =
                            Request::new(Body::from(bincode::serialize(&submit_data).unwrap()));
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
                    }
                    "celestia" => {
                        let token = match std::env::var("CELESTIA_TESTNET_NODE_AUTH_TOKEN_WRITE") {
                            Ok(token) => token,
                            Err(_) => {
                                let json_error = serde_json::json!({
                                    "error": "Token was not specified",
                                });
                                let json_error = serde_json::to_string(&json_error).unwrap();
                                return Response::builder()
                                    .status(StatusCode::BAD_REQUEST)
                                    .body(Body::from(json_error))
                                    .unwrap();
                            }
                        };
                        let client = celestia_rpc::Client::new(
                            options.celestia_testnet_sequencer_url.as_str(),
                            Some(token.as_str()),
                        )
                        .await
                        .unwrap();
                        let chain_vm_id: u64 = chain_info
                            .get("sequencer")
                            .unwrap()
                            .get("vm-id")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .parse::<u64>()
                            .unwrap();
                        let blob =
                            Blob::new(Namespace::new_v0(&chain_vm_id.to_be_bytes()).unwrap(), data)
                                .unwrap();
                        let blobs = [blob];
                        match client.blob_submit(&blobs, SubmitOptions::default()).await {
                            Ok(_) => {
                                let json_submitting_result = serde_json::json!({
                                    "result": "blob was submitted",
                                });
                                let json_submitting_result =
                                    serde_json::to_string(&json_submitting_result).unwrap();
                                return Response::builder()
                                    .body(Body::from(json_submitting_result))
                                    .unwrap();
                            }
                            Err(e) => {
                                let json_submitting_result = serde_json::json!({
                                    "result": e.to_string(),
                                });
                                let json_submitting_result =
                                    serde_json::to_string(&json_submitting_result).unwrap();
                                return Response::builder()
                                    .status(StatusCode::CONFLICT)
                                    .body(Body::from(json_submitting_result))
                                    .unwrap();
                            }
                        }
                    }
                    _ => {
                        let json_error = serde_json::json!({
                            "error": "runknown sequencer type",
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
