use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn running_without_command_prints_usage() {
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn auth_login_guides_and_writes_auth_file() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["auth", "login"])
        .env("CHECKVIST_AUTH_FILE", &auth_path)
        .env("CHECKVIST_BASE_URL", "https://example.com")
        .write_stdin("user@example.com\nREMOTEKEY\n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Open and sign in: https://example.com/auth/login",
        ))
        .stdout(predicate::str::contains(
            "Copy your Remote API key from: https://example.com/auth/profile",
        ))
        .stdout(predicate::str::contains(auth_path.to_string_lossy()))
        .stdout(predicate::str::contains("Run `checkvist auth status`"));

    let written = fs::read_to_string(&auth_path).expect("read auth file");
    assert_eq!(
        written,
        "[default]\nusername = user@example.com\nremote_key = REMOTEKEY\n"
    );
}
