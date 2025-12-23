# checkvist-cli TODOs

This file tracks bugs, missing features, and improvements for checkvist-cli.

## Critical Issues

### 🚨 Test suite provides false confidence - tests pass while code is broken
**Status:** 🔥 CRITICAL
**Severity:** BLOCKER
**Discovered:** 2025-12-22

**Description:**
The entire test suite uses mock/stub testing that asserts on HTTP request parameters instead of simulating actual API behavior. This means **tests pass even when the code is fundamentally broken**.

**The Problem:**
The `StubServer` in `tests/common/mod.rs` uses `.with_body_check()` to assert that the client sends specific request parameters. This is testing **implementation details**, not **behavior**.

**Example - List Creation Test:**
```rust
// tests/cli_lists_crud.rs:18-45
StubResponse::json_with_header(...)
    .with_body_check("name=New+List")  // ❌ Asserts on REQUEST
```

**Why This Is Broken:**
1. Test checks that client sends `name=New+List` ✅ (passes)
2. Server returns hardcoded `{"id": 10, "name": "New List"}` ✅ (passes)
3. **Real Checkvist API ignores `name` parameter** ❌ (never tested!)
4. **Real Checkvist API expects `checklist[name]`** ❌ (never tested!)
5. **Real code creates lists named "Name this list"** ❌ (test doesn't catch this!)

**Impact:**
- All existing tests are **nearly worthless** for catching real bugs
- Tests give false confidence that the code works
- Bugs only discovered through manual testing
- Similar bugs likely exist in tasks, notes, and other commands

**What Tests Should Do (E2E Testing):**
```rust
// Server should BEHAVE like real Checkvist API:
// 1. Parse request body
// 2. Check for checklist[name] parameter
// 3. If missing → return "Name this list"
// 4. If present → use the provided name
// 5. NO assertions on request format
```

**The Test Should Only Verify:**
- CLI output matches what server returned
- Status codes are correct
- Error handling works

**Root Cause:**
The test infrastructure was designed for **contract testing** (verify client sends correct format) rather than **integration testing** (verify the whole system works).

**Required Actions:**
1. **REWRITE** `tests/common/mod.rs` to simulate real API behavior
2. **REMOVE** all `.with_body_check()` assertions
3. **IMPLEMENT** actual Checkvist API logic in stub server
4. **REWRITE** all existing tests to verify behavior, not implementation
5. **RUN** rewritten tests to discover other hidden bugs

**Risk Assessment:**
- Given that list creation is broken and tests passed, assume **all commands may have similar parameter bugs**
- Task creation already fails (bug #2) - likely same root cause
- Test rewrite will probably uncover 5-10 more bugs

---

## Bugs

### 1. `lists create` doesn't respect name parameter
**Status:** ✅ FIXED (commit 521f6c2)
**Severity:** Medium
**Discovered:** 2025-12-22
**Fixed:** 2025-12-22

**Description:**
When creating a new list with `checkvist-cli lists create "List Name"`, the list was created with a placeholder name "Name this list" instead of the provided name.

**Root Cause:**
The code was sending parameter as `name=...` instead of `checklist[name]=...`. The Checkvist API uses Rails-style nested parameters and ignores the incorrectly formatted parameter.

**Fix:**
Changed line 252 in `src/api.rs`:
```rust
// Before:
.send_form(&[("name", name)])

// After:
.send_form(&[("checklist[name]", name)])
```

**Verification:**
All 7 e2e tests pass, including:
- `test_lists_create_with_valid_name_succeeds`
- `test_lists_create_with_unicode_name_succeeds`
- `test_lists_create_multiple_lists_all_appear`

---

### 2. `tasks create` returns 400 Bad Request
**Status:** 🐛 Bug
**Severity:** High
**Discovered:** 2025-12-22

**Description:**
Creating tasks fails with "unexpected status 400: Bad Request" error.

**Steps to reproduce:**
```bash
checkvist-cli tasks create --list-id 944404 --content "Test task"
# Exit code: 5
# Error: unexpected status 400: Bad Request
```

**Context:**
This error occurred when trying to add tasks to a newly created list (ID: 944404). The list was readable (`lists show` and `tasks get` both worked), but task creation failed.

**Possible causes:**
- May be related to the list having placeholder name "Name this list"
- Could be an issue with request formatting
- May be a server-side validation issue

**Investigation needed:**
- Try creating tasks in an existing, properly named list
- Check the actual HTTP request being sent
- Verify against Checkvist API documentation

---

## Missing Features

### 3. `lists update` cannot rename lists
**Status:** 🎯 Feature Gap
**Priority:** Medium
**Discovered:** 2025-12-22

**Description:**
The `lists update` command supports archiving and privacy settings, but doesn't provide a way to rename a list.

**Current capabilities:**
```bash
checkvist-cli lists update LIST_ID --archive
checkvist-cli lists update LIST_ID --unarchive
checkvist-cli lists update LIST_ID --public
checkvist-cli lists update LIST_ID --private
```

**Missing capability:**
```bash
checkvist-cli lists update LIST_ID --name "New Name"  # Not supported
```

**Attempted workaround:**
```bash
checkvist-cli lists update 944404 --name "checkvist-cli development"
# Exit code: 2
# Error: unexpected argument: Long("name")
```

**Checkvist API support:**
Need to verify if the Checkvist API supports renaming lists via the update endpoint. If so, this is a missing feature in checkvist-cli. If not, it's an API limitation.

**Workaround:**
Users must rename lists through the web interface at https://checkvist.com

---

## Improvements

### 4. Better error messages for API failures
**Status:** 💡 Enhancement
**Priority:** Low

**Description:**
When API calls fail with 400 Bad Request, the error message doesn't provide details about what went wrong. Including the API response body would help debugging.

**Current behavior:**
```
Exit code 5
unexpected status 400: Bad Request
```

**Suggested improvement:**
```
Exit code 5
unexpected status 400: Bad Request
Response: {"error": "List name cannot be 'Name this list'"}
```

---

### 5. Validate list names during creation
**Status:** 💡 Enhancement
**Priority:** Low

**Description:**
If certain list names are invalid or cause issues (like "Name this list"), the CLI should either:
1. Validate and reject them before making the API call
2. Properly pass the name to the API so it doesn't use a placeholder

---

### 6. Verbose logging doesn't show HTTP request/response details
**Status:** 💡 Enhancement
**Priority:** Medium

**Description:**
The `-v`, `-vv`, and `-vvv` flags enable verbose logging but don't show HTTP request/response details, making debugging API issues difficult.

**Current behavior:**
```bash
checkvist-cli -vvv lists create "Test"
# Shows: config loading, profile info, etc.
# Missing: HTTP method, URL, headers, request body, response status, response body
```

**Expected behavior:**
Verbose modes should show:
- `-v`: Basic request info (method, URL)
- `-vv`: Add request/response headers
- `-vvv`: Add request/response bodies (with sensitive data redacted)

**Current verbosity levels:**
- Shows config resolution and auth file loading
- Does NOT show any HTTP trace information

**Use case:**
When debugging issues like bug #1 (list creation) and bug #2 (task creation), HTTP traces would immediately reveal what's being sent to the API and what the API is responding with.

**Implementation notes:**
The `log.rs` module has infrastructure for verbosity levels, but HTTP logging is not implemented in `api.rs`.

---

## Investigation Needed

### 6. Verify Checkvist API behavior for list creation
**Status:** 🔍 Research
**Priority:** High

**Tasks:**
- [ ] Review Checkvist API documentation for `POST /checklists` endpoint
- [ ] Test list creation via curl to verify expected request format
- [ ] Compare checkvist-cli's request format with API documentation
- [ ] Determine if the name parameter is being sent correctly

### 7. Test task creation with properly named lists
**Status:** 🔍 Research
**Priority:** High

**Tasks:**
- [ ] Create or select an existing list with a proper name
- [ ] Attempt to create tasks in that list
- [ ] Determine if the 400 error is specific to placeholder-named lists or a general issue

---

## Testing Checklist

When fixing these issues, verify:

- [ ] List creation with simple ASCII name
- [ ] List creation with Unicode name (Cyrillic, emoji, etc.)
- [ ] List creation with special characters
- [ ] List creation with empty name
- [ ] List creation with very long name
- [ ] Task creation in newly created lists
- [ ] Task creation in existing lists
- [ ] List renaming via update command
- [ ] Error messages include helpful debugging information

---

## Success Story

✅ **Claude Code skill for checkvist-cli created successfully!**

The new `/checkvist` skill was created and successfully helped discover these issues during its first use. The skill is located at:
- `~/work/checkvist-cli/claude/skills/checkvist/SKILL.md`
- Symlinked to: `~/.claude/skills/checkvist`

The skill successfully demonstrated:
- Authentication verification
- Listing active checklists (75 found)
- Listing archived checklists (243 found)
- JSON format output with timestamps and metadata
- Error detection and reporting

---

**Last updated:** 2025-12-22
