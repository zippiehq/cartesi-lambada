use jsonrpc_cartesi_machine::{JsonRpcCartesiMachineClient, MachineRuntimeConfig};
use std::env;
use std::process::Command;
#[async_std::main]
async fn main() {
    let output = Command::new("sh")
        .arg("program/gen_machine_simple.sh")
        .output()
        .expect("Failed to execute program/gen_machine_simple.sh");
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::info!("Script output: {}", stdout);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::info!("Script execution failed: {}", stderr);
    }

    let args: Vec<String> = env::args().collect();

    let cartesi_machine_path = args.get(1).unwrap();

    let connection = sqlite::open(args.get(2).unwrap()).unwrap();
    let query = "
    CREATE TABLE IF NOT EXISTS blocks (hash BLOB(32) NOT NULL,
    height INTEGER NOT NULL);
";
    connection.execute(query).unwrap();

    println!("cartesi_machine_path {:?}", cartesi_machine_path);
    let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
    let ipfs_url = "http://127.0.0.1:5001";

    let mut machine = JsonRpcCartesiMachineClient::new(cartesi_machine_url)
        .await
        .unwrap();

    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());

    let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
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
    let forked_machine_url = format!("http://{}", machine.fork().await.unwrap());

    let mut machine = JsonRpcCartesiMachineClient::new(forked_machine_url)
        .await
        .unwrap();

    cartesi_lambda::execute(&mut machine, ipfs_url, vec![1, 2, 3, 4], 0, 0, 0, args.get(2).unwrap()).await;
}
