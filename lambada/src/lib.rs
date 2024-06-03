pub mod executor;

use clap::Parser;

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
use jf_merkle_tree::namespaced_merkle_tree::Namespace;
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
