pub mod executor;

use surf_disco::Url;
use clap::Parser;
use ethers::types::Address;

#[derive(Parser, Clone, Debug)]
pub struct Options {
    /// URL of a HotShot sequencer node.
    #[clap(long, env = "ESPRESSO_SEQUENCER_URL")]
    pub sequencer_url: Url,

    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "db_file", default_value = "sequencer_db")]
    pub db_file: String,

    #[clap(long, env = "APPCHAIN")]
    pub appchain: String,

    #[clap(long, env = "CARTESI_MACHINE_URL")]
    pub cartesi_machine_url: String,

    #[clap(long, env = "IPFS_URL")]
    pub ipfs_url: String,

    #[clap(long, env = "COMPUTE_ONLY")]
    pub compute_only: bool,
}