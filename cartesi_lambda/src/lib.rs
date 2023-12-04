use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};

use cid::Cid;
use jsonrpc_cartesi_machine::JsonRpcCartesiMachineClient;
use serde_json::Value;
use std::io::Cursor;

pub const MACHINE_IO_ADDRESSS: u64 = 0x90000000000000;
const READ_BLOCK: u64 = 0x00001;
const EXCEPTION: u64 = 0x00002;
const LOAD_TX: u64 = 0x00003;
const FINISH: u64 = 0x00004;
const WRITE_BLOCK: u64 = 0x000005;
const LOAD_APP: u64 = 0x00006;
const HINT: u64 = 0x00007;

pub async fn execute(
    machine: &mut JsonRpcCartesiMachineClient,
    ipfs_url: &str,
    payload: Vec<u8>,
    state_cid: Vec<u8>,
    app_cid: Vec<u8>,
) -> Result<Vec<u8>, std::io::Error> {
    let client = IpfsClient::from_str(ipfs_url).unwrap();
    machine.reset_iflags_y().await.unwrap();
    tracing::info!("execute");

    loop {
        let interpreter_break_reason = machine.run(u64::MAX).await.unwrap();
        let hex_encoded = hex::encode(
            machine
                .read_memory(MACHINE_IO_ADDRESSS, 1024)
                .await
                .unwrap(),
        );

        tracing::info!("hex encoded data {:?}", hex_encoded);

        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let opt = u64::from_be_bytes(read_opt_be_bytes.try_into().unwrap());

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
                    .write_memory(MACHINE_IO_ADDRESSS + 16, block.clone())
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS, block.len().to_be_bytes().to_vec())
                    .await
                    .unwrap();
            }
            EXCEPTION => {
                tracing::info!("HTIF_YIELD_REASON_TX_EXCEPTION");
                machine.destroy().await.unwrap();
                machine.shutdown().await.unwrap();
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "exception"));
            }
            LOAD_TX => {
                tracing::info!("LOAD_TX");

                let current_cid = Cid::try_from(state_cid.clone()).unwrap().to_bytes();
                let cid_length = current_cid.len() as u64;

                machine
                    .write_memory(MACHINE_IO_ADDRESSS, cid_length.to_be_bytes().to_vec())
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 8, current_cid)
                    .await
                    .unwrap();

                let payload_length = payload.clone().len() as u64;

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16 + cid_length,
                        payload_length.to_be_bytes().to_vec(),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 24 + cid_length, payload.clone())
                    .await
                    .unwrap();

                let app_cid = Cid::try_from(app_cid.clone()).unwrap().to_bytes();
                let app_cid_length = app_cid.len() as u64;

                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 24 + cid_length + payload_length, app_cid_length.to_be_bytes().to_vec())
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 32 + cid_length + payload_length, app_cid)
                    .await
                    .unwrap();


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

                let data = Cursor::new(memory);

                client.block_put(data).await.unwrap();
            }
            LOAD_APP => {
                tracing::info!("LOAD_APP");

                let app_cid = Cid::try_from(app_cid.clone()).unwrap().to_bytes();
                let cid_length = app_cid.len() as u64;

                machine
                    .write_memory(MACHINE_IO_ADDRESSS, cid_length.to_be_bytes().to_vec())
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 8, app_cid)
                    .await
                    .unwrap();
            }
            HINT => {
                let payload_length = u64::from_be_bytes(
                machine
                    .read_memory(
                        MACHINE_IO_ADDRESSS + 8,
                        8,
                    )
                    .await
                    .unwrap().try_into().unwrap()
                );

                let payload = u64::from_be_bytes(
                    machine
                        .read_memory(
                            MACHINE_IO_ADDRESSS + 16,
                            payload_length,
                        )
                        .await
                        .unwrap().try_into().unwrap()
                    );
                tracing::info!("payload {:?}", payload);
                    
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
        machine.reset_iflags_y().await.unwrap()
    }
}
