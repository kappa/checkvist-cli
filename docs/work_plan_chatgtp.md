## Development document: `checkvist-cli` (Rust) — transparent Checkvist API CLI (revised)

### 1) Goal and hard constraints

Build a small, portable, **sync** Rust CLI that maps cleanly to the Checkvist Open API and is suitable for manual use or automation/agents.

**Hard requirements**

* Binary name: **`checkvist-cli`**
* **Synchronous Rust only** (no Tokio / async-std)
* HTTP client: **`ureq`** (minimize dependencies; use a small TLS footprint) ([Crates][1])
* CLI parsing: **`clap`**
* Config system: crate **`config`** with **INI support** (via feature) and layered overrides (files + env + in-process overrides). ([GitHub][2])
* Output format: universal `--format text|json`, default **text** (no colors)
* Auth file `~/.checkvist/auth.ini` contains **login + remote_key** (no token)
* Token is stored separately at `~/.checkvist/token`
* **No `CHECKVIST_TOKEN` env var support** (token is not config)
* **Strict TDD**: write test first, then implementation
* CLI integration tests must use **assert_cmd** patterns. ([Crates][3])
* No JSON schemas.

---

### 2) UX contract

#### 2.1 Global flags

* `--format <text|json>` (default: `text`)
* `--profile <name>` (default: `default`)
* `--base-url <url>` (default: `https://checkvist.com`)
* `--auth-file <path>` (default: `~/.checkvist/auth.ini`)
* `--token-file <path>` (default: `~/.checkvist/token`)
* `-v/--verbose` (stderr logs; redact secrets)

#### 2.2 Output rules

* **stdout**: command output only
* **stderr**: errors/logs only

#### 2.3 Exit codes

* `0` success
* `2` clap/argument error
* `3` auth error (missing creds, login/refresh rejected)
* `4` network/transport error
* `5` API/data error (unexpected status/JSON)
* `6` local error (config parse, filesystem)

---

### 3) Config and credential sources

#### 3.1 Auth config file: `~/.checkvist/auth.ini`

Parsed via `config` crate with INI support. ([GitHub][2])

Example:

```ini
[default]
username = you@example.com
remote_key = XXXXX
# token2fa = 123456   # optional, only if needed

[work]
username = you@work.com
remote_key = YYYYY
```

**Env overrides (supported)**

* `CHECKVIST_PROFILE`
* `CHECKVIST_BASE_URL`
* `CHECKVIST_AUTH_FILE`
* `CHECKVIST_TOKEN_FILE`
* `CHECKVIST_USERNAME`
* `CHECKVIST_REMOTE_KEY`
* `CHECKVIST_TOKEN2FA` (optional)

**Precedence**

1. CLI flags
2. env vars
3. INI file selected section
4. defaults

#### 3.2 Token file: `~/.checkvist/token`

* Single-line text token.
* File permissions: attempt `0600` on Unix.
* Not handled by `config` crate (explicit requirement).

---

### 4) Authentication and token lifecycle (required behavior)

Checkvist endpoints (version=2):

* Login: `GET/POST /auth/login.json?version=2` → returns `{ "token": "..." }` ([Checkvist][4])
* Refresh: `GET/POST /auth/refresh_token.json?version=2` → returns new token; used when token expires ([Checkvist][4])

#### 4.1 Token acquisition flow

On any command that requires API access:

1. **If token file exists** and contains a token → try request with it.
2. If token file missing/empty → **login** using username + remote_key, save token to token file, then proceed.

#### 4.2 Expired/invalid token flow (retry policy)

On any API request:

* If response indicates auth failure (403/401):

  1. Attempt **refresh_token** using the current token.
  2. If refresh succeeds → overwrite token file → **retry the original request once**.
  3. If refresh fails → **login** using remote_key → overwrite token file → **retry the original request once**.
  4. If that fails → exit `3` auth error.

**Limits**

* At most **one refresh attempt** and **one re-login attempt** per command execution to avoid loops.

#### 4.3 Storage and secrecy

* Never print token/remote_key in logs.
* Token file is the only persisted secret for day-to-day operation.

---

### 5) Command surface (v0)

#### 5.1 `lists get` (core)

**Command:** `checkvist-cli lists get`

Maps to: `GET /checklists.json` (one call; no client-side aggregation).

Options:

* `--archived` (bool)
* `--order <id:asc|id:desc|updated_at:asc>`
* `--skip-stats` (bool)

Output:

* `--format text` (default): one list per line
  `12345\tMy List Name`
* `--format json`:

```json
{ "lists": [ { ... }, { ... } ] }
```

(Implement as passthrough JSON objects; typed struct is optional.)

#### 5.2 `auth status` (token sanity check)

**Command:** `checkvist-cli auth status`

