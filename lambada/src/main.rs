use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use async_std::sync::Mutex;
use async_std::task;
use cartesi_lambda::{execute, lambada_worker_subprocess};
use celestia_rpc::BlobClient;
use celestia_types::blob::GasPrice;
use celestia_types::nmt::Namespace;
use celestia_types::Blob;
use cid::Cid;
use clap::Parser;
use commit::Committable;
use futures::TryStreamExt;
use hyper::body::to_bytes;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper::{StatusCode, Uri};
use hyper_tls::HttpsConnector;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use lambada::executor::BincodedCompute;
use lambada::executor::{calculate_sha256, subscribe, ExecutorOptions};
use lambada::Options;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
async fn start_subscriber(options: Arc<lambada::Options>, cid: Cid, server_address: String) {
    let executor_options = ExecutorOptions {
        espresso_testnet_sequencer_url: options.espresso_testnet_sequencer_url.clone(),
        celestia_testnet_sequencer_url: options.celestia_testnet_sequencer_url.clone(),
        avail_testnet_sequencer_url: options.avail_testnet_sequencer_url.clone(),
        ipfs_url: options.ipfs_url.clone(),
        ipfs_write_url: options.ipfs_write_url.clone(),
        db_path: options.db_path.clone(),
        server_address,
        evm_da_url: options.evm_da_url.clone(),
    };

    tracing::info!("Subscribing to appchain {:?}", cid.to_string());
    thread::spawn(move || {
        tracing::info!("in thread");
        let _ = task::block_on(async {
            subscribe(executor_options, cid).await;
        });
        tracing::info!("out of thread");
    });
}

