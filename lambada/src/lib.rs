pub mod executor;

use clap::Parser;

#[derive(Parser, Clone, Debug)]
pub struct Options {
    #[clap(long, env = "ESPRESSO_SEQUENCER_URL")]
    pub sequencer_url: String,

    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "db_path", default_value = "db/")]
    pub db_path: String,

    #[clap(long, env = "APPCHAIN")]
    pub appchain: String,

    #[clap(long, env = "CARTESI_MACHINE_URL")]
    pub cartesi_machine_url: String,

    #[clap(long, env = "IPFS_URL")]
    pub ipfs_url: String,

    #[clap(long, env = "COMPUTE_ONLY")]
    pub compute_only: bool,
}
