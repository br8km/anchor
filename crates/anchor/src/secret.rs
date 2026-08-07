use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use rand::{distributions::Alphanumeric, Rng};

pub fn entry_path(store_root: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;

    let mut path = store_root.to_path_buf();
    let mut segments = name.split('/').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_some() {
            path.push(segment);
        } else {
            path.push(format!("{segment}.gpg"));
        }
    }

    Ok(path)
}

pub fn decrypt_entry(path: &Path) -> Result<String> {
    let output = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--decrypt")
        .arg(path)
        .output()
        .with_context(|| format!("failed to decrypt {}", path.display()))?;

    if !output.status.success() {
        bail!("failed to decrypt {}", path.display());
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("{} was not valid utf-8", path.display()))
}

pub fn encrypt_entry(path: &Path, recipients: &[String], plaintext: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut command = Command::new("gpg");
    command
        .arg("--batch")
        .arg("--yes")
        .arg("--trust-model")
        .arg("always")
        .arg("--encrypt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for recipient in recipients {
        command.arg("--recipient").arg(recipient);
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to encrypt {}", path.display()))?;

    {
        let mut stdin = child.stdin.take().context("failed to open gpg stdin")?;
        stdin
            .write_all(normalize_entry_text(plaintext).as_bytes())
            .with_context(|| format!("failed to write plaintext for {}", path.display()))?;
    }

    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to finish encrypting {}", path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to encrypt {}: {}", path.display(), stderr.trim());
    }

    fs::write(path, output.stdout).with_context(|| format!("failed to write {}", path.display()))
}

pub fn remove_entry(path: &Path) -> Result<()> {
    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
}

pub fn generate_secret() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

pub fn replace_first_line(existing: &str, replacement: &str) -> String {
    let mut updated = String::from(replacement);
    if let Some((_, tail)) = existing.split_once('\n') {
        updated.push('\n');
        updated.push_str(tail);
    } else {
        updated.push('\n');
    }
    updated
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("secret name is required");
    }

    if name.starts_with('/') {
        bail!("secret name must be relative");
    }

    if name.contains('\0') {
        return Err(anyhow!("secret name contains a null byte"));
    }

    for segment in name.split('/') {
        if segment.is_empty() {
            bail!("secret name must not contain empty path segments");
        }
        if segment == "." || segment == ".." {
            bail!("secret name must not contain traversal segments");
        }
    }

    Ok(())
}

fn normalize_entry_text(text: &str) -> String {
    let mut body = text.to_string();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_path_rejects_traversal() {
        let err =
            entry_path(Path::new("/tmp/store"), "../secret").expect_err("should reject traversal");
        assert!(err.to_string().contains("traversal"));
    }

    #[test]
    fn entry_path_appends_gpg_extension_to_leaf() {
        let path = entry_path(Path::new("/tmp/store"), "service/login").expect("path");
        assert_eq!(path, Path::new("/tmp/store/service/login.gpg"));
    }

    #[test]
    fn entry_path_allows_leading_dash_segments() {
        let path = entry_path(Path::new("/tmp/store"), "-prod/api").expect("path");
        assert_eq!(path, Path::new("/tmp/store/-prod/api.gpg"));
    }

    #[test]
    fn replace_first_line_preserves_metadata_tail() {
        let updated = replace_first_line("old\nuser=alice\nnotes=ok\n", "new");
        assert_eq!(updated, "new\nuser=alice\nnotes=ok\n");
    }
}
