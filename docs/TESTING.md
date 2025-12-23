# Testing Guide for checkvist-cli

**REQUIRED**: All changes to this project MUST follow Test-Driven Development (TDD).

## Table of Contents

1. [Testing Philosophy](#testing-philosophy)
2. [Test Suite Architecture](#test-suite-architecture)
3. [Running Tests](#running-tests)
4. [TDD Workflow (REQUIRED)](#tdd-workflow-required)
5. [Writing New Tests](#writing-new-tests)
6. [Building the Fake Server](#building-the-fake-server)
7. [Verification Against Real API](#verification-against-real-api)

---

## Testing Philosophy

### Why TDD is Required

This project previously had tests that gave **false confidence** - they passed while the code was fundamentally broken. Tests were checking implementation details (HTTP request format) rather than actual behavior.

**To prevent this from happening again: TDD is now MANDATORY.**

### Core Principles

1. **One Test Suite, Two Targets**
   - Same tests run against both fake server and real Checkvist API
   - Fake server for fast TDD workflow
   - Real API for occasional verification

2. **Behavioral Testing**
   - Tests verify CLI output and behavior
   - Tests DO NOT assert on HTTP request details
   - Fake server simulates real API behavior

3. **No False Positives**
   - If tests pass, the code works
   - If code is broken, tests fail
   - Period.

---

## Test Suite Architecture

```
tests/
├── fake_server/           # NEW: Behavioral fake Checkvist API
│   ├── mod.rs            # Server infrastructure
│   ├── lists.rs          # List endpoints implementation
│   ├── tasks.rs          # Task endpoints implementation
│   └── ...
├── cli_lists_test.rs     # Tests for list commands
├── cli_tasks_test.rs     # Tests for task commands
└── ...
```

### Fake Server vs Real API

| Feature | Fake Server | Real API |
|---------|-------------|----------|
| **Speed** | Fast (milliseconds) | Slow (network latency) |
| **When to use** | Always (TDD, CI) | Occasionally (verification) |
| **Data persistence** | None (in-memory only) | Full database |
| **Network required** | No | Yes |
| **Credentials required** | No | Yes |

---

## Running Tests

### Against Fake Server (Default)

```bash
# Run all tests against fake server
cargo test

# Run specific test file
cargo test --test cli_lists_test

# Run specific test function
cargo test test_lists_create_with_valid_name
```

**Default behavior**: Tests automatically spin up fake server.

### Against Real API (Verification)

```bash
# Run tests against real checkvist.com
CHECKVIST_TEST_MODE=real cargo test

# Or use the convenience script
./scripts/test-against-real-api.sh
```

**Requirements for real API testing:**
- Valid credentials in `~/.checkvist/auth.ini`
- Network connection
- Will create/modify/delete real data (use test account!)

**⚠️ Warning**: Real API tests are DESTRUCTIVE. They create and delete lists/tasks. Use a test account, not your production account.

---

## TDD Workflow (REQUIRED)

**Every change must follow this workflow. No exceptions.**

### Step 1: Choose a Command

Example: Implementing `lists create`

### Step 2: Research API Behavior

1. Read API documentation: https://checkvist.com/auth/api
2. Test with curl to understand real behavior:
   ```bash
   # Test what happens with correct parameter
   curl -X POST "https://checkvist.com/checklists.json" \
     -H "X-Client-Token: YOUR_TOKEN" \
     --data-urlencode "checklist[name]=Test List"

   # Test what happens with wrong parameter
   curl -X POST "https://checkvist.com/checklists.json" \
     -H "X-Client-Token: YOUR_TOKEN" \
     --data-urlencode "name=Test List"

   # Test error conditions
   curl -X POST "https://checkvist.com/checklists.json" \
     -H "X-Client-Token: invalid"
   ```

3. Document observed behavior

### Step 3: Implement Fake Server Behavior

In `tests/fake_server/lists.rs`:

```rust
pub fn handle_create_checklist(request: &Request) -> Response {
    // Parse form data from request body
    let params = parse_form_data(&request.body);

    // Simulate real Checkvist API behavior
    let name = params.get("checklist[name]")
        .map(|s| s.as_str())
        .unwrap_or("Name this list");  // Default if parameter missing

    // Generate response (in-memory, no persistence)
    let id = generate_id();
    Response::json(201, json!({
        "id": id,
        "name": name,
        "created_at": now(),
        // ... other fields
    }))
}
```

**Key point**: Fake server BEHAVES like real API. No assertions on requests!

### Step 4: Verify Fake Server with Curl

```bash
# Start fake server (we'll build this helper)
cargo run --bin fake-server &
SERVER_PID=$!

# Test it behaves like real API
curl -X POST "http://localhost:8080/checklists.json" \
  --data-urlencode "checklist[name]=Test"
# Should return: {"id": ..., "name": "Test", ...}

curl -X POST "http://localhost:8080/checklists.json" \
  --data-urlencode "name=Wrong"
# Should return: {"id": ..., "name": "Name this list", ...}

kill $SERVER_PID
```

### Step 5: Write Failing Test

In `tests/cli_lists_test.rs`:

```rust
#[test]
fn test_lists_create_with_valid_name() {
    let server = TestServer::new();  // Starts fake server or uses real API
    let temp = setup_test_env(&server);

    // Run the CLI command
    let mut cmd = Command::cargo_bin("checkvist-cli").unwrap();
    cmd.args(["lists", "create", "My New List"])
        .env("CHECKVIST_BASE_URL", server.base_url())
        .env("CHECKVIST_AUTH_FILE", &temp.auth_file)
        .env("CHECKVIST_TOKEN_FILE", &temp.token_file);

    // Assert on CLI behavior only
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("My New List"));

    // Verify list was created (query the server)
    let lists = server.get_lists();
    assert_eq!(lists[0].name, "My New List");
}
```

**What to test**:
- ✅ CLI output contains expected values
- ✅ CLI exit codes are correct
- ✅ Server state changed as expected
- ❌ HTTP request format (that's implementation detail)

### Step 6: Run Test - Should Fail

```bash
cargo test test_lists_create_with_valid_name
# Should fail because code is not implemented yet
```

### Step 7: Implement Code to Pass Test

Fix `src/api.rs`:

```rust
pub fn create_checklist(&self, token: &str, name: &str) -> AppResult<Value> {
    let url = format!("{}/checklists.json", self.base_url);
    let response = self
        .agent
        .post(&url)
        .set("X-Client-Token", token)
        .send_form(&[("checklist[name]", name)])  // FIX: was "name"
        .map_err(map_network_error)?;
    // ...
}
```

### Step 8: Run Test - Should Pass

```bash
cargo test test_lists_create_with_valid_name
# Should pass now
```

### Step 9: Verify Against Real API (Occasionally)

```bash
CHECKVIST_TEST_MODE=real cargo test test_lists_create_with_valid_name
# Should also pass against real checkvist.com
```

### Step 10: Write More Tests

Cover edge cases:
- Empty names
- Very long names
- Unicode characters
- Error conditions (invalid auth, network errors)
- Repeat steps 5-8 for each case

---

## Writing New Tests

### Test File Structure

```rust
// tests/cli_lists_test.rs

mod fake_server;  // Import fake server

use assert_cmd::Command;
use predicates::prelude::*;
use fake_server::TestServer;

// Helper to setup test environment
fn setup_test_env(server: &TestServer) -> TestEnv {
    let temp = tempdir().unwrap();
    let auth_file = temp.path().join("auth.ini");
    let token_file = temp.path().join("token");

    // Write test credentials
    fs::write(&auth_file,
        "[default]\nusername = test@example.com\nremote_key = TEST_KEY\n"
    ).unwrap();
    fs::write(&token_file, "TEST_TOKEN").unwrap();

    TestEnv { temp, auth_file, token_file }
}

#[test]
fn test_lists_create_with_valid_name() {
    let server = TestServer::new();
    let env = setup_test_env(&server);

    // Test implementation...
}
```

### Test Naming Convention

```
test_<command>_<scenario>_<expected_result>
```

Examples:
- `test_lists_create_with_valid_name_succeeds`
- `test_lists_create_with_empty_name_returns_error`
- `test_tasks_create_with_invalid_list_id_fails`

### What Makes a Good Test

✅ **Good Test**:
```rust
#[test]
fn test_lists_create_with_unicode_name() {
    let server = TestServer::new();
    let env = setup_test_env(&server);

    let result = run_cli(&["lists", "create", "Тест 🎯"], &env);

    assert!(result.success);
    assert!(result.stdout.contains("Тест 🎯"));
}
```

❌ **Bad Test** (testing implementation):
```rust
#[test]
fn test_lists_create_sends_correct_http_format() {
    // DON'T DO THIS
    assert!(request_body.contains("checklist[name]="));  // ❌
}
```

---

## Building the Fake Server

### Design Principles

1. **Behavioral Simulation**: Fake server must behave like real API
2. **No Persistence**: In-memory only, reset between tests
3. **Error Simulation**: Must simulate error conditions (401, 404, 500)
4. **Simplicity**: Don't implement unused features

### Implementation Structure

```rust
// tests/fake_server/mod.rs

pub struct TestServer {
    base_url: String,
    handle: JoinHandle<()>,
    state: Arc<Mutex<ServerState>>,
}

struct ServerState {
    lists: HashMap<i64, Checklist>,
    tasks: HashMap<i64, Task>,
    next_id: i64,
}

impl TestServer {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(ServerState::default()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let state_clone = state.clone();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                handle_request(stream.unwrap(), &state_clone);
            }
        });

        TestServer {
            base_url: format!("http://{}", addr),
            handle,
            state,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // Test helpers
    pub fn get_lists(&self) -> Vec<Checklist> {
        self.state.lock().unwrap().lists.values().cloned().collect()
    }
}

fn handle_request(stream: TcpStream, state: &Arc<Mutex<ServerState>>) {
    let request = parse_request(stream);

    let response = match (&request.method[..], &request.path[..]) {
        ("POST", "/checklists.json") => {
            handle_create_checklist(&request, state)
        }
        ("GET", "/checklists.json") => {
            handle_list_checklists(&request, state)
        }
        // ... more routes
        _ => Response::new(404, "Not Found"),
    };

    send_response(stream, response);
}
```

### When to Add Fake Server Features

**Only implement what you need for tests**:

1. Implementing `lists create`? → Add POST /checklists.json
2. Implementing `lists get`? → Add GET /checklists.json
3. Need authentication? → Add token validation

**Don't**:
- Implement full database
- Add features you don't test
- Optimize for performance (it's a test fake)

---

## Verification Against Real API

### When to Verify

Run tests against real API when:
- ✅ Implementing new commands
- ✅ Before major releases
- ✅ Suspecting API changes
- ✅ Fake server behavior seems wrong

**Don't** run against real API:
- ❌ On every commit (use fake server)
- ❌ In CI/CD pipeline (too slow, hammers their servers)
- ❌ During TDD (defeats the purpose)

### How to Verify

```bash
# 1. Setup test account on checkvist.com
# 2. Configure credentials
# 3. Run verification

CHECKVIST_TEST_MODE=real cargo test

# Or specific tests
CHECKVIST_TEST_MODE=real cargo test test_lists_create
```

### Interpreting Results

**If tests pass against fake but fail against real**:
- Fake server behavior is wrong
- Update fake server to match real API
- This should be rare (API is stable)

**If tests fail against both**:
- Code is broken
- Fix the code

**If tests pass against both**:
- ✅ All good!

---

## Migration Plan

### Current Status

- ❌ Old test suite in `tests/` uses assertion-based stubs
- ❌ Tests give false confidence
- ✅ New approach documented here

### Migration Steps

1. **Don't touch old tests yet** - they'll be deleted later
2. **Build fake server** in `tests/fake_server/`
3. **Write new tests** in `tests/*_test.rs` (new files)
4. **For each command**:
   - Research API behavior
   - Implement in fake server
   - Write tests
   - Implement code
   - Verify against real API
5. **When all commands are covered**:
   - Delete old test files
   - Delete `tests/common/mod.rs` (old stub server)
   - Update CI configuration

---

## Enforcement

### Code Review Checklist

**Before merging any PR, verify**:

- [ ] New code has tests
- [ ] Tests were written BEFORE code (TDD)
- [ ] Tests pass against fake server
- [ ] Tests pass against real API (if significant change)
- [ ] Tests verify behavior, not implementation
- [ ] Fake server simulates real API behavior
- [ ] No assertions on HTTP request format

### CI Configuration

```yaml
# .github/workflows/test.yml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v2
    - name: Run tests against fake server
      run: cargo test
    # No real API testing in CI
```

---

## FAQ

**Q: Why not just test against real API all the time?**
A: Too slow, hammers small business servers, requires network, credentials, and creates real data.

**Q: What if fake server diverges from real API?**
A: Periodic verification catches this. API is stable (15 years), so drift is unlikely.

**Q: Can I skip TDD for small changes?**
A: No. Every change requires tests first. This is non-negotiable.

**Q: What about unit tests for individual functions?**
A: These are integration tests (test whole CLI). Unit tests are optional but encouraged for complex logic in `src/`.

**Q: How do I test error conditions?**
A: Fake server must simulate errors. Example:
```rust
if token != "VALID_TOKEN" {
    return Response::new(401, json!({"error": "Unauthorized"}));
}
```

**Q: What if I find a bug in production?**
A: Write a failing test that reproduces the bug, then fix it. Classic TDD.

---

## Summary

1. **TDD is required** - tests before code, always
2. **One test suite** - runs against both fake and real servers
3. **Fake server for TDD** - fast, reliable, no network
4. **Real API for verification** - occasional, manual
5. **Behavioral testing** - verify what code does, not how it does it
6. **No false positives** - if tests pass, code works

**Remember**: The previous test suite gave false confidence. This approach prevents that from ever happening again.

---

**Last updated**: 2025-12-22
