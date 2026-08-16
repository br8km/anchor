use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("anchor").expect("binary exists")
}

fn store_path(dir: &TempDir) -> PathBuf {
    dir.path().join("vault")
}

fn fake_binary(dir: &TempDir, name: &str, script: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, script).expect("write fake binary");
    let mut perms = fs::metadata(&path)
        .expect("fake binary metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod fake binary");
    path
}

fn fake_tomb(dir: &TempDir) -> PathBuf {
    let script = r#"#!/bin/sh
set -eu
log_cmd() {
  if [ -n "${TOMB_LOG:-}" ]; then
    printf '%s\n' "$*" >> "$TOMB_LOG"
  fi
}
cmd="$1"
shift || true
log_cmd "$cmd $*"
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
    fake_binary(dir, "tomb", script)
}

fn fake_gpg(dir: &TempDir) -> PathBuf {
    let script = r#"#!/bin/sh
set -eu
log_cmd() {
  if [ -n "${GPG_LOG:-}" ]; then
    printf '%s\n' "$*" >> "$GPG_LOG"
  fi
}
mode=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --encrypt|--decrypt)
      mode="$1"
      shift
      break
      ;;
    --trust-model)
      shift 2
      ;;
    --recipient)
      shift 2
      ;;
    --batch|--yes|always)
      shift
      ;;
    *)
      shift
      ;;
  esac
done
log_cmd "$mode $*"
case "$mode" in
  --encrypt)
    cat
    ;;
  --decrypt)
    cat "$1"
    ;;
  *)
    exit 1
    ;;
esac
"#;
    fake_binary(dir, "gpg", script)
}

fn fake_clipboard(dir: &TempDir) -> PathBuf {
    let script = r#"#!/bin/sh
set -eu
if [ -n "${CLIPBOARD_LOG:-}" ]; then
  printf '%s\n' "$*" >> "$CLIPBOARD_LOG"
fi
cat > "${CLIPBOARD_DATA:-/dev/null}"
"#;
    fake_binary(dir, "wl-copy", script)
}

fn command_with_env(dir: &TempDir, store: &Path) -> Command {
    let bin_dir = fake_tomb(dir).parent().expect("bin dir").to_path_buf();
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut new_path = std::ffi::OsString::from(&bin_dir);
    new_path.push(":");
    new_path.push(current_path);

    let mut cmd = bin();
    cmd.env("PATH", new_path)
        .args(["--store", store.to_str().expect("store path")]);
    cmd
}

fn command_with_clipboard(dir: &TempDir, store: &Path) -> Command {
    let bin_dir = fake_clipboard(dir).parent().expect("bin dir").to_path_buf();
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut new_path = std::ffi::OsString::from(&bin_dir);
    new_path.push(":");
    new_path.push(current_path);

    let mut cmd = bin();
    cmd.env("PATH", new_path).args([
        "--store",
        store.to_str().expect("store path"),
        "--clipboard-timeout-ms",
        "1",
    ]);
    cmd
}

#[test]
fn add_edit_generate_and_remove_secret_entries() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    let add_body = "first-secret\nurl=https://example.test\nnotes=keep\n";
    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin(add_body)
        .assert()
        .success()
        .stdout(predicates::str::contains("added secret"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read added secret"),
        add_body,
        "add should preserve the provided entry body"
    );

    let edit_body = "second-secret\n";
    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["edit", secret_name])
        .write_stdin(edit_body)
        .assert()
        .success()
        .stdout(predicates::str::contains("edited secret"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read edited secret"),
        "second-secret\nurl=https://example.test\nnotes=keep\n",
        "edit should replace only the first line and preserve metadata"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["generate", secret_name])
        .assert()
        .success()
        .stdout(predicates::str::contains("generated secret"));

    let generated = fs::read_to_string(&secret_path).expect("read generated secret");
    let mut lines = generated.lines();
    let first_line = lines.next().expect("generated first line");
    assert!(
        !first_line.is_empty(),
        "generated secret should not be empty"
    );
    assert_eq!(
        lines.collect::<Vec<_>>(),
        vec!["url=https://example.test", "notes=keep"],
        "generate should preserve metadata lines"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["remove", secret_name])
        .assert()
        .success()
        .stdout(predicates::str::contains("removed secret"));

    assert!(!secret_path.exists(), "remove should delete the entry");

    let tomb_events = fs::read_to_string(&tomb_log).expect("read tomb log");
    assert!(
        tomb_events.lines().any(|line| line.starts_with("open ")),
        "secret commands should open the vault automatically"
    );
    assert!(
        tomb_events.lines().last() == Some("close vault"),
        "secret commands should close the vault after mutation"
    );
}

#[test]
fn secret_operations_do_not_log_plaintext() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret = "top-secret-password";

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "web/example"])
        .write_stdin(format!("{secret}\nurl=https://example.test\n"))
        .assert()
        .success();

    let gpg_events = fs::read_to_string(&gpg_log).expect("read gpg log");
    let tomb_events = fs::read_to_string(&tomb_log).expect("read tomb log");
    assert!(
        !gpg_events.contains(secret),
        "plaintext should not appear in gpg command logs"
    );
    assert!(
        !tomb_events.contains(secret),
        "plaintext should not appear in tomb command logs"
    );
}