Calls: `GET /auth/curr_user.json` (or equivalent in API) to validate token and display identity.
If Checkvist returns 403/401 → apply refresh/relogin logic (same as any request).

Output:

* text: `ok\t<user identifier>`
* json: `{ "user": { ... } }`

---

### 6) HTTP client requirements (sync ureq)

* Use `ureq::Agent` with timeouts (connect 5s, read/write total ~20s).
* Send `Accept: application/json`.
* Authenticate via header `X-Client-Token: <token>` (preferred).
* TLS choice:

  * keep dependency footprint minimal; explicitly choose ureq feature set in `Cargo.toml` (ureq documents rustls/native-tls options). ([Crates][1])

Error mapping:

* Network/TLS → exit `4`
* Non-JSON when JSON expected → exit `5`
* 4xx (non-auth) → exit `5` with concise server message if available

---

### 7) Implementation architecture (testable, minimal)

```
src/
  main.rs            # clap + dispatch + exit codes
  cli.rs             # clap structs
  cfg.rs             # config crate integration (auth.ini + env + overrides)
  token_store.rs     # read/write ~/.checkvist/token
  api.rs             # CheckvistApi (ureq) + request wrapper with auth retry policy
  commands/
    mod.rs
    lists.rs
    auth.rs
  output.rs          # text/json formatting utilities
tests/
  cli_lists_get.rs
  cli_auth_status.rs
```

Key design point: `api.rs` owns the “request with token lifecycle” wrapper so every command gets consistent refresh/relogin semantics.

---

### 8) Testing plan (strict TDD)

#### 8.1 Integration tests (assert_cmd) — required

Use assert_cmd to run the compiled binary and verify stdout/stderr/exit codes. ([Crates][3])

To avoid async deps, use a tiny synchronous local HTTP stub in tests (e.g., `tiny_http` or a minimal `std::net::TcpListener` responder) so you can program scripted sequences:

* first request returns 403 (expired token)
* refresh endpoint returns new token
* retried request returns success

All tests should set:

* `CHECKVIST_BASE_URL` to stub server URL
* `CHECKVIST_AUTH_FILE` to temp INI
* `CHECKVIST_TOKEN_FILE` to temp token file path
* `CHECKVIST_PROFILE` as needed

#### 8.2 Test-first sequence (recommended)

1. **No token file; creds present** → CLI performs login, saves token, then `lists get` succeeds.
2. **Token file present; request succeeds** → no login call is made.
3. **Token file present; request 403; refresh succeeds** → refresh called, token overwritten, original retried, success.
4. **Token file present; request 403; refresh fails; login succeeds** → login called, token overwritten, original retried, success.
5. **Token missing and creds missing** → exit `3`.
6. **API returns malformed JSON** → exit `5`.
7. `--format json` produces valid JSON envelope; `--format text` produces stable tab-separated lines.

Each test initially fails; only then implement the minimal code to pass.

---

### 9) Milestones (each milestone is TDD-driven)

**M0 — Skeleton**

* clap command tree, `--format`, config paths, exit codes.

**M1 — Config + token store**

* Load auth.ini via `config` (INI + env overrides). ([GitHub][2])
* Read/write token file.

**M2 — API client**

* `GET /checklists.json`
* `POST/GET /auth/login.json?version=2`
* `POST/GET /auth/refresh_token.json?version=2` ([Checkvist][4])

**M3 — Auth retry wrapper**

* Implement “request → on 403 refresh→ retry; else relogin → retry” logic.

**M4 — Commands**

* `lists get`, `auth status`

**M5 — Docs**

* README: setup, profiles, examples, troubleshooting.

---

### 10) Deliverables (v0)

* `checkvist-cli` binary
* Profiles in `~/.checkvist/auth.ini` (no tokens stored there)
* Token persisted in `~/.checkvist/token` and refreshed/reacquired automatically
* Commands: `lists get`, `auth status`
* `--format text|json` (default text)
* Full assert_cmd test suite covering token lifecycle (login/refresh/relogin)

---

If you want the next step to be maximally mechanical for a coder, I can write a **test plan as actual Rust test skeletons** (assert_cmd + a tiny stub HTTP server that can return scripted responses), in the exact order they should be written.

[1]: https://crates.io/crates/ureq?utm_source=chatgpt.com "ureq - crates.io: Rust Package Registry"
[2]: https://github.com/rust-cli/config-rs?utm_source=chatgpt.com "rust-cli/config-rs: ⚙️ Layered configuration system ..."
[3]: https://crates.io/crates/rust-ini?utm_source=chatgpt.com "rust-ini - crates.io: Rust Package Registry"
[4]: https://checkvist.com/auth/api?utm_source=chatgpt.com "Open API"

### References

Checkvist API: https://checkvist.com/auth/api.
A simple Checkvist client in Rust: https://github.com/crispinb/cvcap.
