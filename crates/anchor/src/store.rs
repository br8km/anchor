use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::git;
use crate::vault::{vault_marker_path, vault_paths, VaultReport, VaultStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub store_root: PathBuf,
}

pub fn init(store_root: &Path, recipients: &[String]) -> Result<InitReport> {
    ensure_bootstrap_target_is_safe(store_root)?;

    fs::create_dir_all(store_root)
        .with_context(|| format!("failed to create store root {}", store_root.display()))?;

    git::init_repo(store_root)?;
    write_recipient_metadata(store_root, recipients)?;
    git::add_path(store_root, ".gpg-id")?;
    git::commit(store_root, "Initialize anchor vault")?;
    vault_paths(store_root).create_placeholders()?;
    vault_marker_path(store_root).clear()?;

    Ok(InitReport {
        store_root: store_root.to_path_buf(),
    })
}

pub fn vault_open(store_root: &Path) -> Result<VaultReport> {
    ensure_store_exists(store_root)?;
    ensure_git_clean(store_root)?;
    let paths = vault_paths(store_root);
    paths.ensure_container_exists()?;
    vault_marker_path(store_root).open()?;
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
    vault_marker_path(store_root).close()?;
    Ok(VaultReport {
        store_root: store_root.to_path_buf(),
        open: false,
    })
}

pub fn vault_status(store_root: &Path) -> Result<VaultStatus> {
    ensure_store_exists(store_root)?;
    let open = vault_marker_path(store_root).is_open();
    Ok(VaultStatus {
        store_root: store_root.to_path_buf(),
        open,
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
