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
        // Find an available port
        let port = Self::find_available_port();

        // Start Python server
        let process = Command::new("python3")
            .arg("tests/fake_server.py")
            .arg(port.to_string())
            .spawn()
            .expect("Failed to start Python fake server");

        // Wait for server to be ready
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
            // Try 50 times, 100ms each = 5 seconds max
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

    // Write test credentials
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

// ============================================================================
// List Creation Tests
// ============================================================================

#[test]
fn test_lists_create_with_valid_name_succeeds() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Create a list
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "My New List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("My New List"));

    // Verify list appears in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("My New List"));
}

#[test]
fn test_lists_create_with_unicode_name_succeeds() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Create a list with unicode name
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "Тест 🎯 émoji"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Тест 🎯 émoji"));

    // Verify list appears in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Тест 🎯 émoji"));
}

#[test]
fn test_lists_create_multiple_lists_all_appear() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Create first list
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "First List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("First List"));

    // Create second list
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "Second List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Second List"));

    // Verify both lists appear in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("First List"))
        .stdout(predicate::str::contains("Second List"));
}

// ============================================================================
// List Retrieval Tests
// ============================================================================

#[test]
fn test_lists_get_shows_created_lists() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Create a list
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "Test List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();

    // Get all lists
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Test List"));
}

#[test]
fn test_lists_get_empty_when_no_lists() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file);

    // Should succeed with empty output
    cmd.assert().success();
}

#[test]
fn test_lists_get_shows_all_lists() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Create multiple lists
    for name in &["List A", "List B", "List C"] {
        cargo_bin_cmd!("checkvist-cli")
            .args(["lists", "create", name])
            .env("CHECKVIST_BASE_URL", server.base_url())
            .env("CHECKVIST_AUTH_FILE", &env.auth_file)
            .env("CHECKVIST_TOKEN_FILE", &env.token_file)
            .assert()
            .success();
    }

    // Get all lists
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file);

    let output = cmd.assert().success();

    // All lists should appear
    output
        .stdout(predicate::str::contains("List A"))
        .stdout(predicate::str::contains("List B"))
        .stdout(predicate::str::contains("List C"));
}

// ============================================================================
// Integration Test: Create and Verify
// ============================================================================

#[test]
fn test_create_then_get_shows_new_list() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    // Initially no lists - verify with list command
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success();
    // Note: We can't verify output is empty because we don't know the format

    // Create a list
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "Integration Test List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Integration Test List"));

    // Verify list appears in listing
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Integration Test List"));
}
