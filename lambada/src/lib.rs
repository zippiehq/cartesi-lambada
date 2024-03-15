pub mod executor;

use clap::Parser;

#[derive(Parser, Clone, Debug)]
pub struct Options {
    #[clap(long, env = "ESPRESSO_TESTNET_SEQUENCER_URL")]
    pub espresso_testnet_sequencer_url: String,

    #[clap(long, env = "CELESTIA_TESTNET_SEQUENCER_URL")]
    pub celestia_testnet_sequencer_url: String,

    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "db_path", default_value = "db/")]
    pub db_path: String,

    #[clap(long, env = "IPFS_URL")]
    pub ipfs_url: String,

    #[clap(long, env = "IPFS_WRITE_URL")]
    pub ipfs_write_url: String,
}
