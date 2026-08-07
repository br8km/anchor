use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("anchor").expect("binary exists")
}

fn store_path(dir: &TempDir) -> PathBuf {
    dir.path().join("vault")
}

fn fake_tomb(dir: &TempDir) -> PathBuf {
    let tomb = dir.path().join("tomb");
    let script = r#"#!/bin/sh
set -eu
cmd="$1"
shift || true
case "$cmd" in
  dig)
    tomb_file="$1"
    mkdir -p "$(dirname "$tomb_file")"
    : > "$tomb_file"
    ;;
  forge)
    key_file="$1"
    mkdir -p "$(dirname "$key_file")"
    : > "$key_file"
    ;;
  lock)
    :
    ;;
  open)
    tomb_file="$1"
    name="$(basename "$tomb_file" .tomb)"
    root="${ANCHOR_STORE_ROOT:?}"
    marker="$(dirname "$root")/.${name}.vault-open"
    : > "$marker"
    ;;
  close)
    name="$1"
    root="${ANCHOR_STORE_ROOT:?}"
    marker="$(dirname "$root")/.${name}.vault-open"
    rm -f "$marker"
    ;;
  status)
    name="$1"
    root="${ANCHOR_STORE_ROOT:?}"
    marker="$(dirname "$root")/.${name}.vault-open"
    if [ -f "$marker" ]; then
      printf '%s\n' open
    else
      printf '%s\n' closed
    fi
    ;;
  *)
    exit 1
    ;;
esac
"#;
    fs::write(&tomb, script).expect("write tomb script");
    let mut perms = fs::metadata(&tomb).expect("tomb metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tomb, perms).expect("chmod tomb script");
    tomb
}

fn command_with_env(dir: &TempDir, store: &Path) -> Command {
    let mut cmd = bin();
    cmd.env("ANCHOR_TOMB_BIN", fake_tomb(dir))
        .args(["--store", store.to_str().expect("store path")]);
    cmd
}

#[test]
fn init_creates_layout_and_reports_success() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    command_with_env(&tmp, &store)
        .args(["init"])
        .assert()
        .success()
        .stdout(predicates::str::contains("initialized store"));

    assert!(store.is_dir(), "store root should exist");
    assert!(store.join(".git").is_dir(), "git repository should exist");
    assert!(
        store.join(".gpg-id").is_file(),
        "recipient metadata should exist"
    );

    let parent = store.parent().expect("parent");
    let tomb = parent.join("vault.tomb");
    let tomb_key = parent.join("vault.tomb.key");
    assert!(tomb.is_file(), "tomb container should exist");
    assert!(tomb_key.is_file(), "tomb key should exist");

    command_with_env(&tmp, &store)
        .args(["vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));
}

#[test]
fn vault_open_and_close_toggle_status() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    command_with_env(&tmp, &store)
        .args(["init"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .args(["vault", "open"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault opened"));

    command_with_env(&tmp, &store)
        .args(["vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault open"));

    command_with_env(&tmp, &store)
        .args(["vault", "close"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));

    command_with_env(&tmp, &store)
        .args(["vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));
}

#[test]
fn mutating_vault_commands_fail_on_dirty_git_state() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    command_with_env(&tmp, &store)
        .args(["init"])
        .assert()
        .success();

    fs::write(store.join("dirty.txt"), b"dirty").expect("write dirty file");

    command_with_env(&tmp, &store)
        .args(["vault", "open"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git working tree is dirty"));
}

#[test]
fn init_rejects_non_empty_target_directory() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    fs::create_dir_all(&store).expect("store dir");
    fs::write(store.join("file.txt"), b"x").expect("seed file");

    command_with_env(&tmp, &store)
        .args(["init"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("store path is not empty"));
}
