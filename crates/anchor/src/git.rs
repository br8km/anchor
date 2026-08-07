use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn init_repo(store_root: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("init")
        .arg("-q")
        .arg(store_root)
        .status()
        .context("failed to invoke git init")?;

    ensure_success(status, "git init")?;
    Ok(())
}

pub fn add_path(store_root: &Path, path: &str) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .arg("add")
        .arg(path)
        .status()
        .context("failed to invoke git add")?;

    ensure_success(status, "git add")?;
    Ok(())
}

pub fn commit(store_root: &Path, message: &str) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .env("GIT_AUTHOR_NAME", "anchor")
        .env("GIT_AUTHOR_EMAIL", "anchor@example.com")
        .env("GIT_COMMITTER_NAME", "anchor")
        .env("GIT_COMMITTER_EMAIL", "anchor@example.com")
        .args(["commit", "-q", "--allow-empty", "-m", message])
        .status()
        .context("failed to invoke git commit")?;

    ensure_success(status, "git commit")?;
    Ok(())
}

pub fn has_repo(store_root: &Path) -> Result<bool> {
    Ok(store_root.join(".git").is_dir())
}

pub fn is_clean(store_root: &Path) -> Result<bool> {
    if !has_repo(store_root)? {
        return Ok(true);
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["status", "--porcelain"])
        .output()
        .context("failed to invoke git status")?;

    if !output.status.success() {
        bail!("git status failed");
    }

    Ok(output.stdout.is_empty())
}

fn ensure_success(status: std::process::ExitStatus, action: &str) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    bail!("{action} failed");
}
