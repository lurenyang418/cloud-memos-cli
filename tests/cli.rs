use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_documents_profile_and_binary_name() {
    let mut command = Command::cargo_bin("cloud-memos").expect("binary");
    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("cloud-memos"))
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("--profile"));
}

#[test]
fn profile_help_lists_full_lifecycle() {
    let mut command = Command::cargo_bin("cloud-memos").expect("binary");
    command
        .args(["profile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("add"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("use"))
        .stdout(predicate::str::contains("remove"));
}

#[test]
fn unpaired_environment_credentials_fail_before_keyring_access() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let mut command = Command::cargo_bin("cloud-memos").expect("binary");
    command
        .env("CLOUD_MEMOS_CONFIG", temporary.path().join("config.toml"))
        .env("CLOUD_MEMOS_URL", "https://memos.example.com")
        .env_remove("CLOUD_MEMOS_TOKEN")
        .assert()
        .failure()
        .stderr(predicate::str::contains("必须成对提供"));
}
