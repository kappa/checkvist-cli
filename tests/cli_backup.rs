mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{StubResponse, StubServer};
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
fn backup_creates_opml_files_for_all_lists() {
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

    cmd.assert().success();

    // Check that OPML files were created with correct names
    let work_file = output_dir.join("1-Work.opml");
    let personal_file = output_dir.join("2-Personal.opml");
    let old_file = output_dir.join("3-Old.opml");

    assert!(work_file.exists(), "Work OPML file not created");
    assert!(personal_file.exists(), "Personal OPML file not created");
    assert!(old_file.exists(), "Old OPML file not created");

    // Check file contents
    assert_eq!(
        fs::read_to_string(&work_file).expect("read work"),
        "<opml>Work</opml>"
    );
    assert_eq!(
        fs::read_to_string(&personal_file).expect("read personal"),
        "<opml>Personal</opml>"
    );
    assert_eq!(
        fs::read_to_string(&old_file).expect("read old"),
        "<opml>Old</opml>"
    );
}

#[test]
fn backup_sanitizes_filenames() {
    let temp = tempdir().expect("tempdir");
    let auth_path = temp.path().join("auth.ini");
    let token_path = temp.path().join("token");
    write_auth_ini(&auth_path);
    fs::write(&token_path, "TOK").expect("write token");

    let mut opml = StubResponse::raw(
        "GET",
        "/checklists/10.opml?export_status=true&export_notes=true&export_details=true&export_color=true",
        200,
        "<opml>test</opml>",
    );
    opml.required_header = Some(("x-client-token", "TOK"));

    let server = StubServer::new(vec![
        StubResponse::json_with_header(
            "GET",
            "/checklists.json",
            200,
            serde_json::json!([
                {"id": 10, "name": "Test/Name:With*Bad?Chars"}
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
    ])
    .env("CHECKVIST_BASE_URL", server.base_url())
    .env("CHECKVIST_AUTH_FILE", auth_path)
    .env("CHECKVIST_TOKEN_FILE", &token_path)
    .current_dir(temp.path());

    cmd.assert().success();

    // Check that filename was sanitized (special chars replaced with _)
    let sanitized_file = output_dir.join("10-Test_Name_With_Bad_Chars.opml");
    assert!(sanitized_file.exists(), "sanitized file not created");
    assert_eq!(
        fs::read_to_string(&sanitized_file).expect("read"),
        "<opml>test</opml>"
    );
}
