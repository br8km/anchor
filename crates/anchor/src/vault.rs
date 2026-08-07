use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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
    pub fn create_placeholders(&self) -> Result<()> {
        if let Some(parent) = self.tomb_file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create tomb parent {}", parent.display()))?;
        }

        if !self.tomb_file.exists() {
            fs::write(&self.tomb_file, b"")
                .with_context(|| format!("failed to write {}", self.tomb_file.display()))?;
        }

        if !self.tomb_key_file.exists() {
            fs::write(&self.tomb_key_file, b"")
                .with_context(|| format!("failed to write {}", self.tomb_key_file.display()))?;
        }

        Ok(())
    }

    pub fn ensure_container_exists(&self) -> Result<()> {
        if !self.tomb_file.exists() || !self.tomb_key_file.exists() {
            anyhow::bail!("tomb container is missing");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct VaultMarker {
    path: PathBuf,
}

impl VaultMarker {
    pub fn open(&self) -> Result<()> {
        fs::write(&self.path, b"open")
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .with_context(|| format!("failed to remove {}", self.path.display()))?;
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.close()
    }

    pub fn is_open(&self) -> bool {
        self.path.exists()
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

pub fn vault_marker_path(store_root: &Path) -> VaultMarker {
    let name = store_root
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("anchor");
    let parent = store_root.parent().unwrap_or_else(|| Path::new("."));
    VaultMarker {
        path: parent.join(format!(".{name}.vault-open")),
    }
}
