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
fn lists_create_posts_name_and_prints_line() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "POST",
            "/checklists.json",
            201,
            serde_json::json!({"id": 10, "name": "New List"}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("name=New+List"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "create", "New List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("10\tNew List"));
}

#[test]
fn lists_delete_prevents_non_empty_lists() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists/5/tasks.json",
        200,
        serde_json::json!([{"id": 1, "content": "Existing"}]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "delete", "5"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cannot delete non-empty list"));
}

#[test]
fn lists_update_archives_and_makes_private() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "PUT",
            "/checklists/7.json",
            200,
            serde_json::json!({"id": 7, "name": "Updated", "archived": true, "public": false}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("archived=true")
        .with_body_check("public=false"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "update", "7", "--archive", "--private"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("7\tUpdated"));
}

#[test]
fn lists_show_tasks_calls_task_get() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists/9/tasks.json",
        200,
        serde_json::json!([
            {"id": 1, "content": "Root"},
            {"id": 2, "content": "Child", "parent_id": 1}
        ]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "show", "9", "--tasks"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("1\tRoot"))
        .stdout(predicate::str::contains("  2\tChild"));
}
