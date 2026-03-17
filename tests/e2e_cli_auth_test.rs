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

// ============================================================================
// Auth Status Tests
// ============================================================================

#[test]
fn test_auth_status_shows_email() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    cargo_bin_cmd!("checkvist-cli")
        .args(["auth", "status"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("ok\ttest@example.com"));
}

#[test]
fn test_auth_status_json_output() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    cargo_bin_cmd!("checkvist-cli")
        .args(["auth", "status", "--format", "json"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"user\""))
        .stdout(predicate::str::contains("test@example.com"));
}

#[test]
fn test_auth_status_missing_credentials_fails() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let auth_file = temp.path().join("auth.ini");
    let token_file = temp.path().join("token");

    // Write incomplete credentials (missing remote_key)
    fs::write(&auth_file, "[default]\nusername = test@example.com\n")
        .expect("write incomplete auth");

    cargo_bin_cmd!("checkvist-cli")
        .args(["auth", "status"])
        .env("CHECKVIST_BASE_URL", "http://localhost:9")
        .env("CHECKVIST_AUTH_FILE", &auth_file)
        .env("CHECKVIST_TOKEN_FILE", &token_file)
        .assert()
        .failure()
        .code(3);
}

#[test]
fn test_auth_status_no_token_file_triggers_login() {
    let server = PythonFakeServer::new();
    let temp = tempfile::tempdir().expect("create tempdir");
    let auth_file = temp.path().join("auth.ini");
    let token_file = temp.path().join("token");

    // Write full credentials but no token file
    fs::write(
        &auth_file,
        "[default]\nusername = test@example.com\nremote_key = TEST_KEY\n",
    )
    .expect("write auth file");

    // Don't create token file — CLI should auto-login

    cargo_bin_cmd!("checkvist-cli")
        .args(["auth", "status"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &auth_file)
        .env("CHECKVIST_TOKEN_FILE", &token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("ok\ttest@example.com"));

    // Token file should now exist (written after auto-login)
    assert!(
        token_file.exists(),
        "Token file should be created after auto-login"
    );
}

// ============================================================================
// Auth with Token Refresh
// ============================================================================

#[test]
fn test_auth_operations_work_after_auto_login() {
    // Tests that the full auth flow works:
    // 1. No token file exists
    // 2. CLI auto-logs in using credentials
    // 3. Token is saved
    // 4. Subsequent operations use saved token
    let server = PythonFakeServer::new();
    let temp = tempfile::tempdir().expect("create tempdir");
    let auth_file = temp.path().join("auth.ini");
    let token_file = temp.path().join("token");

    fs::write(
        &auth_file,
        "[default]\nusername = test@example.com\nremote_key = TEST_KEY\n",
    )
    .expect("write auth file");

    // First call: no token → auto-login
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &auth_file)
        .env("CHECKVIST_TOKEN_FILE", &token_file)
        .assert()
        .success();

    // Token should be saved now
    assert!(token_file.exists(), "Token file should be created");

    // Second call: uses saved token
    cargo_bin_cmd!("checkvist-cli")
        .args(["lists", "create", "After Login List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &auth_file)
        .env("CHECKVIST_TOKEN_FILE", &token_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("After Login List"));
}
