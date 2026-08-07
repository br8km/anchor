use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultReport {
    pub store_root: PathBuf,
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultStatus {
    pub store_root: PathBuf,
    pub open: bool,
}

#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub store_root: PathBuf,
    pub tomb_file: PathBuf,
    pub tomb_key_file: PathBuf,
}

impl VaultPaths {
    pub fn initialize(&self, recipient: &str) -> Result<()> {
        run_tomb(
            &self.store_root,
            [
                "dig",
                self.tomb_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb path contains invalid UTF-8"))?,
                "-s",
                "10",
            ],
        )?;
        run_tomb(
            &self.store_root,
            [
                "forge",
                self.tomb_key_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb key path contains invalid UTF-8"))?,
                "-gr",
                recipient,
            ],
        )?;
        run_tomb(
            &self.store_root,
            [
                "lock",
                self.tomb_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb path contains invalid UTF-8"))?,
                "-k",
                self.tomb_key_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb key path contains invalid UTF-8"))?,
                "-gr",
                recipient,
            ],
        )
    }

    pub fn ensure_container_exists(&self) -> Result<()> {
        if !self.tomb_file.exists() || !self.tomb_key_file.exists() {
            bail!("tomb container is missing");
        }
        Ok(())
    }

    pub fn open(&self) -> Result<()> {
        run_tomb(
            &self.store_root,
            [
                "open",
                self.tomb_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb path contains invalid UTF-8"))?,
                "-k",
                self.tomb_key_file
                    .to_str()
                    .ok_or_else(|| anyhow!("tomb key path contains invalid UTF-8"))?,
                "-p",
                self.store_root
                    .to_str()
                    .ok_or_else(|| anyhow!("store root contains invalid UTF-8"))?,
            ],
        )
    }

    pub fn close(&self) -> Result<()> {
        let name = tomb_name(&self.tomb_file)?;
        run_tomb(&self.store_root, ["close", &name])
    }

    pub fn status(&self) -> Result<bool> {
        let name = tomb_name(&self.tomb_file)?;
        let output = Command::new("tomb")
            .current_dir(&self.store_root)
            .env("ANCHOR_STORE_ROOT", &self.store_root)
            .arg("status")
            .arg(name)
            .output()
            .context("failed to invoke tomb status")?;

        if !output.status.success() {
            bail!("tomb status failed");
        }

        let stdout =
            String::from_utf8(output.stdout).context("tomb status output was not utf-8")?;
        Ok(stdout.trim().eq_ignore_ascii_case("open"))
    }
}

pub fn vault_paths(store_root: &Path) -> VaultPaths {
    let name = store_root
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("anchor");
    let parent = store_root.parent().unwrap_or_else(|| Path::new("."));
    let tomb_file = parent.join(format!("{name}.tomb"));
    let tomb_key_file = parent.join(format!("{name}.tomb.key"));

    VaultPaths {
        store_root: store_root.to_path_buf(),
        tomb_file,
        tomb_key_file,
    }
}

fn run_tomb<I, S>(store_root: &Path, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = Command::new("tomb")
        .current_dir(store_root.parent().unwrap_or_else(|| Path::new(".")))
        .env("ANCHOR_STORE_ROOT", store_root)
        .args(args)
        .status()
        .context("failed to invoke tomb")?;

    if status.success() {
        Ok(())
    } else {
        bail!("tomb command failed");
    }
}

fn tomb_name(path: &Path) -> Result<String> {
    let stem = path
        .file_stem()
        .and_then(|part| part.to_str())
        .ok_or_else(|| anyhow!("failed to determine tomb name"))?;
    Ok(stem.to_string())
}
