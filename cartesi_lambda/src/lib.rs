use cartesi_jsonrpc_interfaces::index::MemoryRangeConfig;
use futures::TryStreamExt;
use ipfs_api::{IpfsApi, IpfsClient, TryFromUri};
use jsonrpc_cartesi_machine::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};

use serde_json::Value;
use std::ffi::CStr;
use std::io::Cursor;

pub const MACHINE_IO_ADDRESSS: u64 = 0x80000000000000;
const READ_BLOCK: u64 = 1;
const HINT: u64 = 2;
const WRITE_BLOCK: u64 = 3;
const STEP: u64 = 4;

pub async fn execute(
    machine: &mut JsonRpcCartesiMachineClient,
    ipfs_url: &str,
    payload: Vec<u8>,
    timestamp: u64,
    block_number: u64,
    input_index: u64,
) {
    let client = IpfsClient::from_str(ipfs_url).unwrap();

    machine.reset_iflags_y().await.unwrap();

    let input_metadata = InputMetadata {
        msg_sender: String::from("0x71C7656EC7ab88b098defB751B7401B5f6d8976F"),
        block_number: block_number,
        time_stamp: timestamp,
        epoch_index: 0,
        input_index: input_index,
    };
    let initial_config = cartesi_jsonrpc_interfaces::index::MachineConfig::from(
        &machine.clone().get_initial_config().await.unwrap(),
    );
    let rollup_config: cartesi_jsonrpc_interfaces::index::RollupConfig =
        initial_config.rollup.unwrap();
    load_rollup_input_and_metadata(machine, rollup_config, payload, input_metadata).await;
//    let initial_root_hash = machine.get_root_hash().await.unwrap();

    loop {
        let interpreter_break_reason = machine.run(u64::MAX).await.unwrap();

        if interpreter_break_reason == Value::String("yielded_manually".to_string()) {
            let read_opt_le_bytes = machine.read_memory(MACHINE_IO_ADDRESSS, 8).await.unwrap();

            let opt = u64::from_le_bytes(read_opt_le_bytes.try_into().unwrap());

            match opt {
                READ_BLOCK => {
                    let cid = String::from_utf8(
                        machine
                            .read_memory(MACHINE_IO_ADDRESSS + 0x100, 46)
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
                        .write_memory(MACHINE_IO_ADDRESSS + 0x200, block.clone())
                        .await
                        .unwrap();
                    machine
                        .write_memory(
                            MACHINE_IO_ADDRESSS + 0x100,
                            block.len().to_le_bytes().to_vec(),
                        )
                        .await
                        .unwrap();
                }
                HINT => {
                    let str_bytes = machine
                        .read_memory(MACHINE_IO_ADDRESSS + 0x100, 256)
                        .await
                        .unwrap();
                    let c_str = CStr::from_bytes_until_nul(&str_bytes).unwrap();
                    let regular_str = c_str.to_str().unwrap();

                    println!("{:?}", regular_str);
                }
                WRITE_BLOCK => {
                    let length = u64::from_le_bytes(
                        machine
                            .read_memory(MACHINE_IO_ADDRESSS + 0x100, 8)
                            .await
                            .unwrap()
                            .try_into()
                            .unwrap(),
                    );

                    let memory = machine
                        .read_memory(MACHINE_IO_ADDRESSS + 0x200, length)
                        .await
                        .unwrap();

                    let data = Cursor::new(memory);

                    client.block_put(data).await.unwrap();
                }
                /*STEP => {
                    let step = u64::from_le_bytes(
                        machine
                            .read_memory(MACHINE_IO_ADDRESSS + 0x100, 8)
                            .await
                            .unwrap()
                            .try_into()
                            .unwrap(),
                    );

                    machine
                        .store(
                            std::format!("{}-{:?}", hex::encode(initial_root_hash), step).as_str(),
                        )
                        .await
                        .unwrap();
                }*/
                _ => {}
            }

            machine.reset_iflags_y().await.unwrap();
        } else if interpreter_break_reason == Value::String("halted".to_string()) {
            machine.shutdown().await.unwrap();
        } else {
            tracing::info!(
                "Machine root hash is : {:?}",
                machine.get_root_hash().await.unwrap(),
            );
            break;
        }
    }
    machine.destroy().await.unwrap();
}

fn encode_input_metadata(data: InputMetadata) -> Vec<u8> {
    let msg_sender = unhexhash(data.msg_sender, "msg_sender");

    let mut encoded_data = Vec::new();
    encoded_data.extend_from_slice(&[0u8; 12]);
    encoded_data.extend_from_slice(&msg_sender);
    encoded_data.append(&mut write_be256(data.block_number));
    encoded_data.append(&mut write_be256(data.time_stamp));
    encoded_data.append(&mut write_be256(data.epoch_index));
    encoded_data.append(&mut write_be256(data.input_index));

    encoded_data
}

fn unhexhash(addr: String, name: &str) -> Vec<u8> {
    if !addr.starts_with("0x") {
        panic!("invalid {} {} (missing 0x prefix)", name, addr);
    }

    if addr.len() != 42 {
        panic!(
            "{} must contain 40 hex digits ({} has {} digits)",
            name,
            addr,
            addr.len() - 2
        );
    }

    let hex_digits = &addr[2..];
    let bin_result = hex::decode(hex_digits);

    match bin_result {
        Ok(bin) => bin,
        Err(err) => panic!("invalid {} {} ({})", name, addr, err),
    }
}

fn encode_string(payload: Vec<u8>) -> Vec<u8> {
    let mut encoded_string = write_be256(32);
    encoded_string.append(&mut write_be256(payload.len() as u64));
    encoded_string.append(&mut payload.clone());
    encoded_string
}

fn write_be256(value: u64) -> Vec<u8> {
    let mut buffer = [0; 32];
    buffer[24..].copy_from_slice(&value.to_be_bytes());
    buffer.to_vec()
}

async fn load_rollup_input_and_metadata(
    machine: &mut JsonRpcCartesiMachineClient,
    config: cartesi_jsonrpc_interfaces::index::RollupConfig,
    payload: Vec<u8>,
    input_metadata: InputMetadata,
) {
    machine
        .replace_memory_range(config.input_metadata.clone().unwrap())
        .await
        .unwrap();
    load_memory_range(
        machine,
        config.input_metadata.unwrap(),
        encode_input_metadata(input_metadata),
    )
    .await;

    machine
        .replace_memory_range(config.rx_buffer.clone().unwrap())
        .await
        .unwrap();
    load_memory_range(machine, config.rx_buffer.unwrap(), encode_string(payload)).await;

    machine
        .replace_memory_range(config.voucher_hashes.unwrap())
        .await
        .unwrap();
    machine
        .replace_memory_range(config.notice_hashes.unwrap())
        .await
        .unwrap();
}

async fn load_memory_range(
    machine: &mut JsonRpcCartesiMachineClient,
    config: cartesi_jsonrpc_interfaces::index::MemoryRangeConfig,
    data: Vec<u8>,
) {
    let mut address = config.start.unwrap();
    let chunk_len = 1024*1024;
    for chunk in data.chunks(chunk_len) {
        machine
        .write_memory(address, chunk.to_vec())
        .await
        .unwrap();
        address += 1024*1024;
    }
}
struct InputMetadata {
    msg_sender: String,
    block_number: u64,
    time_stamp: u64,
    epoch_index: u64,
    input_index: u64,
}
