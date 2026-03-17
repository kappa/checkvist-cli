/// End-to-end tests for the `backup` command using the Python fake server.
use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers (same pattern as e2e_cli_lists_test.rs)
// ---------------------------------------------------------------------------

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
            if ureq::get(&format!("{}/checklists.json", base_url))
                .set("X-Client-Token", "TEST")
                .call()
                .is_ok()
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

fn cli_cmd(server: &PythonFakeServer, env: &TestEnv) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &env.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &env.token_file);
    cmd
}

fn create_list(server: &PythonFakeServer, env: &TestEnv, name: &str) -> String {
    let output = cli_cmd(server, env)
        .args(["lists", "create", name])
        .output()
        .expect("cli lists create");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split('\t').next().unwrap().trim().to_string()
}

fn create_task(server: &PythonFakeServer, env: &TestEnv, list_id: &str, content: &str) -> String {
    let output = cli_cmd(server, env)
        .args(["tasks", "create", "--list", list_id, "--content", content])
        .output()
        .expect("cli tasks create");
    assert!(output.status.success(), "tasks create failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split('\t').next().unwrap().trim().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn backup_exports_active_lists_as_opml() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    let id1 = create_list(&server, &env, "Work");
    let id2 = create_list(&server, &env, "Personal");
    create_task(&server, &env, &id1, "Write tests");

    let output_dir = env._temp.path().join("backup_output");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let output = cli_cmd(&server, &env)
        .args(["backup", "--output", output_dir.to_str().unwrap()])
        .output()
        .expect("backup command");

    assert!(output.status.success(), "backup failed: {}", String::from_utf8_lossy(&output.stderr));

    let work_file = output_dir.join(format!("{}-Work.opml", id1));
    let personal_file = output_dir.join(format!("{}-Personal.opml", id2));

    assert!(work_file.exists(), "Work OPML file not created: {:?}",
        fs::read_dir(&output_dir).unwrap().map(|e| e.unwrap().file_name()).collect::<Vec<_>>());
    assert!(personal_file.exists(), "Personal OPML file not created");

    let work_content = fs::read_to_string(&work_file).expect("read work opml");
    assert!(work_content.contains("<opml"), "OPML should contain <opml tag");
    assert!(work_content.contains("Work"), "OPML title should match list name");
    assert!(work_content.contains("Write tests"), "OPML should contain task content");

    let personal_content = fs::read_to_string(&personal_file).expect("read personal opml");
    assert!(personal_content.contains("<opml"), "Personal OPML should contain <opml tag");
    assert!(personal_content.contains("Personal"), "Personal OPML should contain list name");
}

#[test]
fn backup_sanitizes_filenames() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    let id = create_list(&server, &env, "Test/Name:With*Bad?Chars");

    let output_dir = env._temp.path().join("backup_output");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let output = cli_cmd(&server, &env)
        .args(["backup", "--output", output_dir.to_str().unwrap()])
        .output()
        .expect("backup command");

    assert!(output.status.success(), "backup failed: {}", String::from_utf8_lossy(&output.stderr));

    let sanitized_file = output_dir.join(format!("{}-Test_Name_With_Bad_Chars.opml", id));
    assert!(sanitized_file.exists(), "sanitized file not created: {:?}",
        fs::read_dir(&output_dir).unwrap().map(|e| e.unwrap().file_name()).collect::<Vec<_>>());

    let content = fs::read_to_string(&sanitized_file).expect("read");
    assert!(content.contains("<opml"), "OPML should contain <opml tag");
}

#[test]
fn backup_creates_output_directory() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    create_list(&server, &env, "My List");

    let output_dir = env._temp.path().join("does_not_exist_yet");
    assert!(!output_dir.exists());

    let output = cli_cmd(&server, &env)
        .args(["backup", "--output", output_dir.to_str().unwrap()])
        .output()
        .expect("backup command");

    assert!(output.status.success(), "backup failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(output_dir.exists(), "output directory should have been created");

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(files.len(), 1, "should have one OPML file");
    assert!(files[0].ends_with(".opml"), "file should be .opml");
}

#[test]
fn backup_handles_empty_server() {
    let server = PythonFakeServer::new();
    let env = setup_test_env();

    let output_dir = env._temp.path().join("backup_output");
    fs::create_dir_all(&output_dir).expect("create output dir");

    let output = cli_cmd(&server, &env)
        .args(["backup", "--output", output_dir.to_str().unwrap()])
        .output()
        .expect("backup command");

    assert!(output.status.success(), "backup failed: {}", String::from_utf8_lossy(&output.stderr));

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(files.is_empty(), "should have no OPML files for empty server");
}
