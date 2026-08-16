use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::io::Read;
use std::path::PathBuf;

use crate::clipboard;
use crate::secret;
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
    #[arg(
        long = "clipboard-timeout-ms",
        global = true,
        value_name = "MILLISECONDS",
        default_value_t = 10_000
    )]
    clipboard_timeout_ms: u64,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init(InitArgs),
    Vault(VaultArgs),
    Add(SecretArgs),
    Edit(SecretArgs),
    Remove(SecretArgs),
    Generate(SecretArgs),
    Show(SecretArgs),
    Copy(SecretArgs),
    List,
    Grep(GrepArgs),
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

#[derive(Args, Debug)]
struct SecretArgs {
    #[arg(value_name = "NAME")]
    name: String,
}

#[derive(Args, Debug)]
struct GrepArgs {
    #[arg(value_name = "TERM")]
    term: String,
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
        Commands::Add(args) => {
            let plaintext = read_stdin()?;
            let report = store::add_secret(&cli.store, &args.name, &plaintext)?;
            println!("added secret at {}", report.entry_path.display());
        }
        Commands::Edit(args) => {
            let plaintext = read_stdin()?;
            let replacement = secret::first_line(&plaintext)?;
            let report = store::edit_secret(&cli.store, &args.name, replacement)?;
            println!("edited secret at {}", report.entry_path.display());
        }
        Commands::Remove(args) => {
            let report = store::remove_secret(&cli.store, &args.name)?;
            println!("removed secret at {}", report.entry_path.display());
        }
        Commands::Generate(args) => {
            let report = store::generate_secret(&cli.store, &args.name)?;
            println!("generated secret at {}", report.entry_path.display());
        }
        Commands::Show(args) => {
            let secret = store::show_secret(&cli.store, &args.name)?;
            println!("{secret}");
        }
        Commands::Copy(args) => {
            let secret = store::show_secret(&cli.store, &args.name)?;
            clipboard::copy_with_timeout(
                &secret,
                std::time::Duration::from_millis(cli.clipboard_timeout_ms),
            )?;
            println!("copied secret from {}", args.name);
        }
        Commands::List => {
            for name in store::list_secrets(&cli.store)? {
                println!("{name}");
            }
        }
        Commands::Grep(args) => {
            for name in store::grep_secrets(&cli.store, &args.term)? {
                println!("{name}");
            }
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

fn read_stdin() -> Result<String> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    Ok(input)
}
