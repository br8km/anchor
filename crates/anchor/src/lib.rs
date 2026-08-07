pub mod cli;
pub mod git;
pub mod store;
pub mod vault;

use anyhow::Result;

pub fn run() -> Result<()> {
    cli::run()
}
