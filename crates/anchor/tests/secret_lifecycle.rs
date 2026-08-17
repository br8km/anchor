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
recipients=""
target=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --encrypt|--decrypt)
      mode="$1"
      shift
      ;;
    --trust-model)
      shift 2
      ;;
    --recipient)
      recipients="${recipients}${recipients:+,}$2"
      shift 2
      ;;
    --batch|--yes|always)
      shift
      ;;
    *)
      if [ -z "$target" ]; then
        target="$1"
      fi
      shift
      ;;
  esac
done
log_cmd "$mode recipients=${recipients:-none} ${target:-}"
case "$mode" in
  --encrypt)
    cat
    ;;
  --decrypt)
    cat "$target"
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
fn recipient_management_lists_and_reencrypts_secret_entries() {
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
        .args(["recipients", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("alice@example.com"));

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["recipients", "add", "bob@example.com"])
        .assert()
        .success()
        .stdout(predicates::str::contains("added recipient bob@example.com"));

    assert_eq!(
        fs::read_to_string(store.join(".gpg-id")).expect("read updated recipients"),
        "alice@example.com\nbob@example.com\n",
        "adding a recipient should update the vault recipient metadata"
    );
    assert_eq!(
        fs::read_to_string(&secret_path).expect("read reencrypted secret"),
        "first-secret\nurl=https://example.test\nnotes=keep\n",
        "adding a recipient should re-encrypt the existing secret without changing its body"
    );

    let gpg_events = fs::read_to_string(&gpg_log).expect("read gpg log");
    assert!(
        gpg_events.contains("recipients=alice@example.com,bob@example.com"),
        "recipient rotation should re-encrypt existing entries with the expanded recipient set"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["recipients", "remove", "alice@example.com"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "removed recipient alice@example.com",
        ));

    assert_eq!(
        fs::read_to_string(store.join(".gpg-id")).expect("read reduced recipients"),
        "bob@example.com\n",
        "removing a recipient should shrink the vault recipient metadata"
    );
    assert_eq!(
        fs::read_to_string(&secret_path).expect("read rotated secret"),
        "first-secret\nurl=https://example.test\nnotes=keep\n",
        "removing the old recipient should keep the secret body intact after re-encryption"
    );

    let gpg_events = fs::read_to_string(&gpg_log).expect("read gpg log after removal");
    assert!(
        gpg_events.contains("recipients=bob@example.com"),
        "recipient removal should re-encrypt remaining entries for the surviving recipient set"
    );
}

#[test]
fn recipient_management_rejects_removing_the_last_recipient() {
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
        .args(["recipients", "remove", "alice@example.com"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "at least one recipient is required",
        ));

    assert_eq!(
        fs::read_to_string(store.join(".gpg-id")).expect("read original recipients"),
        "alice@example.com\n",
        "removing the final recipient should leave the vault recipient set unchanged"
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
fn otp_lifecycle_stores_canonical_uri_and_generates_codes() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let clip_log = tmp.path().join("clip.log");
    let clip_data = tmp.path().join("clip.data");
    let secret_name = "services/email";
    let secret_path = store.join("services/email.gpg");
    let canonical_uri =
        "otpauth://totp/services%2Femail?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30";

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
        .write_stdin("first-secret\nurl=https://example.test\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["otp", "add", secret_name])
        .write_stdin("jbswy3dpehpk3pxp\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("stored TOTP data"));

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read otp entry"),
        format!("first-secret\nurl=https://example.test\notp={canonical_uri}\n"),
        "otp add should preserve existing metadata and store the canonical URI"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["otp", "uri", secret_name])
        .assert()
        .success()
        .stdout(predicates::str::contains(canonical_uri));

    let uri_output = command_with_clipboard(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .env("CLIPBOARD_LOG", &clip_log)
        .env("CLIPBOARD_DATA", &clip_data)
        .args(["otp", "uri", secret_name, "--clipboard"])
        .output()
        .expect("run otp uri");

    assert!(
        uri_output.status.success(),
        "otp uri command should succeed"
    );
    assert_eq!(
        fs::read_to_string(&clip_data).expect("read URI clipboard data"),
        canonical_uri,
        "otp uri should copy the canonical URI to the clipboard"
    );

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["otp", "validate", canonical_uri])
        .assert()
        .success();

    let output = command_with_clipboard(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .env("CLIPBOARD_LOG", &clip_log)
        .env("CLIPBOARD_DATA", &clip_data)
        .args(["otp", "code", secret_name, "--clipboard"])
        .output()
        .expect("run otp code");

    assert!(output.status.success(), "otp code command should succeed");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let code = stdout.trim();
    assert_eq!(code.len(), 6, "otp code should be six digits by default");
    assert!(
        code.chars().all(|ch| ch.is_ascii_digit()),
        "otp code should contain only digits"
    );

    assert_eq!(
        fs::read_to_string(&clip_data).expect("read clipboard data"),
        code,
        "clipboard should receive the current TOTP code"
    );
}

#[test]
fn otp_add_preserves_existing_metadata_key_spelling() {
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
        .write_stdin("first-secret\nOTP=legacy\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["otp", "add", secret_name])
        .write_stdin("jbswy3dpehpk3pxp\n")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&secret_path).expect("read preserved-key entry"),
        "first-secret\nOTP=otpauth://totp/services%2Femail?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30\n",
        "otp add should preserve the original metadata key spelling"
    );
}

#[test]
fn otp_validate_rejects_hotp_uris() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let _gpg = fake_gpg(&tmp);

    command_with_env(&tmp, &store)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .args([
            "otp",
            "validate",
            "otpauth://hotp/account?secret=JBSWY3DPEHPK3PXP&counter=1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("only TOTP"));
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

#[test]
fn migration_json_and_csv_round_trip_secret_metadata_and_totp_data() {
    let tmp = TempDir::new().expect("tempdir");
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let store_one = tmp.path().join("vault-one");
    let store_two = tmp.path().join("vault-two");
    let store_three = tmp.path().join("vault-three");
    let json_path = tmp.path().join("vault.json");
    let csv_path = tmp.path().join("vault.csv");
    let secret_name = "services/email";
    let canonical_uri =
        "otpauth://totp/services%2Femail?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30";
    let entry_body =
        format!("first-secret\nurl=https://example.test\nnotes=keep\notp={canonical_uri}\n");

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", secret_name])
        .write_stdin("first-secret\nurl=https://example.test\nnotes=keep\n")
        .assert()
        .success();

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["otp", "add", secret_name])
        .write_stdin("jbswy3dpehpk3pxp\n")
        .assert()
        .success();

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["export", json_path.to_str().expect("json path")])
        .assert()
        .success()
        .stdout(predicates::str::contains("exported 1 secret"));

    let json_export = fs::read_to_string(&json_path).expect("read JSON export");
    assert!(
        json_export.contains("\"name\": \"services/email\""),
        "JSON export should include the entry name"
    );
    assert!(
        json_export.contains("otpauth://totp/services%2Femail"),
        "JSON export should preserve canonical TOTP data"
    );

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["import", json_path.to_str().expect("json path")])
        .assert()
        .success()
        .stdout(predicates::str::contains("imported 1 secret"));

    assert_eq!(
        fs::read_to_string(store_two.join("services/email.gpg")).expect("read imported JSON"),
        entry_body,
        "JSON import should round-trip the secret text and metadata"
    );

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["export", csv_path.to_str().expect("csv path")])
        .assert()
        .success()
        .stdout(predicates::str::contains("exported 1 secret"));

    let csv_export = fs::read_to_string(&csv_path).expect("read CSV export");
    assert!(
        csv_export.contains("services/email"),
        "CSV export should include the entry name"
    );
    assert!(
        csv_export.contains("otpauth://totp/services%2Femail"),
        "CSV export should preserve canonical TOTP data"
    );

    command_with_env(&tmp, &store_three)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store_three)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["import", csv_path.to_str().expect("csv path")])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(store_three.join("services/email.gpg")).expect("read imported CSV"),
        entry_body,
        "CSV import should round-trip the secret text and metadata"
    );
}

