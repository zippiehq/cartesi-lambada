use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};

use cid::Cid;
use jsonrpc_cartesi_machine::JsonRpcCartesiMachineClient;
use std::io::Cursor;

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
const READ_BLOCK: u64 = 0x00001;
const EXCEPTION: u64 = 0x00002;
const LOAD_TX: u64 = 0x00003;
const FINISH: u64 = 0x00004;
const WRITE_BLOCK: u64 = 0x000005;

pub async fn execute(
    machine: &mut JsonRpcCartesiMachineClient,
    ipfs_url: &str,
    payload: Vec<u8>,
    state_cid: Vec<u8>
) -> Result<Vec<u8>, std::io::Error>{

    let client = IpfsClient::from_str(ipfs_url).unwrap();
    machine.reset_iflags_y().await.unwrap();

    loop {
        let read_opt_be_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();
        let opt = u64::from_be_bytes(read_opt_be_bytes.try_into().unwrap());

        match opt {
            READ_BLOCK => {
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let cid = String::from_utf8(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                        .await
                        .unwrap(),
                )
                .unwrap();

                let block = client
                    .block_get(cid.as_str())
                    .map_ok(|chunk| chunk.to_vec())
                    .try_concat()
                    .await
                    .unwrap();

                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 16, block.clone())
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS, block.len().to_le_bytes().to_vec())
                    .await
                    .unwrap();
            }
            EXCEPTION => {
                let length = u64::from_be_bytes(
                    machine
                        .read_memory(MACHINE_IO_ADDRESSS + 8, 8)
                        .await
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );

                let data = machine
                    .read_memory(MACHINE_IO_ADDRESSS + 16, length)
                    .await
                    .unwrap();

                println!("HTIF_YIELD_REASON_TX_EXCEPTION");
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "exception"));

            }
            LOAD_TX => {
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

                let payload_length = payload.clone().len().to_be_bytes();

                machine
                    .write_memory(
                        MACHINE_IO_ADDRESSS + 16 + cid_length,
                        payload_length.to_vec(),
                    )
                    .await
                    .unwrap();
                machine
                    .write_memory(MACHINE_IO_ADDRESSS + 24 + cid_length, payload.clone())
                    .await
                    .unwrap();
            }
            FINISH => {
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
                        println!("HTIF_YIELD_REASON_RX_ACCEPTED");
                        return Ok(data)
                    }
                    1 => {
                        println!("HTIF_YIELD_REASON_RX_REJECTED");
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "transaction was rejected"));
                    }
                    _ => {}
                }
            }
            WRITE_BLOCK => {
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
            _ => {}
        }
    }
}