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
fn tasks_get_prints_hierarchy() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists/2/tasks.json",
        200,
        serde_json::json!([
            {"id": 10, "content": "Parent"},
            {"id": 11, "content": "Child", "parent_id": 10}
        ]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["tasks", "get", "--list", "2"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("10\tParent"))
        .stdout(predicate::str::contains("  11\tChild"));
}

#[test]
fn tasks_create_posts_content_and_parent() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "POST",
            "/checklists/3/tasks.json",
            201,
            serde_json::json!({"id": 20, "content": "New Task"}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("content=New+Task")
        .with_body_check("parent_id=19"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "tasks",
        "create",
        "--list",
        "3",
        "--content",
        "New Task",
        "--parent-id",
        "19",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("20\tNew Task"));
}

#[test]
fn tasks_update_sends_status_and_content() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "PUT",
            "/checklists/4/tasks/40.json",
            200,
            serde_json::json!({"id": 40, "content": "Updated", "status": "done"}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("status=done")
        .with_body_check("content=Updated"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "tasks",
        "update",
        "--list",
        "4",
        "--task-id",
        "40",
        "--status",
        "done",
        "--content",
        "Updated",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("40\tUpdated"));
}

#[test]
fn tasks_remove_deletes_task() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::raw(
        "DELETE",
        "/checklists/6/tasks/60.json",
        204,
        "",
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["tasks", "remove", "--list", "6", "--task-id", "60"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert().success();
}
