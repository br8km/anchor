use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::git;
use crate::secret;
use crate::totp;
use crate::vault::{vault_paths, VaultReport, VaultStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub store_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReport {
    pub store_root: PathBuf,
    pub entry_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipientReport {
    pub store_root: PathBuf,
    pub recipients: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub source: PathBuf,
    pub imported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub destination: PathBuf,
    pub exported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub store_root: PathBuf,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub pulled: bool,
    pub pushed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStatus {
    pub store_root: PathBuf,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub clean: bool,
    pub remote_branch_exists: Option<bool>,
}

pub fn init(store_root: &Path, recipients: &[String]) -> Result<InitReport> {
    ensure_bootstrap_target_is_safe(store_root)?;

    fs::create_dir_all(store_root)
        .with_context(|| format!("failed to create store root {}", store_root.display()))?;

    let recipient = recipients
        .first()
        .map(String::as_str)
        .context("at least one recipient is required")?;
    let paths = vault_paths(store_root);
    paths.initialize(recipient)?;
    paths.open()?;

    let result = (|| -> Result<()> {
        git::init_repo(store_root)?;
        write_recipient_metadata(store_root, recipients)?;
        git::add_path(store_root, ".gpg-id")?;
        git::commit(store_root, "Initialize anchor vault")?;
        Ok(())
    })();

    let close_result = paths.close();
    result?;
    close_result?;

    Ok(InitReport {
        store_root: store_root.to_path_buf(),
    })
}

pub fn vault_open(store_root: &Path) -> Result<VaultReport> {
    ensure_store_exists(store_root)?;
    ensure_git_clean(store_root)?;
    let paths = vault_paths(store_root);
    paths.ensure_container_exists()?;
    paths.open()?;
    Ok(VaultReport {
        store_root: store_root.to_path_buf(),
        open: true,
    })
}

pub fn vault_close(store_root: &Path) -> Result<VaultReport> {
    ensure_store_exists(store_root)?;
    ensure_git_clean(store_root)?;
    let paths = vault_paths(store_root);
    paths.ensure_container_exists()?;
    paths.close()?;
    Ok(VaultReport {
        store_root: store_root.to_path_buf(),
        open: false,
    })
}

pub fn vault_status(store_root: &Path) -> Result<VaultStatus> {
    ensure_store_exists(store_root)?;
    let open = vault_paths(store_root).status()?;
    Ok(VaultStatus {
        store_root: store_root.to_path_buf(),
        open,
    })
}

pub fn sync(store_root: &Path) -> Result<SyncReport> {
    with_mutating_vault(store_root, || {
        let branch = git::current_branch(store_root)?
            .context("git repository has no current branch to sync")?;
        let remote = select_sync_remote(store_root, Some(&branch))?;
        let mut pulled = false;
        let mut pushed = false;

        if let Some(remote) = remote.as_deref() {
            if git::remote_branch_exists(store_root, remote, &branch)? {
                git::pull_ff_only(store_root, remote, &branch)?;
                pulled = true;
            }
            git::push(store_root, remote, &branch)?;
            pushed = true;
        }

        Ok(SyncReport {
            store_root: store_root.to_path_buf(),
            branch: Some(branch),
            remote,
            pulled,
            pushed,
        })
    })
}

pub fn sync_status(store_root: &Path) -> Result<SyncStatus> {
    with_readonly_vault(store_root, || {
        let clean = git::is_clean(store_root)?;
        let branch = git::current_branch(store_root)?;
        let remote = select_sync_remote(store_root, branch.as_deref())?;
        let remote_branch_exists = match (&branch, &remote) {
            (Some(branch), Some(remote)) => {
                Some(git::remote_branch_exists(store_root, remote, branch)?)
            }
            _ => None,
        };

        Ok(SyncStatus {
            store_root: store_root.to_path_buf(),
            branch,
            remote,
            clean,
            remote_branch_exists,
        })
    })
}

pub fn add_secret(store_root: &Path, name: &str, plaintext: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    required_first_line(plaintext)?;
    let recipients = load_recipients(store_root)?;

    with_mutating_vault(store_root, || {
        if entry_path.exists() {
            bail!("secret already exists");
        }

        secret::encrypt_entry(&entry_path, &recipients, plaintext)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Add secret {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn edit_secret(store_root: &Path, name: &str, replacement: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    required_first_line(replacement)?;
    let recipients = load_recipients(store_root)?;

    with_mutating_vault(store_root, || {
        if !entry_path.is_file() {
            bail!("secret does not exist");
        }

        let existing = secret::decrypt_entry(&entry_path)?;
        let updated = secret::replace_first_line(&existing, replacement);
        secret::encrypt_entry(&entry_path, &recipients, &updated)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Edit secret {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn generate_secret(store_root: &Path, name: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    let recipients = load_recipients(store_root)?;

    with_mutating_vault(store_root, || {
        let updated = if entry_path.is_file() {
            let existing = secret::decrypt_entry(&entry_path)?;
            secret::replace_first_line(&existing, &secret::generate_secret())
        } else {
            format!("{}\n", secret::generate_secret())
        };

        secret::encrypt_entry(&entry_path, &recipients, &updated)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Generate secret {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn remove_secret(store_root: &Path, name: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;

    with_mutating_vault(store_root, || {
        if !entry_path.is_file() {
            bail!("secret does not exist");
        }

        secret::remove_entry(&entry_path)?;
        git::remove_path(store_root, &relative)?;
        git::commit(store_root, &format!("Remove secret {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn show_secret(store_root: &Path, name: &str) -> Result<String> {
    with_readonly_vault(store_root, || {
        let body = decrypt_secret_body(store_root, name)?;
        Ok(secret::first_line(&body)?.to_string())
    })
}

pub fn show_metadata(store_root: &Path, name: &str) -> Result<String> {
    with_readonly_vault(store_root, || {
        let body = decrypt_secret_body(store_root, name)?;
        Ok(secret::entry_metadata(&body)?.to_string())
    })
}

pub fn list_secrets(store_root: &Path) -> Result<Vec<String>> {
    with_readonly_vault(store_root, || {
        let mut names = Vec::new();
        collect_secrets(store_root, store_root, &mut names)?;
        names.sort();
        Ok(names)
    })
}

pub fn grep_secrets(store_root: &Path, term: &str) -> Result<Vec<String>> {
    ensure_store_exists(store_root)?;
    let term = term.to_lowercase();

    let matches = with_readonly_vault(store_root, || {
        let mut matches = Vec::new();
        let mut names = Vec::new();
        collect_secrets(store_root, store_root, &mut names)?;
        names.sort();

        for name in names {
            let body = decrypt_secret_body(store_root, &name)?;
            if name.to_lowercase().contains(&term) || body.to_lowercase().contains(&term) {
                matches.push(name);
            }
        }
        Ok(matches)
    })?;

    Ok(matches)
}

pub fn edit_metadata(store_root: &Path, name: &str, replacement: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    let recipients = load_recipients(store_root)?;

    with_mutating_vault(store_root, || {
        if !entry_path.is_file() {
            bail!("secret does not exist");
        }

        let existing = secret::decrypt_entry(&entry_path)?;
        let current_metadata = secret::entry_metadata(&existing)?;
        secret::validate_metadata_keys(current_metadata)?;
        secret::validate_metadata_keys(replacement)?;
        let updated = secret::replace_entry_metadata(&existing, replacement)?;
        secret::encrypt_entry(&entry_path, &recipients, &updated)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Edit metadata {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn import_secrets(
    store_root: &Path,
    source: &Path,
    overwrite: bool,
    rename: bool,
) -> Result<ImportReport> {
    ensure_store_exists(store_root)?;
    ensure_git_clean(store_root)?;
    if overwrite && rename {
        bail!("overwrite and rename are mutually exclusive");
    }

    let format = MigrationFormat::from_path(source)?;
    let entries = read_migration_entries(source, format)?;
    let recipients = load_recipients(store_root)?;
    let mut planned_names = BTreeSet::new();
    let mut resolved = Vec::with_capacity(entries.len());

    for entry in &entries {
        let name = resolve_import_name(store_root, &entry.name, overwrite, rename, &planned_names)?;
        planned_names.insert(name.clone());
        resolved.push(name);
    }

    with_mutating_vault(store_root, || {
        for (entry, name) in entries.iter().zip(resolved.iter()) {
            let entry_path = secret::entry_path(store_root, name)?;
            let relative = relative_entry_path(store_root, &entry_path)?;
            let body = migration_entry_body(entry)?;

            secret::encrypt_entry(&entry_path, &recipients, &body)?;
            git::add_path(store_root, &relative)?;
        }

        git::commit(
            store_root,
            &format!("Import secrets from {}", source.display()),
        )?;
        Ok(())
    })?;

    Ok(ImportReport {
        source: source.to_path_buf(),
        imported: resolved.len(),
    })
}

pub fn export_secrets(store_root: &Path, destination: &Path) -> Result<ExportReport> {
    ensure_store_exists(store_root)?;
    let format = MigrationFormat::from_path(destination)?;
    let destination = destination.to_path_buf();

    let entries = with_readonly_vault(store_root, || {
        let mut names = Vec::new();
        collect_secrets(store_root, store_root, &mut names)?;
        names.sort();

        let mut exports = Vec::with_capacity(names.len());
        for name in names {
            let body = decrypt_secret_body(store_root, &name)?;
            let secret = secret::first_line(&body)?.to_string();
            let metadata = secret::entry_metadata(&body)?.to_string();
            exports.push(MigrationEntry {
                name,
                secret,
                metadata,
            });
        }

        Ok(exports)
    })?;

    write_migration_entries(&destination, format, &entries)?;

    Ok(ExportReport {
        destination,
        exported: entries.len(),
    })
}

pub fn list_recipients(store_root: &Path) -> Result<Vec<String>> {
    with_readonly_vault(store_root, || load_recipients(store_root))
}

pub fn add_recipient(store_root: &Path, recipient: &str) -> Result<RecipientReport> {
    let recipient = normalize_recipient(recipient)?;

    update_recipients(
        store_root,
        &format!("Add recipient {recipient}"),
        |recipients| {
            if recipients.iter().any(|existing| existing == &recipient) {
                bail!("recipient already exists");
            }

            recipients.push(recipient.clone());
            Ok(())
        },
    )
}

pub fn remove_recipient(store_root: &Path, recipient: &str) -> Result<RecipientReport> {
    let recipient = normalize_recipient(recipient)?;

    update_recipients(
        store_root,
        &format!("Remove recipient {recipient}"),
        |recipients| {
            let original_len = recipients.len();
            recipients.retain(|existing| existing != &recipient);

            if recipients.len() == original_len {
                bail!("recipient does not exist");
            }

            if recipients.is_empty() {
                bail!("at least one recipient is required");
            }

            Ok(())
        },
    )
}

pub fn resolve_update_targets(store_root: &Path, targets: &[String]) -> Result<Vec<String>> {
    ensure_store_exists(store_root)?;

    let mut resolved = BTreeSet::new();
    for target in targets {
        for name in resolve_update_target(store_root, target)? {
            resolved.insert(name);
        }
    }

    if resolved.is_empty() {
        bail!("no matching secrets found");
    }

    Ok(resolved.into_iter().collect())
}

pub fn update_secret(
    store_root: &Path,
    name: &str,
    replacement: &str,
    multiline: bool,
) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    let replacement_line = required_first_line(replacement)?;
    let recipients = load_recipients(store_root)?;

    with_mutating_vault(store_root, || {
        if !entry_path.is_file() {
            bail!("secret does not exist");
        }

        let existing = secret::decrypt_entry(&entry_path)?;
        let updated = if multiline {
            replacement.to_string()
        } else {
            secret::replace_first_line(&existing, replacement_line)
        };
        secret::encrypt_entry(&entry_path, &recipients, &updated)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Update secret {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn add_totp(store_root: &Path, name: &str, input: &str) -> Result<SecretReport> {
    let entry_path = secret::entry_path(store_root, name)?;
    let relative = relative_entry_path(store_root, &entry_path)?;
    let recipients = load_recipients(store_root)?;
    let canonical_uri = totp::canonicalize_input(input, Some(name))?;

    with_mutating_vault(store_root, || {
        if !entry_path.is_file() {
            bail!("secret does not exist");
        }

        let existing = secret::decrypt_entry(&entry_path)?;
        let current_metadata = secret::entry_metadata(&existing)?;
        secret::validate_metadata_keys(current_metadata)?;
        let updated = upsert_metadata_value(&existing, "otp", &canonical_uri)?;
        secret::encrypt_entry(&entry_path, &recipients, &updated)?;
        git::add_path(store_root, &relative)?;
        git::commit(store_root, &format!("Add TOTP data {name}"))?;
        Ok(())
    })?;

    Ok(SecretReport {
        store_root: store_root.to_path_buf(),
        entry_path,
    })
}

pub fn show_totp_uri(store_root: &Path, name: &str) -> Result<String> {
    with_readonly_vault(store_root, || {
        let body = decrypt_secret_body(store_root, name)?;
        let metadata = secret::entry_metadata(&body)?;
        let uri = secret::metadata_lookup(metadata, "otp")?
            .context("TOTP data is missing from this entry")?;
        totp::canonicalize_uri(uri)
    })
}

pub fn show_totp_code(store_root: &Path, name: &str) -> Result<String> {
    with_readonly_vault(store_root, || {
        let body = decrypt_secret_body(store_root, name)?;
        let metadata = secret::entry_metadata(&body)?;
        let uri = secret::metadata_lookup(metadata, "otp")?
            .context("TOTP data is missing from this entry")?;
        totp::current_code(uri)
    })
}

pub fn validate_totp_uri(input: &str) -> Result<()> {
    totp::validate_uri(input)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MigrationEntry {
    name: String,
    secret: String,
    #[serde(default)]
    metadata: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationFormat {
    Json,
    Csv,
}

impl MigrationFormat {
    fn from_path(path: &Path) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(OsStr::to_str)
            .map(|extension| extension.to_ascii_lowercase())
            .context("migration file must have a .json or .csv extension")?;

        match extension.as_str() {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            _ => bail!("migration file must have a .json or .csv extension"),
        }
    }
}

fn read_migration_entries(path: &Path, format: MigrationFormat) -> Result<Vec<MigrationEntry>> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure_migration_content_matches_format(&contents, format, path)?;

    match format {
        MigrationFormat::Json => serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse JSON migration {}", path.display())),
        MigrationFormat::Csv => {
            let mut reader = csv::Reader::from_reader(contents.as_bytes());
            let mut entries = Vec::new();
            for row in reader.deserialize() {
                let entry: MigrationEntry = row
                    .with_context(|| format!("failed to parse CSV migration {}", path.display()))?;
                entries.push(entry);
            }
            Ok(entries)
        }
    }
}

fn ensure_migration_content_matches_format(
    contents: &str,
    format: MigrationFormat,
    path: &Path,
) -> Result<()> {
    let trimmed = contents.trim_start();
    match format {
        MigrationFormat::Json => {
            if !matches!(trimmed.chars().next(), Some('[') | Some('{')) {
                bail!("{} does not look like JSON migration data", path.display());
            }
        }
        MigrationFormat::Csv => {
            if matches!(trimmed.chars().next(), Some('[') | Some('{')) {
                bail!("{} does not look like CSV migration data", path.display());
            }
        }
    }

    Ok(())
}

fn write_migration_entries(
    path: &Path,
    format: MigrationFormat,
    entries: &[MigrationEntry],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match format {
        MigrationFormat::Json => {
            let contents = serde_json::to_string_pretty(entries)
                .context("failed to serialize JSON migration")?;
            fs::write(path, contents)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        MigrationFormat::Csv => {
            let mut writer = csv::Writer::from_path(path)
                .with_context(|| format!("failed to write {}", path.display()))?;
            for entry in entries {
                writer.serialize(entry).with_context(|| {
                    format!("failed to serialize CSV migration {}", path.display())
                })?;
            }
            writer
                .flush()
                .with_context(|| format!("failed to finish writing {}", path.display()))?;
        }
    }

    Ok(())
}

fn migration_entry_body(entry: &MigrationEntry) -> Result<String> {
    let secret_line = secret::first_line(&entry.secret)?.to_string();
    let metadata = normalize_migration_metadata(&entry.name, &entry.metadata)?;

    let mut body = secret_line;
    body.push('\n');
    body.push_str(&metadata);
    Ok(body)
}

fn normalize_migration_metadata(name: &str, metadata: &str) -> Result<String> {
    secret::validate_metadata_keys(metadata)?;

    let mut normalized = String::new();
    for line in metadata.lines() {
        let Some((key, value)) = line.split_once('=') else {
            normalized.push_str(line);
            normalized.push('\n');
            continue;
        };

        if key.eq_ignore_ascii_case("otp") {
            let canonical = totp::canonicalize_input(value, Some(name))?;
            normalized.push_str(key);
            normalized.push('=');
            normalized.push_str(&canonical);
            normalized.push('\n');
            continue;
        }

        normalized.push_str(line);
        normalized.push('\n');
    }

    Ok(normalized)
}

fn resolve_import_name(
    store_root: &Path,
    name: &str,
    overwrite: bool,
    rename: bool,
    planned_names: &BTreeSet<String>,
) -> Result<String> {
    let entry_path = secret::entry_path(store_root, name)?;
    let collides = entry_path.is_file() || planned_names.contains(name);

    if !collides || overwrite {
        return Ok(name.to_string());
    }

    if !rename {
        bail!("import collision for {name}");
    }

    let mut attempt = 0usize;
    loop {
        let candidate = if attempt == 0 {
            append_import_suffix(name, "-imported")
        } else {
            append_import_suffix(name, &format!("-imported-{attempt}"))
        };
        let candidate_path = secret::entry_path(store_root, &candidate)?;
        if !candidate_path.is_file() && !planned_names.contains(&candidate) {
            return Ok(candidate);
        }
        attempt += 1;
    }
}

fn append_import_suffix(name: &str, suffix: &str) -> String {
    match name.rsplit_once('/') {
        Some((parent, leaf)) => format!("{parent}/{}{}", leaf, suffix),
        None => format!("{name}{suffix}"),
    }
}

fn ensure_bootstrap_target_is_safe(store_root: &Path) -> Result<()> {
    if !store_root.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(store_root)
        .with_context(|| format!("failed to inspect {}", store_root.display()))?;

    if entries.next().is_some() {
        bail!("store path is not empty");
    }

    Ok(())
}

fn ensure_store_exists(store_root: &Path) -> Result<()> {
    if !store_root.is_dir() {
        bail!("store has not been initialized");
    }
    Ok(())
}

fn ensure_git_clean(store_root: &Path) -> Result<()> {
    if git::has_repo(store_root)? && !git::is_clean(store_root)? {
        bail!("git working tree is dirty");
    }
    Ok(())
}

fn select_sync_remote(store_root: &Path, branch: Option<&str>) -> Result<Option<String>> {
    if let Some(branch) = branch {
        if let Some(remote) = git::branch_remote(store_root, branch)? {
            return Ok(Some(remote));
        }
    }

    let remotes = git::remotes(store_root)?;
    if remotes.is_empty() {
        return Ok(None);
    }

    if remotes.iter().any(|remote| remote == "origin") {
        return Ok(Some("origin".to_string()));
    }

    Ok(remotes.into_iter().next())
}

fn update_recipients(
    store_root: &Path,
    commit_message: &str,
    mutate: impl FnOnce(&mut Vec<String>) -> Result<()>,
) -> Result<RecipientReport> {
    with_mutating_vault(store_root, || {
        let mut recipients = load_recipients(store_root)?;
        mutate(&mut recipients)?;
        let changed_entries = reencrypt_vault(store_root, &recipients)?;
        for entry_path in changed_entries {
            git::add_path(store_root, &entry_path)?;
        }
        git::add_path(store_root, ".gpg-id")?;
        git::commit(store_root, commit_message)?;
        Ok(())
    })?;

    Ok(RecipientReport {
        store_root: store_root.to_path_buf(),
        recipients: load_recipients(store_root)?,
    })
}

#[derive(Debug)]
struct ReencryptPlan {
    entry_path: PathBuf,
    staged_path: PathBuf,
    backup_path: PathBuf,
}

fn reencrypt_vault(store_root: &Path, recipients: &[String]) -> Result<Vec<PathBuf>> {
    let mut names = Vec::new();
    collect_secrets(store_root, store_root, &mut names)?;
    names.sort();

    let mut plans = Vec::with_capacity(names.len());
    for name in names {
        let entry_path = secret::entry_path(store_root, &name)?;
        let body = secret::decrypt_entry(&entry_path)?;
        let staged_path = staged_entry_path(&entry_path);
        let backup_path = backup_entry_path(&entry_path);

        if let Err(err) = secret::encrypt_entry(&staged_path, recipients, &body) {
            cleanup_staged_entries(&plans)?;
            if staged_path.exists() {
                fs::remove_file(&staged_path)
                    .with_context(|| format!("failed to remove {}", staged_path.display()))?;
            }
            return Err(err);
        }

        plans.push(ReencryptPlan {
            entry_path,
            staged_path,
            backup_path,
        });
    }

    for (backups_created, plan) in plans.iter().enumerate() {
        if let Err(err) = fs::rename(&plan.entry_path, &plan.backup_path) {
            rollback_backups(&plans[..backups_created])?;
            cleanup_staged_entries(&plans)?;
            return Err(err)
                .with_context(|| format!("failed to preserve {}", plan.entry_path.display()));
        }
    }

    for plan in &plans {
        if let Err(err) = fs::rename(&plan.staged_path, &plan.entry_path) {
            rollback_backups(&plans)?;
            cleanup_backups(&plans)?;
            cleanup_staged_entries(&plans)?;
            return Err(err)
                .with_context(|| format!("failed to install {}", plan.entry_path.display()));
        }
    }

    write_recipient_metadata(store_root, recipients)?;
    cleanup_backups(&plans)?;

    Ok(plans.into_iter().map(|plan| plan.entry_path).collect())
}

fn staged_entry_path(entry_path: &Path) -> PathBuf {
    let mut staged = entry_path.to_path_buf();
    let file_name = entry_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("entry.gpg");
    staged.set_file_name(format!(
        "{file_name}.anchor-reencrypt-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    staged
}

fn backup_entry_path(entry_path: &Path) -> PathBuf {
    let mut backup = entry_path.to_path_buf();
    let file_name = entry_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("entry.gpg");
    backup.set_file_name(format!(
        "{file_name}.anchor-reencrypt-backup-{}-{}.bak",
        std::process::id(),
        rand::random::<u64>()
    ));
    backup
}

fn rollback_backups(plans: &[ReencryptPlan]) -> Result<()> {
    let mut first_error = None;
    for plan in plans.iter().rev() {
        if let Err(err) = fs::rename(&plan.backup_path, &plan.entry_path) {
            if first_error.is_none() {
                first_error = Some(anyhow!(
                    "failed to restore {} from {}: {}",
                    plan.entry_path.display(),
                    plan.backup_path.display(),
                    err
                ));
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn cleanup_staged_entries(plans: &[ReencryptPlan]) -> Result<()> {
    let mut first_error = None;
    for plan in plans {
        if plan.staged_path.exists() {
            if let Err(err) = fs::remove_file(&plan.staged_path) {
                if first_error.is_none() {
                    first_error = Some(anyhow!(
                        "failed to remove {}: {}",
                        plan.staged_path.display(),
                        err
                    ));
                }
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn cleanup_backups(plans: &[ReencryptPlan]) -> Result<()> {
    let mut first_error = None;
    for plan in plans {
        if plan.backup_path.exists() {
            if let Err(err) = fs::remove_file(&plan.backup_path) {
                if first_error.is_none() {
                    first_error = Some(anyhow!(
                        "failed to remove {}: {}",
                        plan.backup_path.display(),
                        err
                    ));
                }
            }
        }
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn with_mutating_vault<T>(store_root: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    ensure_store_exists(store_root)?;
    ensure_git_clean(store_root)?;

    let paths = vault_paths(store_root);
    paths.ensure_container_exists()?;

    let was_open = paths.status()?;
    if !was_open {
        paths.open()?;
    }

    let action_result = action();
    let close_result = if was_open { Ok(()) } else { paths.close() };

    match (action_result, close_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), Ok(())) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), Err(_)) => Err(err),
    }
}

fn with_readonly_vault<T>(store_root: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    ensure_store_exists(store_root)?;

    let paths = vault_paths(store_root);
    paths.ensure_container_exists()?;

    let was_open = paths.status()?;
    if !was_open {
        paths.open()?;
    }

    let action_result = action();
    let close_result = if was_open { Ok(()) } else { paths.close() };

    match (action_result, close_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), Ok(())) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), Err(_)) => Err(err),
    }
}

fn load_recipients(store_root: &Path) -> Result<Vec<String>> {
    let path = store_root.join(".gpg-id");
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let recipients = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if recipients.is_empty() {
        bail!("at least one recipient is required");
    }

    Ok(recipients)
}

fn normalize_recipient(recipient: &str) -> Result<String> {
    let recipient = recipient.trim();
    if recipient.is_empty() {
        bail!("recipient is required");
    }

    Ok(recipient.to_string())
}

fn relative_entry_path(store_root: &Path, entry_path: &Path) -> Result<PathBuf> {
    entry_path
        .strip_prefix(store_root)
        .map(Path::to_path_buf)
        .context("secret path escaped the store root")
}

fn decrypt_secret_body(store_root: &Path, name: &str) -> Result<String> {
    let entry_path = secret::entry_path(store_root, name)?;
    secret::decrypt_entry(&entry_path)
}

fn required_first_line(input: &str) -> Result<&str> {
    secret::first_line(input)
}

fn upsert_metadata_value(existing: &str, key: &str, value: &str) -> Result<String> {
    let first_line = secret::first_line(existing)?.to_string();
    let metadata = secret::entry_metadata(existing)?;
    let mut updated = String::new();
    updated.push_str(&first_line);
    updated.push('\n');

    let mut replaced = false;
    for line in metadata.lines() {
        let Some((line_key, _)) = line.split_once('=') else {
            updated.push_str(line);
            updated.push('\n');
            continue;
        };

        if line_key.eq_ignore_ascii_case(key) {
            if !replaced {
                updated.push_str(line_key);
                updated.push('=');
                updated.push_str(value);
                updated.push('\n');
                replaced = true;
            }
            continue;
        }

        updated.push_str(line);
        updated.push('\n');
    }

    if !replaced {
        updated.push_str(key);
        updated.push('=');
        updated.push_str(value);
        updated.push('\n');
    }

    Ok(updated)
}

fn resolve_update_target(store_root: &Path, target: &str) -> Result<Vec<String>> {
    let path = target_path(store_root, target);
    if path.is_dir() {
        return collect_update_targets_from_directory(store_root, &path);
    }

    if path.is_file() && path.extension() == Some(OsStr::new("gpg")) {
        return Ok(vec![secret_name_from_entry_path(store_root, &path)?]);
    }

    let entry_path = secret::entry_path(store_root, target)?;
    if entry_path.is_file() {
        return Ok(vec![target.to_string()]);
    }

    if contains_glob_metacharacters(target) {
        return resolve_glob_target(store_root, target);
    }

    bail!("no matching secrets found for {target}");
}

fn resolve_glob_target(store_root: &Path, target: &str) -> Result<Vec<String>> {
    let pattern = target_path(store_root, target)
        .to_string_lossy()
        .into_owned();
    let mut names = Vec::new();

    for entry in glob::glob(&pattern).with_context(|| format!("invalid glob pattern {target}"))? {
        let path = entry.with_context(|| format!("failed to resolve glob pattern {target}"))?;
        if path.is_dir() {
            names.extend(collect_update_targets_from_directory(store_root, &path)?);
            continue;
        }

        if path.extension() == Some(OsStr::new("gpg")) {
            names.push(secret_name_from_entry_path(store_root, &path)?);
        }
    }

    finalize_update_targets(names, || format!("no matching secrets found for {target}"))
}

fn collect_update_targets_from_directory(
    store_root: &Path,
    directory: &Path,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    collect_secrets(store_root, directory, &mut names)?;
    finalize_update_targets(names, || {
        format!("no matching secrets found in {}", directory.display())
    })
}

fn target_path(store_root: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        store_root.join(path)
    }
}

fn contains_glob_metacharacters(target: &str) -> bool {
    target.chars().any(|ch| matches!(ch, '*' | '?' | '['))
}

fn secret_name_from_entry_path(store_root: &Path, entry_path: &Path) -> Result<String> {
    let relative = relative_entry_path(store_root, entry_path)?;
    let mut name = relative.to_string_lossy().to_string();
    if let Some(stripped) = name.strip_suffix(".gpg") {
        name = stripped.to_string();
    }
    Ok(name)
}

fn finalize_update_targets(
    mut names: Vec<String>,
    empty_message: impl FnOnce() -> String,
) -> Result<Vec<String>> {
    names.sort();
    names.dedup();

    if names.is_empty() {
        bail!("{}", empty_message());
    }

    Ok(names)
}

fn write_recipient_metadata(store_root: &Path, recipients: &[String]) -> Result<()> {
    let path = store_root.join(".gpg-id");
    let body = if recipients.is_empty() {
        String::new()
    } else {
        let mut body = recipients.join("\n");
        body.push('\n');
        body
    };
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn collect_secrets(root: &Path, current: &Path, names: &mut Vec<String>) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("failed to inspect {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == OsStr::new(".git")
            || file_name == OsStr::new(".tomb")
            || file_name == OsStr::new(".tomb.key")
            || file_name == OsStr::new(".gpg-id")
        {
            continue;
        }

        if path.is_dir() {
            collect_secrets(root, &path, names)?;
            continue;
        }

        if path.extension() == Some(OsStr::new("gpg")) {
            let relative = path
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .context("secret path escaped the store root")?;
            let mut name = relative.to_string_lossy().to_string();
            if let Some(stripped) = name.strip_suffix(".gpg") {
                name = stripped.to_string();
            }
            names.push(name);
        }
    }

    Ok(())
}
