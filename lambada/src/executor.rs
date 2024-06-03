use sha3::{Digest, Sha3_256};

use cid::Cid;
use futures_util::TryStreamExt;
use hyper::Request;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use serde::{Deserialize, Serialize};
use sqlite::State;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
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
pub async fn subscribe(opt: ExecutorOptions, appchain: Cid) {
    tracing::info!("starting subscribe() of {:?}", appchain.to_string());
    let mut current_cid = appchain.clone();
    let genesis_cid_text = current_cid.to_string();
    let mut current_height: u64 = u64::MAX;
    let ipfs_client = IpfsClient::from_str(&opt.ipfs_url).unwrap();

    // Set what our current chain info is, so we can notice later on if it changes
    let current_chain_info_cid: Arc<Mutex<Option<Cid>>> =
        Arc::new(Mutex::new(get_chain_info_cid(&opt, current_cid).await));
    if *current_chain_info_cid.lock().unwrap() == None {
        tracing::debug!("not chain info found, leaving");
        return;
    }
    tracing::info!("starting subscribe loop of {:?}", appchain.to_string());
    let mut chain_info = ipfs_client
        .cat(&format!("{}/gov/chain-info.json", current_cid.to_string()))
        .map_ok(|chunk| chunk.to_vec())
        .try_concat()
        .await
        .unwrap();

    // enter subscription loop
    loop {
        {
            let connection = sqlite::Connection::open_thread_safe(format!(
                "{}/chains/{}",
                opt.db_path, genesis_cid_text
            ))
            .unwrap();
            let query = "
        CREATE TABLE IF NOT EXISTS blocks (state_cid BLOB(48) NOT NULL,
        height INTEGER NOT NULL, sequencer_block_reference BLOB(48) NOT NULL, finalized BOOL NOT NULL );
    ";
            connection.execute(query).unwrap();

            let chain_info = serde_json::from_slice::<serde_json::Value>(&chain_info)
                .expect("error reading chain-info.json file");

            let starting_block_height: u64 = chain_info
                .get("sequencer")
                .unwrap()
                .get("height")
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();

            let mut initial_block_height: i64 = starting_block_height as i64 - 1;
            if initial_block_height < 0 {
                initial_block_height = 0
            };

            let mut statement = connection
                .prepare("SELECT * FROM blocks ORDER BY height DESC LIMIT 1")
                .unwrap();

            if let Ok(State::Row) = statement.next() {
                let height = statement.read::<i64, _>("height").unwrap() as u64;
                let cid =
                    Cid::try_from(statement.read::<Vec<u8>, _>("state_cid").unwrap()).unwrap();
                tracing::info!(
                    "persisted state of chain {:?} is height {:?} = CID {:?}",
                    genesis_cid_text,
                    height,
                    cid.to_string()
                );
                current_cid = cid;
                current_height = height;
            } else {
                tracing::info!("new chain, not persisted: {:?}", genesis_cid_text);
                let mut statement = connection
                    .prepare("INSERT INTO blocks (state_cid, height, sequencer_block_reference, finalized) VALUES (?, ?, ?, ?)")
                    .unwrap();
                statement
                    .bind((1, &current_cid.to_bytes() as &[u8]))
                    .unwrap();
                statement.bind((2, initial_block_height as i64)).unwrap();
                statement.bind((3, "")).unwrap();
                statement.bind((4, 1)).unwrap();
                statement.next().unwrap();
            }
        }
        // Set up subscription: read what sequencer and (if we don't know it already)
        chain_info = ipfs_client
            .cat(&format!("{}/gov/chain-info.json", current_cid.to_string()))
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

        let starting_block_height: u64 = chain_info
            .get("sequencer")
            .unwrap()
            .get("height")
            .unwrap()
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();

        let chain_vm_id = chain_info
            .get("sequencer")
            .unwrap()
            .get("vm-id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        if current_height == u64::MAX {
            current_height = starting_block_height;
        }

        if current_height < starting_block_height {
            tracing::error!(
            "Current height less than starting block height in chain info, should not be possible"
        );
            return;
        }

        tracing::info!("iterating through blocks from height {:?}", current_height);

        let r#type: String = chain_info
            .get("sequencer")
            .unwrap()
            .get("type")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        tracing::info!("chain type: {:?}", r#type);
        match r#type.as_str() {
            "avail" => {
                let input = SubscribeInput {
                    height: current_height,
                    opt: opt.clone(),
                    current_cid: current_cid.to_bytes(),
                    chain_info_cid: current_chain_info_cid.lock().unwrap().unwrap().to_bytes(),
                    chain_vm_id: chain_vm_id.clone(),
                    genesis_cid_text: genesis_cid_text.clone(),
                };
                let subscribe_avail = String::from("/bin/subscribe-avail");

                let subscribe_avail_child = Command::new(subscribe_avail)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let subscribe_avail_child = Arc::new(Mutex::new(subscribe_avail_child));
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&input).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());
                {
                    let mut subscribe_avail_child_stdin = subscribe_avail_child.lock().unwrap();

                    let subscribe_avail_child_stdin = subscribe_avail_child_stdin
                        .stdin
                        .as_mut()
                        .expect("Failed to open avail binary stdin");

                    subscribe_avail_child_stdin
                        .write_all(&execute_parameter_bytes)
                        .unwrap();
                }

                //Waits for avail child to finish
                let subscribe_avail_child = Arc::clone(&subscribe_avail_child);
                loop {
                    let mut child = subscribe_avail_child.lock().unwrap();
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            break;
                        }
                        _ => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
            "espresso" => {
                let input = SubscribeInput {
                    height: current_height,
                    opt: opt.clone(),
                    current_cid: current_cid.to_bytes(),
                    chain_info_cid: current_chain_info_cid.lock().unwrap().unwrap().to_bytes(),
                    chain_vm_id: chain_vm_id.clone(),
                    genesis_cid_text: genesis_cid_text.clone(),
                };
                let subscribe_espresso = String::from("/bin/subscribe-espresso");

                let subscribe_espresso_child = Command::new(subscribe_espresso)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let subscribe_espresso_child = Arc::new(Mutex::new(subscribe_espresso_child));
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&input).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                {
                    let mut subscribe_espresso_child_stdin =
                        subscribe_espresso_child.lock().unwrap();
                    let subscribe_espresso_child_stdin = subscribe_espresso_child_stdin
                        .stdin
                        .as_mut()
                        .expect("Failed to open espresso binary stdin");

                    subscribe_espresso_child_stdin
                        .write_all(&execute_parameter_bytes)
                        .unwrap();
                }
                //Waits for espresso child to finish
                let subscribe_espresso_child = Arc::clone(&subscribe_espresso_child);
                loop {
                    let mut child = subscribe_espresso_child.lock().unwrap();
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            break;
                        }
                        _ => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
            "celestia" => {
                let input = SubscribeInput {
                    height: current_height,
                    opt: opt.clone(),
                    current_cid: current_cid.to_bytes(),
                    chain_info_cid: current_chain_info_cid.lock().unwrap().unwrap().to_bytes(),
                    chain_vm_id: chain_vm_id.clone(),
                    genesis_cid_text: genesis_cid_text.clone(),
                };
                let subscribe_celestia = String::from("/bin/subscribe-celestia");

                let subscribe_celestia_child = Command::new(subscribe_celestia)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let subscribe_celestia_child = Arc::new(Mutex::new(subscribe_celestia_child));
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&input).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                let mut subscribe_celestia_child_stdin = subscribe_celestia_child.lock().unwrap();
                let subscribe_celestia_child_stdin = subscribe_celestia_child_stdin
                    .stdin
                    .as_mut()
                    .expect("Failed to open celestia subscribe stdin");

                subscribe_celestia_child_stdin
                    .write_all(&execute_parameter_bytes)
                    .unwrap();

                //Waits for celestia child to finish
                let subscribe_celestia_child = Arc::clone(&subscribe_celestia_child);
                loop {
                    let mut child = subscribe_celestia_child.lock().unwrap();
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            break;
                        }
                        _ => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
            "evm-da" => {
                let input = SubscribeInput {
                    height: current_height,
                    opt: opt.clone(),
                    current_cid: current_cid.to_bytes(),
                    chain_info_cid: current_chain_info_cid.lock().unwrap().unwrap().to_bytes(),
                    chain_vm_id: chain_vm_id.clone(),
                    genesis_cid_text: genesis_cid_text.clone(),
                };
                let subscribe_evm_da = String::from("/bin/subscribe-evm-da");

                let subscribe_evm_da_child = Command::new(subscribe_evm_da)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let subscribe_evm_da_child = Arc::new(Mutex::new(subscribe_evm_da_child));
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&input).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                let mut subscribe_evm_da_child_stdin = subscribe_evm_da_child.lock().unwrap();
                let subscribe_evm_da_child_stdin = subscribe_evm_da_child_stdin
                    .stdin
                    .as_mut()
                    .expect("Failed to open celestia subscribe stdin");

                subscribe_evm_da_child_stdin
                    .write_all(&execute_parameter_bytes)
                    .unwrap();

                //Waits for evm-da child to finish
                let subscribe_evm_da_child = Arc::clone(&subscribe_evm_da_child);
                loop {
                    let mut child = subscribe_evm_da_child.lock().unwrap();
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            break;
                        }
                        _ => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }

            "evm-blocks" => {
                let input = SubscribeInput {
                    height: current_height,
                    opt: opt.clone(),
                    current_cid: current_cid.to_bytes(),
                    chain_info_cid: current_chain_info_cid.lock().unwrap().unwrap().to_bytes(),
                    chain_vm_id: chain_vm_id.clone(),
                    genesis_cid_text: genesis_cid_text.clone(),
                };
                let subscribe_evm_blocks = String::from("/bin/subscribe-evm-blocks");

                let subscribe_evm_blocks_child = Command::new(subscribe_evm_blocks)
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let subscribe_evm_blocks_child = Arc::new(Mutex::new(subscribe_evm_blocks_child));
                let mut execute_parameter_bytes = Vec::new();
                let data_json = serde_json::to_string(&input).unwrap();
                execute_parameter_bytes.extend(data_json.as_bytes().len().to_le_bytes());
                execute_parameter_bytes.extend(data_json.as_bytes().to_vec());

                let mut subscribe_evm_blocks_child_stdin =
                    subscribe_evm_blocks_child.lock().unwrap();
                let subscribe_evm_blocks_child_stdin = subscribe_evm_blocks_child_stdin
                    .stdin
                    .as_mut()
                    .expect("Failed to open celestia subscribe stdin");

                subscribe_evm_blocks_child_stdin
                    .write_all(&execute_parameter_bytes)
                    .unwrap();

                //Waits for evm-blocks child to finish
                let subscribe_evm_blocks_child = Arc::clone(&subscribe_evm_blocks_child);
                loop {
                    let mut child = subscribe_evm_blocks_child.lock().unwrap();
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            break;
                        }
                        _ => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
            _ => tracing::info!("Unknown sequencer type: {}", r#type),
        }
    }
}

async fn get_chain_info_cid(opt: &ExecutorOptions, current_cid: Cid) -> Option<Cid> {
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

pub fn calculate_sha256(input: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    hasher.update(input);
    hasher.finalize().to_vec()
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct BincodedCompute {
    pub metadata: HashMap<Vec<u8>, Vec<u8>>,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubscribeResponse {
    finished: bool,
}
