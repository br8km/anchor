use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::store;

#[derive(Parser, Debug)]
#[command(name = "anchor")]
#[command(about = "Linux-only CLI password manager")]
pub struct Cli {
    #[arg(
        long = "store",
        global = true,
        value_name = "PATH",
        default_value_os_t = default_store_root()
    )]
    store: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init(InitArgs),
    Vault(VaultArgs),
}

#[derive(Args, Debug, Default)]
struct InitArgs {
    #[arg(long = "recipient", value_name = "RECIPIENT")]
    recipients: Vec<String>,
}

#[derive(Args, Debug)]
struct VaultArgs {
    #[command(subcommand)]
    action: VaultAction,
}

#[derive(Subcommand, Debug)]
enum VaultAction {
    Open,
    Close,
    Status,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init(args) => {
            let report = store::init(&cli.store, &args.recipients)?;
            println!("initialized store at {}", report.store_root.display());
        }
        Commands::Vault(args) => match args.action {
            VaultAction::Open => {
                let report = store::vault_open(&cli.store)?;
                println!("vault opened at {}", report.store_root.display());
            }
            VaultAction::Close => {
                let report = store::vault_close(&cli.store)?;
                println!("vault closed at {}", report.store_root.display());
            }
            VaultAction::Status => {
                let status = store::vault_status(&cli.store)?;
                println!(
                    "vault {} at {}",
                    if status.open { "open" } else { "closed" },
                    status.store_root.display()
                );
            }
        },
    }

    Ok(())
}

fn default_store_root() -> PathBuf {
    std::env::var_os("ANCHOR_STORE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
            PathBuf::from(home).join(".password-store")
        })
}
