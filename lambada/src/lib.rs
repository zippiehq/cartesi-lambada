#[cfg(not(feature = "no_sqlite"))]
pub mod executor;
use async_compatibility_layer::logging::setup_backtrace;
use async_compatibility_layer::logging::setup_logging;
use clap::Parser;
use nix::libc;
use os_pipe::dup_stdin;
use std::env;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;

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
    pub chain_vm_id: String,
    pub genesis_cid_text: String,
}
pub fn setup_subscriber(sequencer: &str) -> Option<(SubscribeInput, String)> {
    let chain_cid = &env::args().collect::<Vec<_>>()[1];
    let log_directory_path: String =
        std::env::var("LAMBADA_LOGS_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let my_stdout = File::create(format!(
        "{}/{}-{}-stdout.log",
        log_directory_path, chain_cid, sequencer
    ))
    .expect("Failed to create stdout file");
    let my_stderr = File::create(format!(
        "{}/{}-{}-stderr.log",
        log_directory_path, chain_cid, sequencer
    ))
    .expect("Failed to create stderr file");
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
    let mut stdin = dup_stdin().unwrap();
    if let Ok(parameter) = read_message(&mut stdin) {
        return Some((
            serde_json::from_slice::<SubscribeInput>(&parameter).unwrap(),
            chain_cid.to_string(),
        ));
    }
    return None;
}
pub fn read_message(mut pipe: &os_pipe::PipeReader) -> Result<Vec<u8>, std::io::Error> {
    let mut len: [u8; 8] = [0; 8];
    pipe.read_exact(&mut len)?;
    let len = u64::from_le_bytes(len);
    let mut message: Vec<u8> = vec![0; len as usize];
    pipe.read_exact(&mut message)?;
    Ok(message)
}
#[derive(Parser, Clone, Debug)]
pub struct Options {
    #[clap(long, env = "ESPRESSO_TESTNET_SEQUENCER_URL")]
    pub espresso_testnet_sequencer_url: String,

    #[clap(long, env = "CELESTIA_TESTNET_SEQUENCER_URL")]
    pub celestia_testnet_sequencer_url: String,

    #[clap(long, env = "AVAIL_TESTNET_SEQUENCER_URL")]
    pub avail_testnet_sequencer_url: String,

    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "db_path", default_value = "db/")]
    pub db_path: String,

    #[clap(long, env = "IPFS_URL")]
    pub ipfs_url: String,

    #[clap(long, env = "IPFS_WRITE_URL")]
    pub ipfs_write_url: String,

    #[clap(long, env = "EVM_DA_URL")]
    pub evm_da_url: String,

    #[clap(long, env = "AUTOMATIC_SUBSCRIBE", default_value = "")]
    pub automatic_subscribe: String,
}
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use committable::{Commitment, Committable};
use derive_more::{Display, Into};
use serde::{Deserialize, Serialize};
#[derive(
    Serialize,
    Deserialize,
    Ord,
    Display,
    PartialOrd,
    PartialEq,
    Eq,
    Hash,
    Debug,
    CanonicalDeserialize,
    CanonicalSerialize,
    Default,
    Clone,
    Copy,
    Into,
)]
#[display(fmt = "{_0}")]
pub struct NamespaceId(u64);

impl From<u64> for NamespaceId {
    fn from(number: u64) -> Self {
        Self(number)
    }
}
#[derive(Serialize, Deserialize)]
pub struct EspressoTransaction {
    namespace: NamespaceId,
    #[serde(with = "base64_bytes")]
    payload: Vec<u8>,
}

impl Committable for EspressoTransaction {
    fn commit(&self) -> Commitment<Self> {
        committable::RawCommitmentBuilder::new("Transaction")
            .u64_field("namespace", self.namespace.into())
            .var_size_bytes(&self.payload)
            .finalize()
    }

    fn tag() -> String {
        "TX".into()
    }
}

impl EspressoTransaction {
    pub fn new(namespace: NamespaceId, payload: Vec<u8>) -> Self {
        Self { namespace, payload }
    }
}
