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
fn notes_list_prints_notes() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::json_with_header(
        "GET",
        "/checklists/1/tasks/9/notes.json",
        200,
        serde_json::json!([
            {"id": 5, "text": "First note"},
            {"id": 6, "text": "Second note"},
        ]),
        ("x-client-token", "TOK"),
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["note", "--list", "1", "--task", "9"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", auth_path)
        .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("5\tFirst note"))
        .stdout(predicate::str::contains("6\tSecond note"));
}

#[test]
fn notes_create_posts_text() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "POST",
            "/checklists/2/tasks/3/notes.json",
            201,
            serde_json::json!({"id": 7, "text": "New note"}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("text=New+note"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "notes", "create", "--list", "2", "--task", "3", "--text", "New note",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("7\tNew note"));
}

#[test]
fn notes_update_sends_text() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "PUT",
            "/checklists/4/tasks/8/notes/11.json",
            200,
            serde_json::json!({"id": 11, "text": "Updated note"}),
            ("x-client-token", "TOK"),
        )
        .with_body_check("text=Updated+note"),
    ]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "notes",
        "update",
        "--list",
        "4",
        "--task",
        "8",
        "--note-id",
        "11",
        "--text",
        "Updated note",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("11\tUpdated note"));
}

#[test]
fn notes_remove_deletes_note() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let server = StubServer::new(vec![StubResponse::raw(
        "DELETE",
        "/checklists/5/tasks/10/notes/12.json",
        204,
        "",
    )]);

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "notes",
        "remove",
        "--list",
        "5",
        "--task",
        "10",
        "--note-id",
        "12",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path);

    cmd.assert().success();
}
