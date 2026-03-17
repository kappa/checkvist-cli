use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Python fake server instance
struct PythonFakeServer {
    process: Child,
    base_url: String,
}

impl PythonFakeServer {
    fn new() -> Self {
        let port = Self::find_available_port();

        let process = Command::new("python3")
            .arg("tests/fake_server.py")
            .arg(port.to_string())
            .spawn()
            .expect("Failed to start Python fake server");

        let base_url = format!("http://127.0.0.1:{}", port);
        Self::wait_for_server(&base_url);

        PythonFakeServer { process, base_url }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn find_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("Failed to bind to port 0")
            .local_addr()
            .expect("Failed to get local address")
            .port()
    }

    fn wait_for_server(base_url: &str) {
        for _ in 0..50 {
            thread::sleep(Duration::from_millis(100));
            if let Ok(_) = ureq::get(&format!("{}/checklists.json", base_url))
                .set("X-Client-Token", "TEST")
                .call()
            {
                return;
            }
        }
        panic!("Server failed to start within 5 seconds");
    }
}

impl Drop for PythonFakeServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Test environment with temporary auth files
struct TestEnv {
    _temp: TempDir,
    auth_file: std::path::PathBuf,
    token_file: std::path::PathBuf,
}

fn setup_test_env() -> TestEnv {
    let temp = tempfile::tempdir().expect("create tempdir");
    let auth_file = temp.path().join("auth.ini");
    let token_file = temp.path().join("token");

    fs::write(
        &auth_file,
        "[default]\nusername = test@example.com\nremote_key = TEST_KEY\n",
    )
    .expect("write auth file");

    fs::write(&token_file, "TEST_TOKEN").expect("write token file");

    TestEnv {
        _temp: temp,
        auth_file,
        token_file,
    }
}

/// Helper: create a checklist and a task, return (list_id, task_id)
fn create_list_and_task(
    server: &PythonFakeServer,
    env: &TestEnv,
) -> (String, String) {
    let list_output = cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "Notes Test List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create list");

    let list_stdout = String::from_utf8_lossy(&list_output.stdout);
    let list_id = list_stdout.split('\t').next().unwrap_or("").trim().to_string();

    let task_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Test task for notes",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create task");

    let task_stdout = String::from_utf8_lossy(&task_output.stdout);
    let task_id = task_stdout.split('\t').next().unwrap_or("").trim().to_string();

    (list_id, task_id)
}

// ============================================================================
// Note Creation Tests
// ============================================================================

#[test]
fn test_note_create_basic() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "This is a note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("This is a note"));
}

#[test]
fn test_note_create_appears_in_listing() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    // Create a note
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Important note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Verify it appears in listing
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Important note"));
}

#[test]
fn test_note_create_with_unicode() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Заметка с юникодом 📝",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Заметка с юникодом 📝"));
}

// ============================================================================
// Note Update Tests
// ============================================================================

#[test]
fn test_note_update_text() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    // Create note and capture its id
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Original note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create note");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let note_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Update the note
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "update",
            "--list", &list_id,
            "--task", &task_id,
            "--note-id", note_id,
            "--text", "Updated note text",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated note text"));

    // Verify in listing
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated note text"));
}

#[test]
fn test_note_update_no_changes_fails() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    // Create note
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Some note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create note");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let note_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Update with no --text should fail
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "update",
            "--list", &list_id,
            "--task", &task_id,
            "--note-id", note_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .failure();
}

// ============================================================================
// Note Remove Tests
// ============================================================================

#[test]
fn test_note_remove_succeeds() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    // Create note
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Delete me",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create note");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let note_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Remove note
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "remove",
            "--list", &list_id,
            "--task", &task_id,
            "--note-id", note_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Verify it's gone
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Delete me").not());
}

// ============================================================================
// Note Listing Tests
// ============================================================================

#[test]
fn test_note_list_empty() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();
}

#[test]
fn test_note_list_multiple() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    for text in &["First note", "Second note", "Third note"] {
        cargo_bin_cmd!("checkvist-cli")
            .args([
                "notes", "create",
                "--list", &list_id,
                "--task", &task_id,
                "--text", text,
            ])
            .env("CHECKVIST_BASE_URL", server.base_url())
            .env("CHECKVIST_AUTH_FILE", &env.auth_file)
            .env("CHECKVIST_TOKEN_FILE", &env.token_file)
            .assert()
            .success();
    }

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("First note"))
        .stdout(predicate::str::contains("Second note"))
        .stdout(predicate::str::contains("Third note"));
}

#[test]
fn test_note_list_uses_shorthand_syntax() {
    // `notes --list X --task Y` should behave like `notes list --list X --task Y`
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Shorthand test",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Using shorthand (no "list" subcommand)
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Shorthand test"));
}

// ============================================================================
// Note JSON Output Tests
// ============================================================================

#[test]
fn test_note_list_json_output() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "JSON note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "--format", "json",
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"notes\""))
        .stdout(predicate::str::contains("JSON note"));
}

// ============================================================================
// Note Full Lifecycle
// ============================================================================

#[test]
fn test_note_full_lifecycle() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let (list_id, task_id) = create_list_and_task(&server, &env);

    // 1. Create note
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "create",
            "--list", &list_id,
            "--task", &task_id,
            "--text", "Lifecycle note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create note");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let note_id = create_stdout.split('\t').next().unwrap_or("").trim().to_string();
    assert!(!note_id.is_empty(), "Note ID should not be empty");

    // 2. Verify in listing
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Lifecycle note"));

    // 3. Update note
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "update",
            "--list", &list_id,
            "--task", &task_id,
            "--note-id", &note_id,
            "--text", "Updated lifecycle note",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated lifecycle note"));

    // 4. Remove note
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "remove",
            "--list", &list_id,
            "--task", &task_id,
            "--note-id", &note_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // 5. Verify it's gone
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "notes", "list",
            "--list", &list_id,
            "--task", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Lifecycle note").not());
}
