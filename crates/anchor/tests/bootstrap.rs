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
    fs::write(&tomb, script).expect("write tomb script");
    let mut perms = fs::metadata(&tomb).expect("tomb metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tomb, perms).expect("chmod tomb script");
    tomb
}

fn fake_gpg(dir: &TempDir) -> PathBuf {
    let gpg = dir.path().join("gpg");
    let script = r#"#!/bin/sh
set -eu
mode=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --encrypt|--decrypt)
      mode="$1"
      shift
      ;;
    --trust-model|--recipient)
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
case "$mode" in
  --encrypt|--decrypt)
    cat
    ;;
  *)
    exit 1
    ;;
esac
"#;
    fs::write(&gpg, script).expect("write gpg script");
    let mut perms = fs::metadata(&gpg).expect("gpg metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&gpg, perms).expect("chmod gpg script");
    gpg
}

fn tomb_log(dir: &TempDir) -> PathBuf {
    dir.path().join("tomb.log")
}

fn command_with_env(dir: &TempDir, store: &Path) -> Command {
    let tomb_dir = fake_tomb(dir).parent().expect("tomb dir").to_path_buf();
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut new_path = std::ffi::OsString::from(&tomb_dir);
    new_path.push(":");
    new_path.push(current_path);

    let mut cmd = bin();
    cmd.env("PATH", new_path)
        .args(["--store", store.to_str().expect("store path")]);
    cmd
}

#[test]
fn init_creates_layout_and_reports_success() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let log = tomb_log(&tmp);
    let recipient = "alice@example.com";

    command_with_env(&tmp, &store)
        .env("TOMB_LOG", &log)
        .args(["init", "--recipient", recipient])
        .assert()
        .success()
        .stdout(predicates::str::contains("initialized store"));

    assert!(store.is_dir(), "store root should exist");
    assert!(store.join(".git").is_dir(), "git repository should exist");
    assert_eq!(
        fs::read_to_string(store.join(".gpg-id")).expect("read recipient metadata"),
        format!("{recipient}\n"),
        "recipient metadata should include the selected recipient"
    );

    let parent = store.parent().expect("parent");
    let tomb = parent.join("vault.tomb");
    let tomb_key = parent.join("vault.tomb.key");
    assert!(tomb.is_file(), "tomb container should exist");
    assert!(tomb_key.is_file(), "tomb key should exist");
    let parent = store.parent().expect("parent");
    let tomb_file = parent.join("vault.tomb");
    let tomb_key_file = parent.join("vault.tomb.key");
    let tomb_events = fs::read_to_string(&log).expect("read tomb log");
    assert_eq!(
        tomb_events.lines().collect::<Vec<_>>(),
        vec![
            format!("dig {} -s 10", tomb_file.display()),
            format!("forge {} -gr {recipient}", tomb_key_file.display()),
            format!(
                "lock {} -k {} -gr {recipient}",
                tomb_file.display(),
                tomb_key_file.display()
            ),
            format!(
                "open {} -k {} -p {}",
                tomb_file.display(),
                tomb_key_file.display(),
                store.display()
            ),
            "close vault".to_string(),
        ],
        "init should create, open, and close the tomb during bootstrap",
    );

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
        .args(["init", "--recipient", "alice@example.com"])
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
        .args(["init", "--recipient", "alice@example.com"])
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

fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git command");

    assert!(
        output.status.success(),
        "git {:?} failed: stdout={:?} stderr={:?}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("git output utf-8")
}

fn git_current_branch(dir: &Path) -> String {
    git_output(dir, &["branch", "--show-current"])
        .trim()
        .to_string()
}

fn git_rev_parse(dir: &Path, reference: &str) -> String {
    git_output(dir, &["rev-parse", reference])
        .trim()
        .to_string()
}

fn git_init_bare(dir: &Path) {
    let status = std::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg("-q")
        .arg(dir)
        .status()
        .expect("run git init --bare");
    assert!(status.success(), "git init --bare failed");
}

fn git_clone(remote: &Path, clone_dir: &Path) {
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg(remote)
        .arg(clone_dir)
        .status()
        .expect("run git clone");
    assert!(status.success(), "git clone failed");
}

