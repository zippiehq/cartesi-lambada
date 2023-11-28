use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};

use cid::Cid;
use jsonrpc_cartesi_machine::JsonRpcCartesiMachineClient;
use std::{io::Cursor, vec};

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
    timestamp: u64,
    block_number: u64,
    input_index: u64,
    connection_path: String,
    genesis_cid: Option<String>
) {
    let query = "CREATE TABLE IF NOT EXISTS transactions (
        block_height INTEGER NOT NULL,
        transaction_index INTEGER NOT NULL,
        count INTEGER NOT NULL,
        data BLOB NOT NULL,
        type INTEGER NOT NULL,
        PRIMARY KEY (block_height, transaction_index, count)
    );";
    let connection = sqlite::open(connection_path.clone()).unwrap();

    connection.execute(query).unwrap();
    let client = IpfsClient::from_str(ipfs_url).unwrap();

    machine.reset_iflags_y().await.unwrap();

    let initial_config = cartesi_jsonrpc_interfaces::index::MachineConfig::from(
        &machine.clone().get_initial_config().await.unwrap(),
    );
    let rollup_config: cartesi_jsonrpc_interfaces::index::RollupConfig =
        initial_config.rollup.unwrap();
    //    let initial_root_hash = machine.get_root_hash().await.unwrap();
    let mut count = 0;
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
                count += 1;
                let mut statement = connection
                        .prepare(
                            "INSERT OR REPLACE INTO transactions (block_height, transaction_index, count, data, type) VALUES (?, ?, ?, ?, ?)",
                        )
                        .unwrap();
                statement.bind((1, block_number as i64)).unwrap();
                statement.bind((2, input_index as i64)).unwrap();
                statement.bind((3, count as i64)).unwrap();

                statement.bind((4, data.as_slice() as &[u8])).unwrap();

                statement.bind((5, "exception")).unwrap();
                statement.next().unwrap();
            }
            LOAD_TX => {
                let mut current_cid: Vec<u8> = Vec::new();
                if genesis_cid.clone().is_some() {
                    current_cid = Cid::try_from(genesis_cid.clone().unwrap()).unwrap().to_bytes()
                } else {
                    let cid = get_current_status(connection_path.clone(), input_index as i64, count).expect("cid not found");
                    current_cid = Cid::try_from(cid).unwrap().to_bytes();
                }
  
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
                        count += 1;
                        let mut statement = connection
                                .prepare(
                                    "INSERT OR REPLACE INTO transactions (block_height, transaction_index, count, data, type) VALUES (?, ?, ?, ?, ?)",
                                )
                                .unwrap();
                        statement.bind((1, block_number as i64)).unwrap();
                        statement.bind((2, input_index as i64)).unwrap();
                        statement.bind((3, count as i64)).unwrap();

                        statement.bind((4, data.as_slice() as &[u8])).unwrap();

                        statement.bind((5, "accepted")).unwrap();
                        statement.next().unwrap();
                    }
                    1 => {
                        println!("HTIF_YIELD_REASON_RX_REJECTED");
                        count += 1;
                        let mut statement = connection
                                .prepare(
                                    "INSERT OR REPLACE INTO transactions (block_height, transaction_index, count, data, type) VALUES (?, ?, ?, ?, ?)",
                                )
                                .unwrap();
                        statement.bind((1, block_number as i64)).unwrap();
                        statement.bind((2, input_index as i64)).unwrap();
                        statement.bind((3, count as i64)).unwrap();

                        statement.bind((4, data.as_slice() as &[u8])).unwrap();

                        statement.bind((5, "rejected")).unwrap();
                        statement.next().unwrap();
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

fn get_current_status(
    connection_path: String,
    input_index: i64,
    count: i64,
) -> Option<Vec<u8>> {
    let connection = sqlite::open(connection_path.clone()).unwrap();

    let mut statement = connection
        .prepare("SELECT * FROM transactions WHERE transaction_index = ? AND count = ?")
        .unwrap();
    statement.bind((1, input_index as i64)).unwrap();
    statement.bind((2, count as i64)).unwrap();

    if let Ok(sqlite::State::Row) = statement.next() {
        let tx_status = statement.read::<String, _>("type").unwrap();

        if tx_status == "exception" {
            get_current_status(connection_path, input_index, count - 1);
        }
        return Some(statement.read::<Vec<u8>, _>("data").unwrap());
    }
    None
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

    let input_metadata = encode_input_metadata(input_metadata);
    machine
        .write_memory(
            MACHINE_IO_ADDRESSS + 8,
            input_metadata.len().to_be_bytes().to_vec(),
        )
        .await
        .unwrap();
    let mut start = config.input_metadata.unwrap();
    start.start = Some(MACHINE_IO_ADDRESSS + 16);
    load_memory_range(machine, start, input_metadata).await;

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
    let chunk_len = 1024 * 1024;
    for chunk in data.chunks(chunk_len) {
        machine.write_memory(address, chunk.to_vec()).await.unwrap();
        address += 1024 * 1024;
    }
}
struct InputMetadata {
    msg_sender: String,
    block_number: u64,
    time_stamp: u64,
    epoch_index: u64,
    input_index: u64,
}
