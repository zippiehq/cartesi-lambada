use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cartesi_machine_json_rpc::client::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use cid::Cid;
use sequencer::L1BlockInfo;
use serde_json::Value;
use sqlite::State;
use std::fs::File;
use std::io::Cursor;
use hyper::Request;

pub const MACHINE_IO_ADDRESSS: u64 = 0x90000000000000;
const READ_BLOCK: u64 = 0x00001;
const EXCEPTION: u64 = 0x00002;
const LOAD_TX: u64 = 0x00003;
const FINISH: u64 = 0x00004;
const WRITE_BLOCK: u64 = 0x000005;
const LOAD_APP: u64 = 0x00006;
const HINT: u64 = 0x00007;

pub async fn execute(
    machine_url: String,
    cartesi_machine_path: &str,
    ipfs_url: &str,
    payload: Vec<u8>,
    state_cid: Vec<u8>,
    block_info: &L1BlockInfo,
) -> Result<Vec<u8>, std::io::Error> {

    tracing::info!("state cid {:?}", Cid::try_from(state_cid.clone()).unwrap().to_string());

    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "http://127.0.0.1:5001/api/v0/dag/resolve?arg={}/app",
            Cid::try_from(state_cid.clone()).unwrap().to_string()
        ))
        .body(hyper::Body::empty())
        .unwrap();

    let mut app_cid = String::new();
    let client = hyper::Client::new();

    match client.request(req).await {
        Ok(res) => {
            let app_cid_value = serde_json::from_slice::<serde_json::Value>(
                &hyper::body::to_bytes(res).await.expect("no cid").to_vec(),
            )
            .unwrap();

            app_cid = app_cid_value
                .get("Cid")
                .unwrap()
                .get("/")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
        }
        Err(e) => {
            panic!("{}", e)
        }
    }
    tracing::info!("app cid {:?}", Cid::try_from(app_cid.clone()).unwrap().to_string());

    let client = IpfsClient::from_str(ipfs_url).unwrap();
    tracing::info!("execute");

    let mut machine = JsonRpcCartesiMachineClient::new(machine_url).await.unwrap();
    tracing::info!(
        "app_cid {}",
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    );
    tracing::info!(
        "state_cid {}",
        Cid::try_from(state_cid.clone()).unwrap().to_string()
    );

    let mut machine_loaded_state = 0;

    if std::path::Path::new(&format!(
        "/data/snapshot/ipfs_using2_{}",
        Cid::try_from(app_cid.clone()).unwrap().to_string()
    ))
    .is_dir()
    {
        tracing::info!(
            "loading machine from /data/snapshot/ipfs_using2_{}",
            Cid::try_from(app_cid.clone()).unwrap().to_string()
        );
        machine
            .load_machine(
                &format!(
                    "/data/snapshot/ipfs_using2_{}",
                    Cid::try_from(app_cid.clone()).unwrap().to_string()
                ),
                &MachineRuntimeConfig {
                    skip_root_hash_check: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        machine_loaded_state = 2;
        tracing::info!("read iflag y {:?}", machine.read_iflags_y().await.unwrap());
    } else if std::path::Path::new(&format!("/data/snapshot/ipfs_using2",)).exists() {
        machine
            .load_machine(
                "/data/snapshot/ipfs_using2",
                &MachineRuntimeConfig {
                    skip_root_hash_check: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        machine_loaded_state = 1;
    } else {
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
    }

    loop {
        let mut interpreter_break_reason =  Value::Null;
        if !machine.read_iflags_y().await.unwrap() {
            interpreter_break_reason = machine.run(u64::MAX).await.unwrap();
        }
        let hex_encoded = hex::encode(
            machine
                .read_memory(MACHINE_IO_ADDRESSS, 1024)
                .await
                .unwrap(),
        );

        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let opt = u64::from_be_bytes(read_opt_be_bytes.try_into().unwrap());
        tracing::info!("before handling iflag y {:?}", machine.read_iflags_y().await.unwrap());

        match opt {
            READ_BLOCK => {
                tracing::info!("READ_BLOCK");
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid = Cid::try_from(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                        .await
                        .unwrap(),
                )
                .unwrap();

                tracing::info!("read cid {:?}", cid.to_string());

                let block = client
                    .block_get(cid.to_string().as_str())
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();
                tracing::info!("block len {:?}", block.len());

                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 16, STANDARD.encode(block.clone()))
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(block.len().to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                tracing::info!("read_block info was written");

            }
            EXCEPTION => {
                tracing::info!("HTIF_YIELD_REASON_TX_EXCEPTION");
                machine.destroy().await.unwrap();
                machine.shutdown().await.unwrap();
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "exception"));
            }
            LOAD_TX => {
                tracing::info!("LOAD_TX");
                if machine_loaded_state == 0 || machine_loaded_state == 1 {
                
                    let app_cid: cid::CidGeneric<64> = Cid::try_from(app_cid.clone()).unwrap();
                    tracing::info!(
                        "load tx to dir: {} and read iflag : {}",
                        format!("/data/snapshot/ipfs_using2_{}", app_cid.clone().to_string()),
                        machine.read_iflags_y().await.unwrap()
                    );

                    machine
                        .store(&format!(
                            "/data/snapshot/ipfs_using2_{}",
                            app_cid.clone().to_string(),
                        ))
                        .await
                        .unwrap();
                }
                let current_cid = Cid::try_from(state_cid.clone()).unwrap();
                let cid_length = current_cid.clone().to_bytes().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(current_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();

                let payload_length = payload.clone().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16 + cid_length,
                        STANDARD.encode(payload_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 24 + cid_length,
                        STANDARD.encode(payload.clone()),
                    )
                    .await
                    .unwrap();

                let block_number = block_info.number.to_be_bytes();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 24 + cid_length + payload_length,
                        STANDARD.encode(block_number.to_vec()),
                    )
                    .await
                    .unwrap();

                let mut block_timestamp = vec![0; 32];

                block_info.timestamp.to_big_endian(&mut block_timestamp);

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 32 + cid_length + payload_length,
                        STANDARD.encode(block_timestamp.to_vec()),
                    )
                    .await
                    .unwrap();

                let hash = block_info.hash.0;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS
                            + 32
                            + block_timestamp.len() as u64
                            + cid_length
                            + payload_length,
                        STANDARD.encode(hash.to_vec()),
                    )
                    .await
                    .unwrap();

                tracing::info!("load_tx info was written");
            }
            FINISH => {
                tracing::info!("FINISH");

                let status = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let data = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 24, length)
                    .await
                    .unwrap();

                match status {
                    0 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        println!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        tracing::info!(
                            "FINISH received cid {}",
                            Cid::try_from(data.clone()).unwrap()
                        );

                        return Ok(data);
                    }
                    1 => {
                        tracing::info!("HTIF_YIELD_REASON_RX_REJECTED");
                        println!("HTIF_YIELD_REASON_RX_REJECTED");
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "transaction was rejected",
                        ));
                    }
                    _ => {
                        machine.destroy().await.unwrap();
                        machine.shutdown().await.unwrap();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "unknown status",
                        ));
                    }
                }
            }
            WRITE_BLOCK => {
                tracing::info!("WRITE_BLOCK");
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let memory = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                let data = Cursor::new(memory.clone());
                tracing::info!("data written to block {:?}", memory.clone());
                let put_response = client.block_put(data).await.unwrap();
                tracing::info!("put_response key {:?}", put_response.key);

            }
            LOAD_APP => {
                tracing::info!("LOAD_APP");
                if machine_loaded_state == 0 {
                    tracing::info!("load app to dir: /data/snapshot/ipfs_using2 and read iflag : {}", machine.read_iflags_y().await.unwrap());
                    machine.store("/data/snapshot/ipfs_using2").await.unwrap();
                }
                let app_cid: cid::CidGeneric<64> = Cid::try_from(app_cid.clone()).unwrap();

                tracing::info!("app cid {:?}", Cid::try_from(app_cid.clone()).unwrap());
                let cid_length = app_cid.clone().to_bytes().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS,
                        STANDARD.encode(cid_length.to_be_bytes().to_vec()),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        STANDARD.encode(app_cid.clone().to_bytes()),
                    )
                    .await
                    .unwrap();
                tracing::info!("load_app info was written");

            }
            HINT => {
                tracing::info!("HINT");

                let payload_length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let payload = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, payload_length)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                tracing::info!("hint payload {:?}", payload);
            }
            _ => {
                tracing::info!("unknown opt {:?}", opt)
            }
        }
        if interpreter_break_reason == Value::String("halted".to_string()) {
            tracing::info!("halted");
            machine.destroy().await.unwrap();
            machine.shutdown().await.unwrap();
        }
        machine.reset_iflags_y().await.unwrap();
    }
}
