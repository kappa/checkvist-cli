mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{StubResponse, StubServer};
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
    let token_path = temp.path().join("token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "POST",
            "/auth/login.json?version=2",
            200,
            serde_json::json!({"token": "NEWTOKEN"}),
            ("content-type", "application/x-www-form-urlencoded"),
        )
        .with_body_check("username=user%40example.com&remote_key=REMOTEKEY"),
        StubResponse::json_with_header(
            "GET",
            "/auth/curr_user.json",
            200,
            serde_json::json!({"user": {"id": 1, "email": "user@example.com"}}),
            ("x-client-token", "NEWTOKEN"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["auth", "login"])
        .env("CHECKVIST_AUTH_FILE", &auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path)
        .env("CHECKVIST_BASE_URL", server.base_url())
        .write_stdin("user@example.com\nREMOTEKEY\n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            format!("Open and sign in: {}/auth/login", server.base_url()),
        ))
        .stdout(predicate::str::contains(
            format!("Copy your Remote API key from: {}/auth/profile", server.base_url()),
        ))
        .stdout(predicate::str::contains(auth_path.to_string_lossy()))
        .stdout(predicate::str::contains(
            "Verifying with `checkvist auth status`",
        ))
        .stdout(predicate::str::contains("ok\tuser@example.com"));

    let written = fs::read_to_string(&auth_path).expect("read auth file");
    assert_eq!(
        written,
        "[default]\nusername = user@example.com\nremote_key = REMOTEKEY\n"
    );
}