async fn get_attestation<T: AsRef<[u8]>>(user_data: T) -> Vec<u8> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let uri = "http://localhost:7777/v1/attestation".parse().unwrap();

    let req_data = AttestationUserData {
        user_data: base64::encode(user_data),
    };

    let mut req = Request::new(Body::from(serde_json::json!(req_data).to_string()));

    *req.uri_mut() = uri;
    *req.method_mut() = Method::POST;

    let response = client.request(req).await.unwrap();
    to_bytes(response.into_body()).await.unwrap().to_vec()
}
#[derive(Debug, Serialize, Deserialize)]
struct AttestationUserData {
    user_data: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct AttestationResponse {
    data: String,
    attestation_doc: String,
}

#[async_std::main]
async fn main() {
    setup_logging();
    setup_backtrace();

    let opt = Options::parse();
    let subscriptions = Vec::<Cid>::new();
    let addr: SocketAddr = ([0, 0, 0, 0], 3033).into();
    let context = Arc::new(opt);
    let subscriptions = Arc::new(Mutex::new(subscriptions));
    let return_callback_ids: Arc<Mutex<HashMap<u64, Option<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let (tx, rx) = mpsc::channel();

    let tx = Arc::new(Mutex::new(tx));
    let rx = Arc::new(Mutex::new(rx));

    lambada_worker_subprocess();

    // Make sure database is initalized and then load subscriptions from database
    {
        let connection =
            sqlite::Connection::open_thread_safe(format!("{}/subscriptions.db", context.db_path))
                .unwrap();
        let query = "
            CREATE TABLE IF NOT EXISTS subscriptions (appchain_cid BLOB(48) NOT NULL);
            CREATE TABLE IF NOT EXISTS block_callbacks (
                genesis_block_cid BLOB NOT NULL,
                callback_url TEXT NOT NULL);";
        connection.execute(query).unwrap();

        let mut statement = connection.prepare("SELECT * FROM subscriptions").unwrap();
        let mut automatic_hit = false;
        while let Ok(State::Row) = statement.next() {
            let cid = Cid::try_from(statement.read::<Vec<u8>, _>("appchain_cid").unwrap()).unwrap();
            if context.automatic_subscribe != "" {
                let automatic_cid = Cid::try_from(context.automatic_subscribe.clone()).unwrap();
                if automatic_cid.cmp(&cid) == Ordering::Equal {
                    automatic_hit = true;
                }
            }

            subscriptions.lock().await.push(cid);
            start_subscriber(Arc::clone(&context), cid, addr.to_string()).await;
        }
        // XXX this doesn't persist it in database
        if !automatic_hit && context.automatic_subscribe != "" {
            let cid = Cid::try_from(context.automatic_subscribe.clone()).unwrap();
            subscriptions.lock().await.push(cid);
            start_subscriber(Arc::clone(&context), cid, addr.to_string()).await;
        }
    }

    let service = make_service_fn(|_| {
        let options = Arc::clone(&context);
        let subscriptions = Arc::clone(&subscriptions);
        let return_callback_ids = Arc::clone(&return_callback_ids);
        let rx = Arc::clone(&rx);
        let tx = Arc::clone(&tx);

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let options = Arc::clone(&options);
                let subscriptions = Arc::clone(&subscriptions);
                let return_callback_ids = Arc::clone(&return_callback_ids);
                let rx = Arc::clone(&rx);
                let tx = Arc::clone(&tx);

                async move {
                    let path = req.uri().path().to_owned();
                    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                    Ok::<_, Infallible>(
                        request_handler(
                            return_callback_ids,
                            addr,
                            req,
                            &segments,
                            options,
                            subscriptions,
                            tx,
                            rx,
                        )
                        .await,
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
    ids: Arc<Mutex<HashMap<u64, Option<String>>>>,
    server_address: SocketAddr,
    request: Request<Body>,
    segments: &[&str],
    options: Arc<lambada::Options>,
    subscriptions: Arc<Mutex<Vec<Cid>>>,
    tx: Arc<Mutex<Sender<(u64, Option<String>)>>>,
    rx: Arc<Mutex<Receiver<(u64, Option<String>)>>>,
) -> Response<Body> {
    let mut parsed_query: HashMap<String, String> = HashMap::new();
    if let Some(query) = request.uri().query() {
        parsed_query = query
            .split('&')
            .filter_map(|part| {
                let mut split_iter = part.splitn(2, '=');
                let key = split_iter.next()?.to_owned();
                let value = split_iter.next().unwrap_or("").to_owned();
                Some((key, value))
            })
            .collect();
    }
    match (request.method().clone(), segments) {
        (hyper::Method::GET, ["chain_info_template", sequencer]) => {
            let random_number: u64 = rand::thread_rng().gen();
            if *sequencer != "espresso" {
                let json_error = serde_json::json!({
                    "error": "Only espresso supported right now",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let https = HttpsConnector::new();
            let client = Client::builder().build::<_, hyper::Body>(https);

            let uri: String = format!(
                "{}/v0/status/block-height",
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

            let json_response = serde_json::json!({
               "sequencer": {"type": "espresso", "height": height.to_string(), "vm-id": random_number.to_string()},
            });
            let json_response = serde_json::to_string(&json_response).unwrap();

            return Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(json_response))
                .unwrap();
        }
        (hyper::Method::POST, ["compute", cid]) => {
            if request.headers().get("content-type")
                == Some(&hyper::header::HeaderValue::from_static(
                    "application/octet-stream",
                ))
            {
                let mut bincoded = false;
                for query in parsed_query {
                    match query.0.as_str() {
                        "bincoded" => match query.1.parse::<bool>() {
                            Ok(bincoded_opt) => {
                                bincoded = bincoded_opt;
                            }
                            Err(e) => {
                                tracing::info!("bincoded should be boolean type");
                                let json_error = serde_json::json!({
                                    "error": e.to_string(),
                                });
                                let json_error = serde_json::to_string(&json_error).unwrap();
                                return Response::builder()
                                    .status(StatusCode::BAD_REQUEST)
                                    .body(Body::from(json_error))
                                    .unwrap();
                            }
                        },
                        _ => {}
                    };
                }

                let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::<Vec<u8>, Vec<u8>>::new();

                metadata.insert(
                    calculate_sha256("sequencer".as_bytes()),
                    calculate_sha256("compute".as_bytes()),
                );

                let mut data = hyper::body::to_bytes(request.into_body())
                    .await
                    .unwrap()
                    .to_vec();

                if bincoded {
                    let bincoded_data: BincodedCompute = bincode::deserialize(&data).unwrap();
                    data = bincoded_data.payload;
                    metadata = bincoded_data.metadata;
                }

                match compute(
                    Some(data),
                    options.clone(),
                    cid.to_string(),
                    None,
                    metadata,
                    None,
                )
                .await
                {
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
        (Method::POST, ["subscribe_to_callbacks", genesis_block_cid]) => {
            let body_bytes = hyper::body::to_bytes(request.into_body()).await.unwrap();
            let callback_url = String::from_utf8(body_bytes.to_vec()).unwrap();
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/subscriptions.db",
                options.db_path
            ))
            .unwrap();
            let mut statement = connection
                .prepare(
                    "INSERT INTO block_callbacks (genesis_block_cid, callback_url) VALUES (?, ?)",
                )
                .unwrap();
            statement.bind((1, *genesis_block_cid)).unwrap();
            statement.bind((2, callback_url.as_str()));
            statement.next().unwrap();

            Response::new(Body::from("Subscribed successfully"))
        }

        (Method::DELETE, ["unsubscribe_from_callbacks", genesis_block_cid]) => {
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/subscriptions.db",
                options.db_path
            ))
            .unwrap();
            let mut statement = connection
                .prepare("DELETE FROM block_callbacks WHERE genesis_block_cid = ?")
                .unwrap();
            statement.bind((1, *genesis_block_cid)).unwrap();
            statement.next().unwrap();

            Response::new(Body::from("Unsubscribed successfully"))
        }
        (hyper::Method::POST, ["compute_with_callback", cid]) => {
            let cid = Arc::new(cid.to_string());

            let mut callback_uri: std::result::Result<Uri, std::string::String> =
                Err("Callback parameter wasn't set".to_string());
            let mut max_cycles: Option<u64> = None;
            let mut only_warmup: bool = false;
            let mut bincoded: bool = false;
            let mut identifier: Option<String> = None;

            // replaced with bincoded metadata if available
            let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::<Vec<u8>, Vec<u8>>::new();

            metadata.insert(
                calculate_sha256("sequencer".as_bytes()),
                calculate_sha256("compute".as_bytes()),
            );

            for query in parsed_query {
                match query.0.as_str() {
                    "callback" => {
                        match query.1.parse::<hyper::Uri>() {
                            Ok(uri) => {
                                callback_uri = Ok(uri);
                            }
                            Err(e) => {
                                callback_uri = Err(e.to_string());
                            }
                        };
                    }
                    "max_cycles" => {
                        match query.1.parse::<u64>() {
                            Ok(cycles) => {
                                max_cycles = Some(cycles);
                            }
                            Err(e) => {
                                tracing::info!("max_cycles should be u64 type");
                                let json_error = serde_json::json!({
                                    "error": e.to_string(),
                                });
                                let json_error = serde_json::to_string(&json_error).unwrap();
                                return Response::builder()
                                    .status(StatusCode::BAD_REQUEST)
                                    .body(Body::from(json_error))
                                    .unwrap();
                            }
                        };
                    }
                    "identifier" => {
                        identifier = Some(query.1.clone());
                    }
                    "only_warmup" => {
                        match query.1.parse::<bool>() {
                            Ok(warmup) => {
                                only_warmup = warmup;
                            }
                            Err(e) => {
                                tracing::info!("only_warmup should be boolean type");
                                let json_error = serde_json::json!({
                                    "error": e.to_string(),
                                });
                                let json_error = serde_json::to_string(&json_error).unwrap();
                                return Response::builder()
                                    .status(StatusCode::BAD_REQUEST)
                                    .body(Body::from(json_error))
                                    .unwrap();
                            }
                        };
                    }
                    "bincoded" => {
                        match query.1.parse::<bool>() {
                            Ok(bincoded_opt) => {
                                bincoded = bincoded_opt;
                            }
                            Err(e) => {
                                tracing::info!("bincoded should be boolean type");
                                let json_error = serde_json::json!({
                                    "error": e.to_string(),
                                });
                                let json_error = serde_json::to_string(&json_error).unwrap();
                                return Response::builder()
                                    .status(StatusCode::BAD_REQUEST)
                                    .body(Body::from(json_error))
                                    .unwrap();
                            }
                        };
                    }
                    _ => {}
                }
            }
            let callback_uri = match callback_uri {
                Ok(uri) => Arc::new(uri),
                Err(e) => {
                    tracing::info!("{}", e.to_string());
                    let json_error = serde_json::json!({
                        "error": e.to_string(),
                    });
                    let json_error = serde_json::to_string(&json_error).unwrap();
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(json_error))
                        .unwrap();
                }
            };
            if callback_uri.scheme().is_none() {
                let json_error = serde_json::json!({
                    "error": "callback address should be in an absolute form",
                });
                let json_error = serde_json::to_string(&json_error).unwrap();
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(json_error))
                    .unwrap();
            }
            let mut data = None;

            if !only_warmup && !bincoded {
                data = Some(
                    hyper::body::to_bytes(request.into_body())
                        .await
                        .unwrap()
                        .to_vec(),
                );
            } else if bincoded {
                let real_data = hyper::body::to_bytes(request.into_body())
                    .await
                    .unwrap()
                    .to_vec();
                let bincoded_data: BincodedCompute = bincode::deserialize(&real_data).unwrap();
                data = Some(bincoded_data.payload);
                metadata = bincoded_data.metadata;
            }

            thread::spawn(move || {
                let _ = task::block_on(async {
                    match compute(
                        data.clone(),
                        options.clone(),
                        cid.to_string(),
                        max_cycles,
                        metadata.clone(),
                        identifier,
                    )
                    .await
                    {
                        Ok(resulted_cid) => {
                            send_callback(
                                data,
                                CallbackData::ComputeOutput(resulted_cid.to_string()),
                                cid.to_string(),
                                callback_uri,
                                metadata.clone(),
                            )
                            .await;
                        }
                        Err(e) => {
                            send_callback(
                                data,
                                CallbackData::Error(e.to_string()),
                                cid.to_string(),
                                callback_uri,
                                metadata.clone(),
                            )
                            .await;
                        }
                    }
                });
            });
            let json_request = r#"{"ok": "true"}"#;
            let response = Response::new(Body::from(json_request));
            response
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
            start_subscriber(options, cid, server_address.to_string()).await;
            let json_request = r#"{"ok": "true"}"#;
            let response = Response::new(Body::from(json_request));
            response
        }
        // XXX: does not handle the case where tx was sent before the start of the block height in the chain
        (hyper::Method::GET, ["find_inclusion", appchain, tx_hash]) => {
            let https = HttpsConnector::new();
            let client = Client::builder().build::<_, hyper::Body>(https);

            let uri: String = format!(
                "{}/availability/transaction/hash/{}",
                options.espresso_testnet_sequencer_url.clone(),
                tx_hash
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

            let json_tx = serde_json::from_slice::<serde_json::Value>(&body_bytes).unwrap();
            match json_tx["transaction"]["vm"].as_u64() {
                Some(vm_id) => {
                    let ipfs_client = IpfsClient::from_str(&options.ipfs_url).unwrap();
                    match ipfs_client
                        .cat(&(appchain.to_string() + "/gov/chain-info.json"))
                        .map_ok(|chunk| chunk.to_vec())
                        .try_concat()
                        .await
                    {
                        Ok(chain_info) => {
                            let chain_info =
                                serde_json::from_slice::<serde_json::Value>(&chain_info)
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

                            let height = json_tx["height"].as_i64().unwrap();

                            if vm_id == chain_vm_id {
                                let json_response = serde_json::json!({
                                    "height": height,
                                });
                                let json_response = serde_json::to_string(&json_response).unwrap();
                                return Response::builder()
                                    .body(Body::from(json_response))
                                    .unwrap();
                            }
                        }
                        Err(_) => {}
                    }
                }
                None => {}
            }

            let json_error = serde_json::json!({
                "error": "not found",
            });
            let json_error = serde_json::to_string(&json_error).unwrap();
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(json_error))
                .unwrap();
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
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/chains/{}",
                options.db_path, appchain
            ))
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
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/chains/{}",
                options.db_path, appchain
            ))
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
            if request.headers().get("content-type")
                == Some(&hyper::header::HeaderValue::from_static(
                    "application/octet-stream",
                ))
            {
                let ipfs_client = IpfsClient::from_str(&options.ipfs_url).unwrap();

                let chain_info = ipfs_client
                    .cat(&(appchain.to_string() + "/gov/chain-info.json"))
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
                let data: Vec<u8> = hyper::body::to_bytes(request.into_body())
                    .await
                    .unwrap()
                    .to_vec();
                tracing::info!("chain type: {:?}", r#type);
                match r#type.as_str() {
                    /* "espresso" => {
                        let ipfs_client =
                            IpfsClient::from_str(options.ipfs_url.clone().as_str()).unwrap();
                        let chain_info = ipfs_client
                            .cat(&(appchain.to_string() + "/gov/chain-info.json"))
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

                        let txn = Transaction::new(chain_vm_id.into(), data.clone());
                        let url = format!("{}", options.espresso_testnet_sequencer_url.clone())
                            .parse()
                            .unwrap();
                        let client: surf_disco::Client<tide_disco::error::ServerError> =
                            surf_disco::Client::new(url);

                        let res = client
                            .post::<()>("submit/submit")
                            .body_binary(&txn)
                            .unwrap()
                            .send()
                            .await;

                        tracing::info!("submitting result {:?}", res);
                        
                        let json_response = serde_json::json!({
                            "hash": txn.commit(),
                        });
                        let json_response = serde_json::to_string(&json_response).unwrap();
                        return Response::builder().body(Body::from(json_response)).unwrap();
                    } */
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
                        match client.blob_submit(&blobs, GasPrice::default()).await {
                            Ok(height) => {
                                let json_submitting_result = serde_json::json!({
                                    "height": height,
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
                            "error": "unknown sequencer type",
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

pub async fn compute(
    data: Option<Vec<u8>>,
    options: Arc<lambada::Options>,
    cid: String,
    max_cycles: Option<u64>,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    identifier: Option<String>,
) -> Result<Cid, std::io::Error> {
    let ipfs_url = options.ipfs_url.as_str();
    let ipfs_write_url = options.ipfs_write_url.as_str();

    tracing::info!("cid {:?}", cid);
    let state_cid = Cid::try_from(cid.clone()).unwrap();

    execute(
        ipfs_url,
        ipfs_write_url,
        data,
        state_cid,
        metadata,
        max_cycles,
        identifier,
    )
    .await
}

async fn send_callback(
    body: Option<Vec<u8>>,
    callback_data: CallbackData,
    cid: String,
    callback_uri: Arc<Uri>,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
) {
    let mut data = serde_json::Value::Null;
    let body = body.unwrap_or_default();
    match callback_data {
        CallbackData::ComputeOutput(resulted_cid) => {
            data = serde_json::json!({
                "output": resulted_cid,
                "hash": sha256::digest(body),
                "state_cid": cid,
                "metadata": metadata.iter().map( |(k, v) | (hex::encode(&k), hex::encode(&v))).collect::<HashMap<String, String>>(),
            });
        }
        CallbackData::Error(error) => {
            data = serde_json::json!({
                "error": error.to_string(),
                "hash": sha256::digest(body),
                "state_cid": cid,
                "metadata": metadata.iter().map( |(k, v) | (hex::encode(&k), hex::encode(&v))).collect::<HashMap<String, String>>(),
            });
        }
    }

    let mut json_data = serde_json::to_string(&data).unwrap();
    match std::env::var("PROVIDE_NITRO_ATTESTATION") {
        Ok(provide_nitro_attestation) => {
            if provide_nitro_attestation.parse().unwrap() {
                let data = hyper::body::to_bytes(json_data.clone())
                    .await
                    .expect("invalid cid response from callback")
                    .to_vec();

                let attestation_doc = get_attestation(data.clone()).await;
                let result = AttestationResponse {
                    data: hex::encode(data),
                    attestation_doc: hex::encode(attestation_doc),
                };
                json_data = serde_json::to_string(&result).unwrap();
            }
        }
        Err(_) => {}
    };

    let mut req = Request::new(Body::from(json_data.clone()));
    *req.uri_mut() = callback_uri.as_ref().clone();
    let mut map = HeaderMap::new();
    map.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    *req.headers_mut() = map;
    *req.method_mut() = Method::POST;
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    match client.request(req).await {
        Ok(_) => {}
        Err(e) => {
            tracing::info!("{}", e.to_string());
        }
    };
}

#[derive(Serialize, Deserialize)]
struct Data {
    vm: u64,
    payload: Vec<u8>,
}

enum CallbackData {
    Error(String),
    ComputeOutput(String),
}
