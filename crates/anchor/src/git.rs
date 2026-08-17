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

pub fn add_path(store_root: &Path, path: impl AsRef<Path>) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .arg("add")
        .arg("--")
        .arg(path.as_ref())
        .status()
        .context("failed to invoke git add")?;

    ensure_success(status, "git add")?;
    Ok(())
}

pub fn remove_path(store_root: &Path, path: impl AsRef<Path>) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["rm", "-q", "--force", "--"])
        .arg(path.as_ref())
        .status()
        .context("failed to invoke git rm")?;

    ensure_success(status, "git rm")?;
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

pub fn current_branch(store_root: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["branch", "--show-current"])
        .output()
        .context("failed to invoke git branch")?;

    if !output.status.success() {
        bail!("git branch failed");
    }

    let branch = String::from_utf8(output.stdout).context("git branch output was not utf-8")?;
    let branch = branch.trim();
    if branch.is_empty() {
        Ok(None)
    } else {
        Ok(Some(branch.to_string()))
    }
}

pub fn has_repo(store_root: &Path) -> Result<bool> {
    Ok(store_root.join(".git").is_dir())
}

pub fn remotes(store_root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .arg("remote")
        .output()
        .context("failed to invoke git remote")?;

    if !output.status.success() {
        bail!("git remote failed");
    }

    let stdout = String::from_utf8(output.stdout).context("git remote output was not utf-8")?;
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn branch_remote(store_root: &Path, branch: &str) -> Result<Option<String>> {
    let key = format!("branch.{branch}.remote");
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["config", "--get", &key])
        .output()
        .context("failed to invoke git config")?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8(output.stdout).context("git config output was not utf-8")?;
    let remote = stdout.trim();
    if remote.is_empty() {
        Ok(None)
    } else {
        Ok(Some(remote.to_string()))
    }
}

pub fn remote_branch_exists(store_root: &Path, remote: &str, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["ls-remote", "--heads", remote, branch])
        .output()
        .context("failed to invoke git ls-remote")?;

    if !output.status.success() {
        bail!("git ls-remote failed");
    }

    Ok(!output.stdout.is_empty())
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

pub fn pull_ff_only(store_root: &Path, remote: &str, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["pull", "--ff-only", remote, branch])
        .output()
        .context("failed to invoke git pull")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("fast-forward")
        || stderr.contains("non-fast-forward")
        || stderr.contains("diverg")
        || stderr.contains("fetch first")
    {
        bail!("git repository has diverged from remote");
    }

    bail!("git pull failed")
}

pub fn push(store_root: &Path, remote: &str, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store_root)
        .args(["push", remote, branch])
        .output()
        .context("failed to invoke git push")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("non-fast-forward") || stderr.contains("rejected") {
        bail!("git repository has diverged from remote");
    }

    bail!("git push failed")
}

fn ensure_success(status: std::process::ExitStatus, action: &str) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    bail!("{action} failed");
}
