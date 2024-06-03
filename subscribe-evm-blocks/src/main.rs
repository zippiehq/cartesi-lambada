use async_compatibility_layer::logging::setup_backtrace;
use async_compatibility_layer::logging::setup_logging;
use async_std::task;
use cid::Cid;
use ethers::prelude::*;
use ethers::types::{BlockId, BlockNumber};
use hyper::{header, Body, Client, Method, Request, Uri};
use nix::libc;
use os_pipe::PipeReader;
use os_pipe::{dup_stdin, dup_stdout};
use polling::{Event, Events, Poller};
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::thread;
use std::time::Duration;
use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::time::sleep;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExecutorOptions {
    pub espresso_testnet_sequencer_url: String,
    pub celestia_testnet_sequencer_url: String,
    pub avail_testnet_sequencer_url: String,
    pub ipfs_url: String,
    pub ipfs_write_url: String,
    pub db_path: String,
    pub server_address: String,
    pub evm_da_url: String,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SubscribeInput {
    pub height: u64,
    pub opt: ExecutorOptions,
    pub current_cid: Vec<u8>,
    pub chain_info_cid: Vec<u8>,
    pub chain_vm_id: String,
    pub genesis_cid_text: String,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BincodedCompute {
    pub metadata: HashMap<Vec<u8>, Vec<u8>>,
    pub payload: Vec<u8>,
}
#[async_std::main]
async fn main() {
    let my_stdout =
        File::create("/tmp/evm-blocks-stdout.log").expect("Failed to create stdout file");
    let my_stderr =
        File::create("/tmp/evm-blocks-stderr.log").expect("Failed to create stderr file");
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
    let stdin = dup_stdin().unwrap();
    let poller = Poller::new().unwrap();
    let key = &stdin.as_fd().as_raw_fd();
    unsafe {
        poller
            .add(&stdin.as_fd(), Event::readable(key.clone() as usize))
            .unwrap()
    };
    let mut events = Events::new();
    loop {
        events.clear();
        poller
            .wait(&mut events, Some(Duration::from_millis(100)))
            .unwrap();

        for ev in events.iter() {
            if ev.key == key.clone() as usize {
                if let Ok(parameter) = read_message(&stdin) {
                    tracing::info!(" after second read");
                    let time_after_execute = SystemTime::now();
                    let subscribe_input =
                        serde_json::from_slice::<SubscribeInput>(&parameter).unwrap();
                    poller
                        .modify(&stdin.as_fd(), Event::readable(key.clone() as usize))
                        .unwrap();
                    subscribe_evm_blocks(
                        subscribe_input.height,
                        subscribe_input.opt,
                        &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
                        subscribe_input.genesis_cid_text,
                    )
                    .await;
                }
            }
        }
    }
}

async fn subscribe_evm_blocks(
    starting_block_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    genesis_cid_text: String,
) {
    let eth_rpc_url = opt.evm_da_url.clone();
    let eth_client = Arc::new(
        ethers::providers::Provider::<ethers::providers::Http>::try_from(&eth_rpc_url)
            .expect("Could not instantiate Ethereum HTTP Provider"),
    );
    let mut current_height = starting_block_height;
    println!("before current height");
    while current_height < u64::MAX {
        let latest_block = eth_client
            .get_block_number()
            .await
            .expect("Failed to fetch the latest block number");
        tracing::info!(
            "EVM-BLOCKS: latest block {} current height {}",
            latest_block,
            current_height
        );
        println!(
            "EVM-BLOCKS: latest block {} current height {}",
            latest_block, current_height
        );

        if latest_block < current_height.into() {
            tracing::info!(
                "Waiting for block number {} to be available. Current latest block number: {}",
                current_height,
                latest_block
            );
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();

        let block = eth_client
            .get_block(BlockId::Number(BlockNumber::Number(current_height.into())))
            .await
            .expect("Failed to fetch block")
            .expect("Block not found");

        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, current_height as i64)).unwrap();

        tracing::info!("Processing block at height {}", current_height);
        let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        metadata.insert(b"sequencer".to_vec(), b"evm-blocks".to_vec());
        metadata.insert(
            b"ethereum-block-height".to_vec(),
            current_height.to_string().as_bytes().to_vec(),
        );
        metadata.insert(
            b"ethereum-block-hash".to_vec(),
            format!("{:?}", block.hash).as_bytes().to_vec(),
        );
        if let Ok(state) = statement.next() {
            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
            if state == State::Done {
                handle_tx(
                    opt.clone(),
                    Some(Vec::new()),
                    current_cid,
                    metadata,
                    current_height,
                    genesis_cid_text.clone(),
                    hex::encode(block.hash.unwrap().0),
                )
                .await;

                current_height += 1;
            }
        }
    }
}

async fn handle_tx(
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
    println!("handle_tx");

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
    let connection = sqlite::Connection::open_thread_safe(format!(
        "{}/chains/{}",
        opt.db_path, genesis_cid_text
    ))
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

pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}
