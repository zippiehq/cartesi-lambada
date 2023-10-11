use blocks_stream::Options;
use clap::Parser;
use futures::join;
use async_compatibility_layer::logging::{setup_backtrace, setup_logging};
use blocks_stream::executor::{ExecutorOptions, subscribe};
use std::process::Command;

#[async_std::main]
async fn main() {
    let output = Command::new("sh")
        .arg("program/gen_machine_simple.sh")
        .output()
        .expect("Failed to execute program/gen_machine_simple.sh");
    setup_logging();
    setup_backtrace();
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        tracing::info!("Script output: {}", stdout);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::info!("Script execution failed: {}", stderr);

    }
    let opt = Options::parse();
    let cartesi_machine_path = "/machines/foo1";
    let cartesi_machine_url = "http://127.0.0.1:50051".to_string();
    let ipfs_url = "http://127.0.0.1:5001";
    let vm_id = 1000;

    let executor_options = ExecutorOptions {
        hotshot_address: opt.hotshot_address,
        l1_provider: opt.l1_provider.clone(),
        sequencer_url: opt.sequencer_url.clone(),
    };
    join!(
        subscribe(&executor_options, cartesi_machine_url, cartesi_machine_path, ipfs_url, vm_id),
    );
}