#[test]
fn show_list_grep_and_copy_secret_entries() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let clip_log = tmp.path().join("clip.log");
    let clip_data = tmp.path().join("clip.data");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "services/email"])
        .write_stdin("first-secret\nurl=https://example.test\nnotes=keep\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "personal/blog"])
        .write_stdin("blog-secret\nurl=https://blog.example\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["show", "services/email"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first-secret"))
        .stdout(predicates::str::contains("url").not());

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("personal/blog"))
        .stdout(predicates::str::contains("services/email"));

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["grep", "blog"])
        .assert()
        .success()
        .stdout(predicates::str::contains("personal/blog"))
        .stdout(predicates::str::contains("services/email").not());

    command_with_clipboard(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .env("CLIPBOARD_LOG", &clip_log)
        .env("CLIPBOARD_DATA", &clip_data)
        .args(["copy", "services/email"])
        .assert()
        .success()
        .stdout(predicates::str::contains("copied secret"));

    std::thread::sleep(Duration::from_millis(20));

    assert_eq!(
        fs::read_to_string(&clip_data).expect("read clipboard data"),
        "",
        "clipboard should be cleared after the timeout"
    );
}

#[test]
fn metadata_only_view_and_edit_secret_entries() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("first-secret\nurl=https://example.test\nnotes=keep\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["meta", secret_name])
        .assert()
        .success()
        .stdout(predicates::str::contains("url=https://example.test"))
        .stdout(predicates::str::contains("notes=keep"))
        .stdout(predicates::str::contains("first-secret").not());

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["metaedit", secret_name])
        .write_stdin("url=https://new.example\nnotes=updated\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("edited metadata"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read edited metadata"),
        "first-secret\nurl=https://new.example\nnotes=updated\n",
        "metaedit should preserve the first line and replace the metadata lines"
    );

    let tomb_events = fs::read_to_string(&tomb_log).expect("read tomb log");
    assert!(
        tomb_events.lines().any(|line| line.starts_with("open ")),
        "metadata commands should open the vault automatically"
    );
    assert!(
        tomb_events.lines().last() == Some("close vault"),
        "metadata commands should close the vault after use"
    );
}

#[test]
fn metadata_edit_rejects_ambiguous_existing_metadata() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("first-secret\nurl=https://example.test\nURL=https://other.example\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["metaedit", secret_name])
        .write_stdin("notes=updated\n")
        .assert()
        .failure()
        .stderr(predicates::str::contains("ambiguous"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read untouched metadata"),
        "first-secret\nurl=https://example.test\nURL=https://other.example\n",
        "ambiguous existing metadata should prevent editing and keep the entry unchanged"
    );
}

#[test]
fn password_update_requires_confirmation_before_changing_secret() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("first-secret\nurl=https://example.test\nnotes=keep\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["update", secret_name])
        .write_stdin("n\n")
        .assert()
        .failure()
        .stderr(predicates::str::contains("update cancelled"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read untouched secret"),
        "first-secret\nurl=https://example.test\nnotes=keep\n",
        "refusing confirmation should leave the entry unchanged"
    );
}

#[test]
fn password_update_replaces_first_line_by_default() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("first-secret\nurl=https://example.test\nnotes=keep\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["update", secret_name])
        .write_stdin("y\nrotated-secret\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("first-secret"))
        .stdout(predicates::str::contains("updated secret at"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read updated secret"),
        "rotated-secret\nurl=https://example.test\nnotes=keep\n",
        "default update should replace only the first line"
    );
}

#[test]
fn password_update_supports_directory_glob_and_multiline() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "services/email"])
        .write_stdin("email-secret\nurl=https://email.example\nnotes=keep-email\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "services/chat"])
        .write_stdin("chat-secret\nurl=https://chat.example\nnotes=keep-chat\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "personal/blog"])
        .write_stdin("blog-secret\nurl=https://blog.example\nnotes=keep-blog\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["update", "services"])
        .write_stdin("y\nshared-secret\n")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(store.join("services/email.gpg")).expect("read updated email"),
        "shared-secret\nurl=https://email.example\nnotes=keep-email\n",
        "directory targeting should update every matching secret"
    );
    assert_eq!(
        fs::read_to_string(store.join("services/chat.gpg")).expect("read updated chat"),
        "shared-secret\nurl=https://chat.example\nnotes=keep-chat\n",
        "directory targeting should update every matching secret"
    );
    assert_eq!(
        fs::read_to_string(store.join("personal/blog.gpg")).expect("read untouched blog"),
        "blog-secret\nurl=https://blog.example\nnotes=keep-blog\n",
        "directory targeting should not affect other branches"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["update", "personal/*", "--multiline"])
        .write_stdin("y\nmultiline-secret\nurl=https://new.example\nnotes=updated\n")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(store.join("personal/blog.gpg")).expect("read multiline blog"),
        "multiline-secret\nurl=https://new.example\nnotes=updated\n",
        "multiline update mode should replace the full entry body"
    );
}

#[test]
fn password_update_treats_literal_glob_characters_as_paths() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let secret_name = "team[old]/api*key";
    let secret_path = store.join("team[old]/api*key.gpg");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("literal-secret\nurl=https://example.test\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["update", secret_name])
        .write_stdin("y\nrotated-literal-secret\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("literal-secret"))
        .stdout(predicates::str::contains("updated secret at"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read literal glob secret"),
        "rotated-literal-secret\nurl=https://example.test\n",
        "literal path targets should still work when the name contains glob metacharacters"
    );
}