#[test]
fn migration_collision_policy_rejects_plain_import_and_allows_overwrite_or_rename() {
    let tmp = TempDir::new().expect("tempdir");
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let store_one = tmp.path().join("vault-one");
    let store_two = tmp.path().join("vault-two");
    let import_path = tmp.path().join("collision.json");
    let original_body = "original-secret\nurl=https://example.test\n";
    let imported_body = "imported-secret\nurl=https://import.example\nnotes=updated\n";
    let collision_json = "[\n  {\n    \"name\": \"services/email\",\n    \"secret\": \"imported-secret\",\n    \"metadata\": \"url=https://import.example\\nnotes=updated\\n\"\n  },\n  {\n    \"name\": \"services/blog\",\n    \"secret\": \"blog-secret\",\n    \"metadata\": \"url=https://blog.example\\n\"\n  }\n]\n"
        .to_string();

    fs::write(&import_path, collision_json).expect("write collision import");

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "services/email"])
        .write_stdin(original_body)
        .assert()
        .success();

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["import", import_path.to_str().expect("import path")])
        .assert()
        .failure()
        .stderr(predicates::str::contains("collision"));

    assert_eq!(
        fs::read_to_string(store_one.join("services/email.gpg")).expect("read unchanged secret"),
        original_body,
        "plain import should leave the colliding entry untouched"
    );

    command_with_env(&tmp, &store_one)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args([
            "import",
            import_path.to_str().expect("import path"),
            "--overwrite",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(store_one.join("services/email.gpg")).expect("read overwritten secret"),
        imported_body,
        "overwrite should replace the colliding entry"
    );

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["add", "services/email"])
        .write_stdin(original_body)
        .assert()
        .success();

    command_with_env(&tmp, &store_two)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args([
            "import",
            import_path.to_str().expect("import path"),
            "--rename",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(store_two.join("services/email.gpg")).expect("read original secret"),
        original_body,
        "rename should keep the original colliding entry"
    );
    assert_eq!(
        fs::read_to_string(store_two.join("services/email-imported.gpg"))
            .expect("read renamed import"),
        imported_body,
        "rename should move the colliding entry to a new name"
    );
    assert!(
        store_two.join("services/blog.gpg").exists(),
        "rename should still import non-colliding entries"
    );
}

#[test]
fn migration_rejects_unsupported_file_extensions() {
    let tmp = TempDir::new().expect("tempdir");
    let _gpg = fake_gpg(&tmp);
    let gpg_log = tmp.path().join("gpg.log");
    let tomb_log = tmp.path().join("tomb.log");
    let store = store_path(&tmp);
    let unsupported = tmp.path().join("archive.txt");

    fs::write(&unsupported, "not a supported migration file").expect("write unsupported file");

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["import", unsupported.to_str().expect("unsupported path")])
        .assert()
        .failure()
        .stderr(predicates::str::contains(".json or .csv"));

    command_with_env(&tmp, &store)
        .env("GPG_LOG", &gpg_log)
        .env("TOMB_LOG", &tomb_log)
        .args(["export", unsupported.to_str().expect("unsupported path")])
        .assert()
        .failure()
        .stderr(predicates::str::contains(".json or .csv"));
}
