pub mod executor;
use crate::executor::BincodedCompute;
use async_compatibility_layer::logging::setup_backtrace;
use async_compatibility_layer::logging::setup_logging;
use cid::Cid;
use clap::Parser;
use hyper::{header, Body, Client, Method, Request};
use nix::libc;
use os_pipe::dup_stdin;
#[cfg(not(feature = "no_sqlite"))]
use sqlite::Connection;
#[cfg(not(feature = "no_sqlite"))]
use sqlite::State;
#[cfg(feature = "no_sqlite")]
use sqlite_no_default::Connection;
#[cfg(feature = "no_sqlite")]
use sqlite_no_default::State;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::Mutex;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutorOptions {
    pub ipfs_url: String,
    pub ipfs_write_url: String,
    pub db_path: String,
    pub server_address: String,
    pub sequencer_map: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SubscribeInput {
    pub height: u64,
    pub opt: ExecutorOptions,
    pub current_cid: Vec<u8>,
    pub current_chain_info_cid: Vec<u8>,
    pub chain_vm_id: String,
    pub genesis_cid_text: String,
    pub network_type: String,
}

pub fn setup_subscriber(sequencer: &str) -> Option<SubscribeInput> {
    let genesis_cid = &env::args().collect::<Vec<_>>()[1];
    let log_directory_path: String =
        std::env::var("LAMBADA_LOGS_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let my_stdout = File::create(format!(
        "{}/{}-{}-stdout.log",
        log_directory_path, genesis_cid, sequencer
    ))
    .expect("Failed to create stdout file");
    let my_stderr = File::create(format!(
        "{}/{}-{}-stderr.log",
        log_directory_path, genesis_cid, sequencer
    ))
    .expect("Failed to create stderr file");
    let stdout_fd = my_stdout.as_raw_fd();
    let stderr_fd = my_stderr.as_raw_fd();
    unsafe {
        libc::close(1);
        libc::close(2);
        libc::dup2(stdout_fd, 1);
        libc::dup2(stderr_fd, 2);
    }
    setup_logging();
    setup_backtrace();
    let mut stdin = dup_stdin().unwrap();
    if let Ok(parameter) = read_message(&mut stdin) {
        return Some(serde_json::from_slice::<SubscribeInput>(&parameter).unwrap());
    }
    return None;
}
pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}

