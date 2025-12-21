mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{StubResponse, StubServer};
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn write_auth_ini(path: &std::path::Path) {
    fs::write(
        path,
        "[default]\nusername = user@example.com\nremote_key = REMOTE\n",
    )
    .expect("write auth ini");
}

#[test]
fn auth_status_ok_text_output() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLDTOKEN").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/auth/curr_user.json",
        200,
        serde_json::json!({"user": {"id": 1, "email": "user@example.com"}}),
        ("x-client-token", "OLDTOKEN"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.arg("auth")
        .arg("status")
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ok\tuser@example.com"));
}

#[test]
fn auth_status_retries_after_refresh() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLDTOKEN").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/auth/curr_user.json",
            403,
            serde_json::json!({"error": "expired"}),
            ("x-client-token", "OLDTOKEN"),
        ),
        StubResponse::json_with_header(
            "POST",
            "/auth/refresh_token.json?version=2",
            200,
            serde_json::json!({"token": "NEWTOKEN"}),
            ("x-client-token", "OLDTOKEN"),
        ),
        StubResponse::json_with_header(
            "GET",
            "/auth/curr_user.json",
            200,
            serde_json::json!({"user": {"id": 1, "email": "user@example.com"}}),
            ("x-client-token", "NEWTOKEN"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.arg("auth")
        .arg("status")
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ok\tuser@example.com"));

    let written = fs::read_to_string(&token_path).expect("read token file");
    assert_eq!(written, "NEWTOKEN");
}

#[test]
fn auth_status_relogin_after_refresh_failure() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLDTOKEN").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/auth/curr_user.json",
            401,
            serde_json::json!({"error": "expired"}),
            ("x-client-token", "OLDTOKEN"),
        ),
        StubResponse::json_with_header(
            "POST",
            "/auth/refresh_token.json?version=2",
            401,
            serde_json::json!({"error": "invalid"}),
            ("x-client-token", "OLDTOKEN"),
        ),
        StubResponse::json_with_header(
            "POST",
            "/auth/login.json?version=2",
            200,
            serde_json::json!({"token": "NEWTOKEN"}),
            ("content-type", "application/x-www-form-urlencoded"),
        )
        .with_body_check("username=user%40example.com&remote_key=REMOTE"),
        StubResponse::json_with_header(
            "GET",
            "/auth/curr_user.json",
            200,
            serde_json::json!({"user": {"id": 1, "email": "user@example.com"}}),
            ("x-client-token", "NEWTOKEN"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.arg("auth")
        .arg("status")
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ok\tuser@example.com"));

    let written = fs::read_to_string(&token_path).expect("read token file");
    assert_eq!(written, "NEWTOKEN");
}

#[test]
fn auth_status_json_output() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLDTOKEN").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/auth/curr_user.json",
        200,
        serde_json::json!({"user": {"id": 1, "email": "user@example.com"}}),
        ("x-client-token", "OLDTOKEN"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["auth", "status", "--format", "json"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"user\"").and(predicate::str::contains("email")));
}

#[test]
fn auth_status_missing_credentials_exits_with_code_three() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    fs::write(&auth_path, "[default]\nusername = user@example.com\n").expect("write auth ini");
    let token_path = temp.path().join("token");

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.arg("auth")
        .arg("status")
        .env("CHECKVIST_BASE_URL", "http://localhost:9")
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", token_path);

    cmd.assert().failure().code(3);
}
