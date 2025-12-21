use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn help_includes_global_flags_and_subcommands() {
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("checkvist-cli"))
        .stdout(predicate::str::contains("--format <FORMAT>"))
        .stdout(predicate::str::contains("--profile <PROFILE>"))
        .stdout(predicate::str::contains("--base-url <BASE_URL>"))
        .stdout(predicate::str::contains("--auth-file <AUTH_FILE>"))
        .stdout(predicate::str::contains("--token-file <TOKEN_FILE>"))
        .stdout(predicate::str::contains("-v, --verbose"))
        .stdout(predicate::str::contains("lists"))
        .stdout(predicate::str::contains("auth"));
}

#[test]
fn lists_get_help_shows_options() {
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["lists", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--archived"))
        .stdout(predicate::str::contains("--order <ORDER>"))
        .stdout(predicate::str::contains("--skip-stats"));
}

#[test]
fn auth_status_help_shows_output_formats() {
    let mut cmd = cargo_bin_cmd!("checkvist-cli");
    cmd.args(["auth", "status", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("auth status"))
        .stdout(predicate::str::contains("--format"));
}