pub async fn trigger_callback_for_newblock(
    options: Arc<ExecutorOptions>,
    genesis_block_cid: &str,
    block_height: u64,
    state_cid: &str,
) {
    let connection =
        Connection::open_thread_safe(format!("{}/subscriptions.db", options.db_path)).unwrap();
    let mut statement = connection
        .prepare("SELECT callback_url FROM block_callbacks WHERE genesis_block_cid = ?")
        .unwrap();
    statement.bind((1, genesis_block_cid)).unwrap();

    while let Ok(State::Row) = statement.next() {
        let callback_url = statement.read::<String, _>(0).unwrap();
        let payload = serde_json::json!({
            "appchain": genesis_block_cid,
            "block_height": block_height,
            "state_cid": state_cid
        });

        let client = Client::new();
        let req = Request::builder()
            .method(Method::POST)
            .uri(callback_url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let _ = client.request(req).await;
    }
}

pub async fn get_chain_info_cid(opt: &ExecutorOptions, current_cid: Cid) -> Option<Cid> {
    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "{}/api/v0/dag/resolve?arg={}/gov/{}",
            opt.ipfs_url,
            current_cid.to_string(),
            "/chain-info.json"
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let client = Arc::new(hyper::Client::new());
    match client.request(req).await {
        Ok(res) => {
            let response_cid_value = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(res).await.expect("no cid").to_vec(),
            )
            .unwrap();

            let response_cid_value = Cid::try_from(
                response_cid_value
                    .get("Cid")
                    .unwrap()
                    .get("/")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            Some(response_cid_value)
        }
        Err(_) => None,
    }
}

pub async fn is_chain_info_same(
    opt: ExecutorOptions,
    current_cid: Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
) -> bool {
    let new_chain_info = get_chain_info_cid(&opt, current_cid).await;
    if new_chain_info == None {
        tracing::error!("No chain info found, leaving");
        return false;
    }
    if new_chain_info != *current_chain_info_cid.lock().unwrap() {
        *current_chain_info_cid.lock().unwrap() = new_chain_info;
        tracing::error!("No support for changing chain info yet");
        return false;
    }

    return true;
}
pub async fn handle_tx(
    opt: ExecutorOptions,
    data: Option<Vec<u8>>,
    current_cid: &mut Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    height: u64,
    genesis_cid_text: String,
    block_hash: String,
) {
    let mut bincoded_compute_data: BincodedCompute = BincodedCompute {
        metadata: metadata.clone(),
        payload: vec![],
    };
    if let Some(payload) = data.clone() {
        bincoded_compute_data.payload = payload;
    }

    let req = Request::builder()
        .method("POST")
        .header("Content-Type", "application/octet-stream")
        .uri(format!(
            "http://{}/compute/{}?bincoded=true",
            opt.server_address,
            current_cid.to_string()
        ))
        .body(Body::from(
            bincode::serialize(&bincoded_compute_data).unwrap(),
        ))
        .unwrap();
    let client = hyper::Client::new();
    match client.request(req).await {
        Ok(result) => {
            let cid = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(result)
                    .await
                    .expect("/compute failed with no response")
                    .to_vec(),
            )
            .expect("/compute failed with no response");
            let cid = Cid::try_from(cid.get("cid").unwrap().as_str().unwrap()).unwrap();
            tracing::info!("old current_cid {:?}", cid.clone());
            *current_cid = cid;
            tracing::info!("resulted current_cid {:?}", current_cid.clone());
        }
        Err(e) => {
            tracing::info!("no output: {:?}", e);
        }
    }

    // XXX Is this right? Shouldn't this be after processing all tx'es?
    let connection =
        Connection::open_thread_safe(format!("{}/chains/{}", opt.db_path, genesis_cid_text))
            .unwrap();

    let mut statement = connection
        .prepare(
            "INSERT INTO blocks (state_cid, height, sequencer_block_reference, finalized) VALUES (?, ?, ?, ?)",
        )
        .unwrap();
    statement
        .bind((1, &current_cid.to_bytes() as &[u8]))
        .unwrap();
    statement.bind((2, height as i64)).unwrap();
    statement.bind((3, &block_hash as &str)).unwrap();
    statement.bind((4, 1)).unwrap();
    statement.next().unwrap();
}
#[derive(Parser, Clone, Debug)]
pub struct Options {
    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "db_path", default_value = "db/")]
    pub db_path: String,

    #[clap(long, env = "IPFS_URL")]
    pub ipfs_url: String,

    #[clap(long, env = "IPFS_WRITE_URL")]
    pub ipfs_write_url: String,

    #[clap(long, env = "AUTOMATIC_SUBSCRIBE", default_value = "")]
    pub automatic_subscribe: String,

    #[clap(long, env = "SEQUENCER_MAP", default_value = "")]
    pub sequencer_map: String,
}
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use committable::{Commitment, Committable};
use derive_more::{Display, Into};
use serde::{Deserialize, Serialize};
#[derive(
    Serialize,
    Deserialize,
    Ord,
    Display,
    PartialOrd,
    PartialEq,
    Eq,
    Hash,
    Debug,
    CanonicalDeserialize,
    CanonicalSerialize,
    Default,
    Clone,
    Copy,
    Into,
)]
#[display(fmt = "{_0}")]
pub struct NamespaceId(u64);

impl From<u64> for NamespaceId {
    fn from(number: u64) -> Self {
        Self(number)
    }
}
#[derive(Serialize, Deserialize)]
pub struct EspressoTransaction {
    namespace: NamespaceId,
    #[serde(with = "base64_bytes")]
    payload: Vec<u8>,
}

impl Committable for EspressoTransaction {
    fn commit(&self) -> Commitment<Self> {
        committable::RawCommitmentBuilder::new("Transaction")
            .u64_field("namespace", self.namespace.into())
            .var_size_bytes(&self.payload)
            .finalize()
    }

    fn tag() -> String {
        "TX".into()
    }
}

impl EspressoTransaction {
    pub fn new(namespace: NamespaceId, payload: Vec<u8>) -> Self {
        Self { namespace, payload }
    }
}
