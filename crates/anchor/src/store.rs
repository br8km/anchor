use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::git;
use crate::secret;
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

fn resolve_update_target(store_root: &Path, target: &str) -> Result<Vec<String>> {
    if contains_glob_metacharacters(target) {
        return resolve_glob_target(store_root, target);
    }

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
