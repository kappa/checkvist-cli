mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use bzip2::read::BzDecoder;
use common::{StubResponse, StubServer};
use predicates::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use tar::Archive;
use tempfile::tempdir;

fn write_auth_ini(path: &std::path::Path) {
    fs::write(
        path,
        "[default]\nusername = user@example.com\nremote_key = REMOTE\n",
    )
    .expect("write auth ini");
}

fn collect_archive_entries(path: &std::path::Path) -> HashMap<String, String> {
    let file = fs::File::open(path).expect("open archive");
    let decoder = BzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut files = HashMap::new();

    for entry in archive.entries().expect("archive entries") {
        let mut entry = entry.expect("entry");
        let path = entry.path().expect("path").to_string_lossy().into_owned();
        let mut contents = String::new();
        entry
            .read_to_string(&mut contents)
            .expect("read entry contents");
        files.insert(path, contents);
    }

    files
}

#[test]
fn backup_creates_archive_with_lists_and_opml() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let mut opml_one = StubResponse::raw(
        "GET",
        "/checklists/1.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
        200,
        "<opml>Work</opml>",
    );
    opml_one.required_header = Some(("x-client-token", "TOK"));
    let mut opml_two = StubResponse::raw(
        "GET",
        "/checklists/2.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
        200,
        "<opml>Personal</opml>",
    );
    opml_two.required_header = Some(("x-client-token", "TOK"));
    let mut opml_archived = StubResponse::raw(
        "GET",
        "/checklists/3.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
        200,
        "<opml>Old</opml>",
    );
    opml_archived.required_header = Some(("x-client-token", "TOK"));

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/checklists.json",
            200,
            serde_json::json!([
                {"id": 1, "name": "Work"},
                {"id": 2, "name": "Personal"}
            ]),
            ("x-client-token", "TOK"),
        ),
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?archived=true",
            200,
            serde_json::json!([
                {"id": 3, "name": "Old"}
            ]),
            ("x-client-token", "TOK"),
        ),
        opml_one,
        opml_two,
        opml_archived,
    ]);

    let output_dir = temp.path().join("output");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "backup",
        "--output",
        output_dir.to_str().expect("output dir"),
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path)
    .current_dir(temp.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Fetching Work"))
        .stdout(predicate::str::contains("Fetching Personal"))
        .stdout(predicate::str::contains("Fetching Old"));

    let archive_path = output_dir.join("checkvist.tar.bz2");
    assert!(archive_path.exists(), "archive not created");

    let files = collect_archive_entries(&archive_path);
    assert_eq!(
        files.get("content.json"),
        Some(&"[{\"id\":1,\"name\":\"Work\"},{\"id\":2,\"name\":\"Personal\"}]".to_string())
    );
    assert_eq!(
        files.get("content_archived.json"),
        Some(&"[{\"id\":3,\"name\":\"Old\"}]".to_string())
    );
    assert_eq!(
        files.get("Work.opml"),
        Some(&"<opml>Work</opml>".to_string())
    );
    assert_eq!(
        files.get("Personal.opml"),
        Some(&"<opml>Personal</opml>".to_string())
    );
    assert_eq!(files.get("Old.opml"), Some(&"<opml>Old</opml>".to_string()));
}

#[test]
fn backup_uses_date_suffix_when_requested() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let mut opml = StubResponse::raw(
        "GET",
        "/checklists/10.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
        200,
        "<opml>Only</opml>",
    );
    opml.required_header = Some(("x-client-token", "TOK"));

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/checklists.json",
            200,
            serde_json::json!([
                {"id": 10, "name": "Only"}
            ]),
            ("x-client-token", "TOK"),
        ),
        StubResponse::json_with_header(
            "GET",
            "/checklists.json?archived=true",
            200,
            serde_json::json!([]),
            ("x-client-token", "TOK"),
        ),
        opml,
    ]);

    let output_dir = temp.path().join("output");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args([
        "backup",
        "--output",
        output_dir.to_str().expect("output dir"),
        "--date",
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path)
    .current_dir(temp.path());

    cmd.assert().success();

    let today = chrono::Local::now().format("%d-%m-%Y").to_string();
    let archive_path = output_dir.join(format!("checkvist-{today}.tar.bz2"));
    assert!(archive_path.exists(), "dated archive missing");

    let files = collect_archive_entries(&archive_path);
    assert!(files.contains_key("content.json"));
    assert!(files.contains_key("content_archived.json"));
    assert_eq!(
        files.get("Only.opml"),
        Some(&"<opml>Only</opml>".to_string())
    );
}
