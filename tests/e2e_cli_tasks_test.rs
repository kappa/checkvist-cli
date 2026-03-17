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

/// Helper: create a checklist via CLI and return its id (first field in output)
fn create_list(server: &PythonFakeServer, env: &TestEnv, name: &str) -> String {
    let output = cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", name])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run lists create");

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split('\t').next().unwrap_or("").trim().to_string()
}

// ============================================================================
// Task Creation Tests
// ============================================================================

#[test]
fn test_task_create_basic() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Task Test List");

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Buy groceries",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Buy groceries"));
}

#[test]
fn test_task_create_appears_in_listing() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Task Test List");

    // Create a task
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Write tests",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Verify it appears in task listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Write tests"));
}

#[test]
fn test_task_create_with_parent_id() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Subtask Test");

    // Create parent task and capture its id
    let parent_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Parent task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run task create");

    let parent_stdout = String::from_utf8_lossy(&parent_output.stdout);
    let parent_id = parent_stdout.split('\t').next().unwrap_or("").trim();

    // Create child task
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Child task",
            "--parent-id", parent_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Child task"));

    // Verify hierarchy in task listing (child should be indented)
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Parent task"))
        .stdout(predicate::str::contains("Child task"));
}

#[test]
fn test_task_create_with_tags_in_content() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Tags Test");

    // Create task with #hashtag in content — fake server auto-parses on create
    // The tag should be extracted from content
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Buy groceries #shopping",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Buy groceries"));
}

#[test]
fn test_task_create_with_unicode_content() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Unicode Test");

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Купить молоко 🥛",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Купить молоко 🥛"));
}

// ============================================================================
// Task Update Tests
// ============================================================================

#[test]
fn test_task_update_content() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Update Test");

    // Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Original content",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run task create");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Update content
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", task_id,
            "--content", "Updated content",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated content"));

    // Verify in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated content"));
}

#[test]
fn test_task_update_status_to_done() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Status Test");

    // Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Complete me",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run task create");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Mark as done
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", task_id,
            "--status", "done",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();
}

#[test]
fn test_task_update_with_parse_flag() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Parse Test");

    // Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Plain task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run task create");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Update with --parse flag and #tag in content
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", task_id,
            "--content", "Tagged task #important",
            "--parse",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Tagged task"));
}

#[test]
fn test_task_update_no_changes_fails() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "No Change Test");

    // Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Some task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("run task create");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Update with no changes should fail
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .failure();
}

// ============================================================================
// Task Listing Tests
// ============================================================================

#[test]
fn test_task_get_empty_list() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Empty List");

    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();
}

#[test]
fn test_task_get_multiple_tasks() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Multi Task");

    for content in &["First task", "Second task", "Third task"] {
        cargo_bin_cmd!("checkvist-cli")
            .args([
                "tasks", "create",
                "--list", &list_id,
                "--content", content,
            ])
            .env("CHECKVIST_BASE_URL", server.base_url())
            .env("CHECKVIST_AUTH_FILE", &env.auth_file)
            .env("CHECKVIST_TOKEN_FILE", &env.token_file)
            .assert()
            .success();
    }

    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("First task"))
        .stdout(predicate::str::contains("Second task"))
        .stdout(predicate::str::contains("Third task"));
}

#[test]
fn test_task_get_shows_hierarchy() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Hierarchy Test");

    // Create parent
    let parent_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Parent",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create parent");

    let parent_stdout = String::from_utf8_lossy(&parent_output.stdout);
    let parent_id = parent_stdout.split('\t').next().unwrap_or("").trim();

    // Create child
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Child",
            "--parent-id", parent_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Get tasks — child should be indented (2 spaces)
    let output = cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("get tasks");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    // First line: parent (no indent)
    assert!(
        lines[0].contains("Parent"),
        "First line should contain 'Parent', got: {}",
        lines[0]
    );
    assert!(
        !lines[0].starts_with("  "),
        "Parent should not be indented, got: {}",
        lines[0]
    );

    // Second line: child (indented)
    assert!(
        lines[1].contains("Child"),
        "Second line should contain 'Child', got: {}",
        lines[1]
    );
    assert!(
        lines[1].starts_with("  "),
        "Child should be indented with 2 spaces, got: {:?}",
        lines[1]
    );
}

#[test]
fn test_task_get_shows_priority() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Priority Test");

    // Create a task via the fake server directly to set priority
    // (CLI doesn't have a --priority flag for create, but we can
    // test that the display works by using curl to set it up)
    // Instead, let's create via CLI and then use curl to set priority
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Urgent task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create task");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Use curl to set priority on the task (since CLI doesn't have --priority)
    let url = format!(
        "{}/checklists/{}/tasks/{}.json",
        server.base_url(),
        list_id,
        task_id
    );
    let _ = ureq::put(&url)
        .set("X-Client-Token", "TEST_TOKEN")
        .send_form(&[("task[priority]", "1")]);

    // Now get tasks — should show "! " prefix for priority 1
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("! Urgent task"));
}

// ============================================================================
// Task Remove Tests
// ============================================================================

#[test]
fn test_task_remove_succeeds() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Remove Test");

    // Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Delete me",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create task");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim();

    // Remove task
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "remove",
            "--list", &list_id,
            "--task-id", task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Verify it's gone
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Delete me").not());
}

// ============================================================================
// Task JSON Output Tests
// ============================================================================

#[test]
fn test_task_get_json_output() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "JSON Test");

    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "JSON task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    cargo_bin_cmd!("checkvist-cli")
        .args(["--format", "json", "tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tasks\""))
        .stdout(predicate::str::contains("JSON task"));
}

// ============================================================================
// Integration: Full Task Lifecycle
// ============================================================================

#[test]
fn test_task_full_lifecycle() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();
    let list_id = create_list(&server, &env, "Lifecycle Test");

    // 1. Create task
    let create_output = cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "create",
            "--list", &list_id,
            "--content", "Lifecycle task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .output()
        .expect("create task");

    let create_stdout = String::from_utf8_lossy(&create_output.stdout);
    let task_id = create_stdout.split('\t').next().unwrap_or("").trim().to_string();
    assert!(!task_id.is_empty(), "Task ID should not be empty");

    // 2. Verify in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Lifecycle task"));

    // 3. Update content
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", &task_id,
            "--content", "Updated lifecycle task",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated lifecycle task"));

    // 4. Mark as done
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "update",
            "--list", &list_id,
            "--task-id", &task_id,
            "--status", "done",
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // 5. Remove task
    cargo_bin_cmd!("checkvist-cli")
        .args([
            "tasks", "remove",
            "--list", &list_id,
            "--task-id", &task_id,
        ])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // 6. Verify it's gone
    cargo_bin_cmd!("checkvist-cli")
        .args(["tasks", "get", "--list", &list_id])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Lifecycle task").not());
}
