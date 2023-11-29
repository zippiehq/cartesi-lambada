use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use blocks_stream::executor::{subscribe, ExecutorOptions};
use blocks_stream::Options;
use cartesi_lambda::execute;
use clap::Parser;
use futures::join;
use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Client, HeaderMap, Method, Request, Response, Server};
use hyper_tls::HttpsConnector;
use jsonrpc_cartesi_machine::JsonRpcCartesiMachineClient;
use jsonrpc_cartesi_machine::MachineRuntimeConfig;
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::fmt::format;
use std::process::Command;
use cid::Cid;
#[async_std::main]
async fn main() {
    let output = Command::new("sh")
        .arg("program/gen_machine_simple.sh")
        .output()
        .expect("Failed to execute program/gen_machine_simple.sh");
    setup_logging();
    setup_backtrace();
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::info!("Script output: {}", stdout);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::info!("Script execution failed: {}", stderr);
    }

    let opt = Options::parse();
    let connection = sqlite::open(opt.db_dir.clone()).unwrap();
    let query = "
    CREATE TABLE IF NOT EXISTS blocks (hash BLOB(32) NOT NULL,
    height INTEGER NOT NULL);
";
    connection.execute(query).unwrap();
    let cartesi_machine_path = opt.machine_dir.as_str();
    let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
    let ipfs_url = "http://127.0.0.1:5001";
    let vm_id = opt.vm_id;
    let min_block_height = opt.height;

    let executor_options = ExecutorOptions {
        hotshot_address: opt.hotshot_address,
        l1_provider: opt.l1_provider.clone(),
        sequencer_url: opt.sequencer_url.clone(),
    };
    let addr = ([0, 0, 0, 0], 3033).into();

    let service = make_service_fn(|_| async move {
        Ok::<_, hyper::Error>(service_fn(|req| async {
            let output = request_handler(req).await;
            match output {
                Ok(res) => Ok(res),
                Err(e) => Err(e.to_string()),
            }
        }))
    });

    let server = Server::bind(&addr).serve(Box::new(service));
    let state_cid = Cid::try_from(opt.state_cid).unwrap().to_bytes();

    join!(
        subscribe(
            &executor_options,
            cartesi_machine_url,
            cartesi_machine_path,
            ipfs_url,
            vm_id,
            min_block_height,
            opt.db_dir,
            state_cid
        ),
        server,
    );
}

async fn request_handler(reqest: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (reqest.method().clone(), reqest.uri().path()) {
        (hyper::Method::POST, "/compute") => {
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
                let cartesi_machine_path = "/machines/txnotice";
                let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
                let ipfs_url = "http://127.0.0.1:5001";
                let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
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
                let ipfs_url = "http://127.0.0.1:5001";
                //execute(&mut machine, ipfs_url, data, 1, 1234, 0, connection).await;
                let connection = sqlite::open("sequencer_db").unwrap();

                let query = "SELECT * FROM transactions where type = 'notice' AND block_height=1234 AND transaction_index=0 ";
                let mut statement = connection.prepare(query).unwrap();
                let mut result: Vec<u8> = Vec::new();
                if let Ok(State::Row) = statement.next() {
                    result = statement.read::<Vec<u8>, _>("data").unwrap();
                }
                let response = Response::new(Body::from(result));
                return Ok(response);
            }
            panic!("Invalid request")
        }
        (hyper::Method::GET, "/health") => {
            let json_request = r#"{"healthy": "true"}"#;
            let response = Response::new(Body::from(json_request));
            Ok(response)
        }
        (hyper::Method::POST, "/submit") => {
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
