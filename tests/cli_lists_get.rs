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
fn lists_get_fetches_lists_with_token() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists.json?skip_stats=true",
        200,
        serde_json::json!([
            {"id": 1, "name": "List One"},
            {"id": 2, "name": "List Two"}
        ]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("1\tList One"))
        .stdout(predicate::str::contains("2\tList Two"));
}

#[test]
fn lists_get_login_when_token_missing() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);

    let server = StubServer::new(vec![
        StubResponse::json(
            "POST",
            "/auth/login.json?version=2",
            200,
            serde_json::json!({"token": "NEWTOK"}),
        )
        .with_body_check("username=user%40example.com&remote_key=REMOTE"),
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?skip_stats=true",
            200,
            serde_json::json!([
                {"id": 3, "name": "Work"}
            ]),
            ("x-client-token", "NEWTOK"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("3\tWork"));

    let written = fs::read_to_string(&token_path).expect("read token");
    assert_eq!(written, "NEWTOK");
}

#[test]
fn lists_get_refresh_on_403_then_retry() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLD").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?skip_stats=true",
            403,
            serde_json::json!({"error": "expired"}),
            ("x-client-token", "OLD"),
        ),
        StubResponse::json_with_header(
            "POST",
            "/auth/refresh_token.json?version=2",
            200,
            serde_json::json!({"token": "NEW"}),
            ("x-client-token", "OLD"),
        ),
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?skip_stats=true",
            200,
            serde_json::json!([
                {"id": 5, "name": "After Refresh"}
            ]),
            ("x-client-token", "NEW"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("5\tAfter Refresh"));

    let written = fs::read_to_string(&token_path).expect("read token");
    assert_eq!(written, "NEW");
}

#[test]
fn lists_get_relogin_when_refresh_fails() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "OLD").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?skip_stats=true",
            401,
            serde_json::json!({"error": "expired"}),
            ("x-client-token", "OLD"),
        ),
        StubResponse::json_with_header(
            "POST",
            "/auth/refresh_token.json?version=2",
            401,
            serde_json::json!({"error": "nope"}),
            ("x-client-token", "OLD"),
        ),
        StubResponse::json(
            "POST",
            "/auth/login.json?version=2",
            200,
            serde_json::json!({"token": "NEW"}),
        )
        .with_body_check("username=user%40example.com&remote_key=REMOTE"),
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?skip_stats=true",
            200,
            serde_json::json!([
                {"id": 8, "name": "After Relogin"}
            ]),
            ("x-client-token", "NEW"),
        ),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("8\tAfter Relogin"));

    let written = fs::read_to_string(&token_path).expect("read token");
    assert_eq!(written, "NEW");
}

#[test]
fn lists_get_json_output_format() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists.json?skip_stats=true",
        200,
        serde_json::json!([
            {"id": 9, "name": "Json"}
        ]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get", "--format", "json"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"lists\"").and(predicate::str::contains("Json")));
}

#[test]
fn lists_get_missing_credentials_exits_code_three() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    fs::write(&auth_path, "[default]\nusername = only_user\n").expect("write auth ini");
    let token_path = temp.path().join("token");

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get"])
        .env("CHECKVIST_BASE_URL", "http://localhost:9")
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert().failure().code(3);
}
