pub mod executor;

use surf_disco::Url;
use clap::Parser;
use ethers::types::Address;

#[derive(Parser, Clone, Debug)]
pub struct Options {

    #[clap(long, env = "HEIGHT")]
    pub height: u64,

    #[clap(long, env = "MACHINE_DIR")]
    pub machine_dir: String,

    #[clap(long, env = "DB_DIR", default_value = "sequencer_db")]
    pub db_dir: String,
}