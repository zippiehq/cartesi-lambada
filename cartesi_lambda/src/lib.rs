use async_std::task;
use cid::Cid;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
// How we communicate between host (this) and guest (the cartesi machine) is through read-writing the second flash drive
// which is placed in memory at MACHINE_IO_ADDRESSS. Special care is needed on machine side to flush/ignore OS caches until PMEM/DAX comes around.
//
// The guest writes certain operations along with serialized parameters into memory, does an automatic yield, and the host handles the operation
// and resumes its execution after clearing yield flag.
//
// The model we are assuming here:
// - a base machine image, that'll boot up
// - signal LOAD_APP (give me an app CID),
// - and the app will signal LOAD_TX (give me a payload and state CID + app CID (for double checking it's running the right image))
// - and result in FINISH (accept/reject) with a new state CID or EXCEPTION
// - halting or out of cycles is a failed tx

pub const MACHINE_IO_ADDRESSS: u64 = 0x90000000000000;

pub static mut LAMBADA_WORKER_TX: Option<Arc<Mutex<Sender<ExecuteResultSender>>>> = None;

// execute is the entry point for the computation to be done, we have a particular state CID with it's associated /app directory
// and a transaction payload + metadata and we want to do this computation and get the new state CID back or an error
pub async fn execute(
    ipfs_url: &str,
    ipfs_write_url: &str,
    payload: Option<Vec<u8>>,
    state_cid: Cid,
    metadata: HashMap<Vec<u8>, Vec<u8>>,
    max_cycles_input: Option<u64>,
    identifier: Option<String>,
) -> Result<Cid, std::io::Error> {
    let identifier = match identifier {
        Some(id) => id,
        None => rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect(),
    };

    let execute_parameter = ExecuteParameters {
        ipfs_url: ipfs_url.to_string(),
        ipfs_write_url: ipfs_write_url.to_string(),
        payload: payload.clone(),
        state_cid: state_cid.to_bytes(),
        metadata: metadata
            .iter()
            .map(|(k, v)| (hex::encode(&k), hex::encode(&v)))
            .collect::<HashMap<String, String>>(),
        max_cycles_input: max_cycles_input,
        identifier: identifier.clone(),
    };
    tracing::info!("Sending ExecuteParameter to the lambada_worker subprocess");

    let (result_sender, result_receiver) = mpsc::channel();

    let execute_result_sender = ExecuteResultSender {
        parameters: execute_parameter,
        result_sender: Arc::new(Mutex::new(result_sender)),
    };
    unsafe {
        LAMBADA_WORKER_TX
            .clone()
            .unwrap()
            .lock()
            .unwrap()
            .send(execute_result_sender)
            .unwrap();
        tracing::info!("Waiting for result from the lambada_worker subprocess");

        let response = result_receiver.recv().unwrap();

        tracing::info!("Response was received inside execute");

        match response.result {
            Ok(cid) => return Ok(Cid::try_from(cid).unwrap()),
            Err(e) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ));
            }
        }
    }
}

pub fn calculate_sha256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecuteParameters {
    ipfs_url: String,
    ipfs_write_url: String,
    payload: Option<Vec<u8>>,
    state_cid: Vec<u8>,
    metadata: HashMap<String, String>,
    max_cycles_input: Option<u64>,
    identifier: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExecuteResult {
    pub result: Result<Vec<u8>, serde_error::Error>,
    pub identifier: String,
}

pub struct ExecuteResultSender {
    pub parameters: ExecuteParameters,
    pub result_sender: Arc<Mutex<Sender<ExecuteResult>>>,
}
pub fn read_message<T>(pipe: &mut T) -> Result<Vec<u8>, std::io::Error>
where
    T: std::io::Read,
{
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}

fn init() -> Child {
    let mut lambada_worker_path = String::from("/lambada-worker");
    if let Ok(path) = std::env::var("LAMBADA_WORKER") {
        lambada_worker_path = path;
    }

    let lambada_worker_execution = Command::new(lambada_worker_path)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    return lambada_worker_execution;
}

pub fn lambada_worker_subprocess() {
    // Channel for sharing inputs and outputs between execute function and lambada_worker running subprocess
    let (lambada_worker_tx, lambada_worker_rx) = mpsc::channel();

    unsafe {
        LAMBADA_WORKER_TX = Some(Arc::new(Mutex::new(lambada_worker_tx)));
    }
    let mut child = init();
    let sender_hash_map: Arc<Mutex<HashMap<String, Arc<Mutex<Sender<ExecuteResult>>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let sender_hash_map_insert: Arc<Mutex<HashMap<String, Arc<Mutex<Sender<ExecuteResult>>>>>> =
        sender_hash_map.clone();
    let sender_hash_map_get: Arc<Mutex<HashMap<String, Arc<Mutex<Sender<ExecuteResult>>>>>> =
        sender_hash_map.clone();

    thread::spawn(move || {
        let _ = task::block_on(async {
            loop {
                let execute_result_sender: ExecuteResultSender = lambada_worker_rx.recv().unwrap();
                tracing::info!(
                    "Writing the input to the lambada_worker and adding sender to the hashmap"
                );
                sender_hash_map_insert.lock().unwrap().insert(
                    execute_result_sender.parameters.identifier.clone(),
                    execute_result_sender.result_sender,
                );
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&execute_result_sender.parameters).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                let lambada_worker_stdin = child
                    .stdin
                    .as_mut()
                    .expect("Failed to open lambada_worker stdin");

                lambada_worker_stdin
                    .write_all(&execute_parameter_bytes)
                    .unwrap();
                tracing::info!("Input was written to the lambada_worker");
            }
        });
    });

    thread::spawn(move || {
        let _ = task::block_on(async {
            loop {
                let lambada_worker_stdout = child.stdout.as_mut().unwrap();

                if let Ok(execute_result) = read_message(lambada_worker_stdout) {
                    let response =
                        serde_json::from_slice::<ExecuteResult>(&execute_result).unwrap();
                    for (key, sender) in sender_hash_map_get.lock().unwrap().clone().into_iter() {
                        if &key == &response.identifier {
                            sender.lock().unwrap().send(response).unwrap();
                            break;
                        }
                    }
                    tracing::info!("Response was sent from thread subprocess");
                }
            }
        });
    });
}
