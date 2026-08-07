use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("anchor").expect("binary exists")
}

fn store_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("vault")
}

#[test]
fn init_creates_layout_and_reports_success() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    bin()
        .args(["--store", store.to_str().unwrap(), "init"])
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

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));
}

#[test]
fn vault_open_and_close_toggle_status() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    bin()
        .args(["--store", store.to_str().unwrap(), "init"])
        .assert()
        .success();

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "open"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault opened"));

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault open"));

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "close"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "status"])
        .assert()
        .success()
        .stdout(predicates::str::contains("vault closed"));
}

#[test]
fn mutating_vault_commands_fail_on_dirty_git_state() {
    let tmp = TempDir::new().expect("tempdir");
    let store = store_path(&tmp);

    bin()
        .args(["--store", store.to_str().unwrap(), "init"])
        .assert()
        .success();

    fs::write(store.join("dirty.txt"), b"dirty").expect("write dirty file");

    bin()
        .args(["--store", store.to_str().unwrap(), "vault", "open"])
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

    bin()
        .args(["--store", store.to_str().unwrap(), "init"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("store path is not empty"));
}
