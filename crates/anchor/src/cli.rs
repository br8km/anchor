use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

use crate::clipboard;
use crate::secret;
use crate::store;
use crate::totp;

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
    Update(UpdateArgs),
    Show(SecretArgs),
    #[command(name = "meta")]
    Meta(SecretArgs),
    #[command(name = "metaedit")]
    MetaEdit(SecretArgs),
    Copy(SecretArgs),
    List,
    Grep(GrepArgs),
    Otp(OtpArgs),
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
struct UpdateArgs {
    #[arg(value_name = "PATH", num_args = 1..)]
    targets: Vec<String>,
    #[arg(long = "multiline")]
    multiline: bool,
}

#[derive(Args, Debug)]
struct GrepArgs {
    #[arg(value_name = "TERM")]
    term: String,
}

#[derive(Args, Debug)]
struct OtpArgs {
    #[command(subcommand)]
    action: OtpAction,
}

#[derive(Subcommand, Debug)]
enum OtpAction {
    Add(OtpNameArgs),
    Code(OtpCodeArgs),
    Uri(OtpUriArgs),
    Validate(OtpValidateArgs),
}

#[derive(Args, Debug)]
struct OtpNameArgs {
    #[arg(value_name = "NAME")]
    name: String,
}

#[derive(Args, Debug)]
struct OtpCodeArgs {
    #[arg(value_name = "NAME")]
    name: String,
    #[arg(long = "clipboard")]
    clipboard: bool,
}

#[derive(Args, Debug)]
struct OtpUriArgs {
    #[arg(value_name = "NAME")]
    name: String,
    #[arg(long = "clipboard")]
    clipboard: bool,
}

#[derive(Args, Debug)]
struct OtpValidateArgs {
    #[arg(value_name = "URI")]
    uri: String,
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
        Commands::Update(args) => {
            let targets = store::resolve_update_targets(&cli.store, &args.targets)?;
            show_update_preview(&cli.store, &targets)?;
            let mut stdin = std::io::stdin().lock();
            confirm_update(targets.len(), &mut stdin)?;
            let replacement = read_update_replacement(args.multiline, &mut stdin)?;

            for target in targets {
                let report =
                    store::update_secret(&cli.store, &target, &replacement, args.multiline)?;
                println!("updated secret at {}", report.entry_path.display());
            }
        }
        Commands::Show(args) => {
            let secret = store::show_secret(&cli.store, &args.name)?;
            println!("{secret}");
        }
        Commands::Meta(args) => {
            let metadata = store::show_metadata(&cli.store, &args.name)?;
            print!("{metadata}");
        }
        Commands::MetaEdit(args) => {
            let metadata = read_stdin()?;
            let report = store::edit_metadata(&cli.store, &args.name, &metadata)?;
            println!("edited metadata at {}", report.entry_path.display());
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
        Commands::Otp(args) => match args.action {
            OtpAction::Add(args) => {
                let input = read_stdin()?;
                let report = store::add_totp(&cli.store, &args.name, &input)?;
                println!("stored TOTP data at {}", report.entry_path.display());
            }
            OtpAction::Code(args) => {
                let code = store::show_totp_code(&cli.store, &args.name)?;
                if args.clipboard {
                    clipboard::copy_with_timeout(
                        &code,
                        std::time::Duration::from_millis(cli.clipboard_timeout_ms),
                    )?;
                }
                println!("{code}");
            }
            OtpAction::Uri(args) => {
                let uri = store::show_totp_uri(&cli.store, &args.name)?;
                if args.clipboard {
                    clipboard::copy_with_timeout(
                        &uri,
                        std::time::Duration::from_millis(cli.clipboard_timeout_ms),
                    )?;
                }
                println!("{uri}");
            }
            OtpAction::Validate(args) => {
                totp::validate_uri(&args.uri)?;
            }
        },
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

fn show_update_preview(store_root: &Path, targets: &[String]) -> Result<()> {
    for target in targets {
        let current = store::show_secret(store_root, target)?;
        if targets.len() == 1 {
            println!("{current}");
        } else {
            println!("{target}: {current}");
        }
    }

    Ok(())
}

fn confirm_update(target_count: usize, stdin: &mut impl BufRead) -> Result<()> {
    eprint!(
        "replace {}? [y/N] ",
        if target_count == 1 {
            "this secret"
        } else {
            "these secrets"
        }
    );
    std::io::stderr().flush()?;

    let mut response = String::new();
    stdin.read_line(&mut response)?;
    let trimmed = response.trim();
    if matches!(trimmed, "y" | "Y" | "yes" | "YES") {
        Ok(())
    } else {
        Err(anyhow!("update cancelled"))
    }
}

fn read_update_replacement(multiline: bool, stdin: &mut impl BufRead) -> Result<String> {
    if multiline {
        let mut body = String::new();
        stdin.read_to_string(&mut body)?;
        Ok(body)
    } else {
        let mut body = String::new();
        stdin.read_line(&mut body)?;
        Ok(body)
    }
}