fn git_commit_and_push(repo: &Path, branch: &str, file_name: &str, body: &str) {
    fs::write(repo.join(file_name), body).expect("write git commit file");

    let status = std::process::Command::new("git")
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "anchor")
        .env("GIT_AUTHOR_EMAIL", "anchor@example.com")
        .env("GIT_COMMITTER_NAME", "anchor")
        .env("GIT_COMMITTER_EMAIL", "anchor@example.com")
        .args(["add", file_name])
        .status()
        .expect("git add");
    assert!(status.success(), "git add failed");

    let status = std::process::Command::new("git")
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "anchor")
        .env("GIT_AUTHOR_EMAIL", "anchor@example.com")
        .env("GIT_COMMITTER_NAME", "anchor")
        .env("GIT_COMMITTER_EMAIL", "anchor@example.com")
        .args(["commit", "-q", "-m", "Remote update"])
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit failed");

    let status = std::process::Command::new("git")
        .current_dir(repo)
        .args(["push", "origin", "HEAD"])
        .status()
        .expect("git push");
    assert!(status.success(), "git push failed for branch {branch}");
}

#[test]
fn sync_status_reports_remote_state() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let remote = tmp.path().join("remote.git");
    let _gpg = fake_gpg(&tmp);

    git_init_bare(&remote);

    command_with_env(&tmp, &store)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    let branch = git_current_branch(&store);
    git_output(
        &store,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("remote path"),
        ],
    );

    command_with_env(&tmp, &store)
        .args(["sync", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sync status for"))
        .stdout(predicates::str::contains(&branch))
        .stdout(predicates::str::contains("state: clean"))
        .stdout(predicates::str::contains("remote: origin"))
        .stdout(predicates::str::contains("remote branch: missing"));
}

#[test]
fn sync_pushes_and_pulls_remote_commits() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let remote = tmp.path().join("remote.git");
    let remote_clone = tmp.path().join("remote-clone");
    let _gpg = fake_gpg(&tmp);

    git_init_bare(&remote);

    command_with_env(&tmp, &store)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    let branch = git_current_branch(&store);
    git_output(
        &store,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("remote path"),
        ],
    );
    let local_head_before_sync = git_rev_parse(&store, "HEAD");

    command_with_env(&tmp, &store)
        .args(["sync"])
        .assert()
        .success()
        .stdout(predicates::str::contains("synced store at"))
        .stdout(predicates::str::contains("pushed local commits"));

    assert_eq!(
        git_rev_parse(&store, "HEAD"),
        local_head_before_sync,
        "initial sync should leave the local branch unchanged"
    );
    assert_eq!(
        git_rev_parse(&remote, &format!("refs/heads/{branch}")),
        local_head_before_sync,
        "initial sync should push the local branch to the remote"
    );

    git_clone(&remote, &remote_clone);
    git_commit_and_push(&remote_clone, &branch, "remote.txt", "remote change");
    let remote_head = git_rev_parse(&remote, &format!("refs/heads/{branch}"));

    command_with_env(&tmp, &store)
        .args(["sync"])
        .assert()
        .success()
        .stdout(predicates::str::contains("pulled latest remote changes"))
        .stdout(predicates::str::contains("pushed local commits"));

    assert_eq!(
        git_rev_parse(&store, "HEAD"),
        remote_head,
        "sync should fast-forward the local branch to the remote head"
    );
}

#[test]
fn sync_fails_closed_on_dirty_or_divergent_state() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);
    let remote = tmp.path().join("remote.git");
    let remote_clone = tmp.path().join("remote-divergent");
    let _gpg = fake_gpg(&tmp);

    git_init_bare(&remote);

    command_with_env(&tmp, &store)
        .args(["init", "--recipient", "alice@example.com"])
        .assert()
        .success();

    let branch = git_current_branch(&store);
    git_output(
        &store,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("remote path"),
        ],
    );

    fs::write(store.join("dirty.txt"), b"dirty").expect("write dirty file");
    command_with_env(&tmp, &store)
        .args(["sync"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git working tree is dirty"));

    fs::remove_file(store.join("dirty.txt")).expect("remove dirty file");
    command_with_env(&tmp, &store)
        .args(["sync"])
        .assert()
        .success();

    git_clone(&remote, &remote_clone);
    git_commit_and_push(
        &remote_clone,
        &branch,
        "remote-only.txt",
        "remote-only change",
    );

    command_with_env(&tmp, &store)
        .args(["add", "local-only"])
        .write_stdin("local-secret\n")
        .assert()
        .success();

    command_with_env(&tmp, &store)
        .args(["sync"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "git repository has diverged from remote",
        ));
}
