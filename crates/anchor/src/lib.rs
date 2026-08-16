pub mod cli;
pub mod clipboard;
pub mod git;
mod secret;
pub mod store;
pub mod totp;
pub mod vault;

use anyhow::Result;

pub fn run() -> Result<()> {
    cli::run()
}
