use checkvist_cli::cfg::{ConfigLoader, MissingAuthHint};
use checkvist_cli::token_store;
use serial_test::serial;
use std::env;
use std::fs;
use tempfile::tempdir;

fn clear_env() {
    let vars = [
        "CHECKVIST_PROFILE",
        "CHECKVIST_BASE_URL",
        "CHECKVIST_USERNAME",
        "CHECKVIST_REMOTE_KEY",
        "CHECKVIST_TOKEN2FA",
        "CHECKVIST_AUTH_FILE",
        "CHECKVIST_TOKEN_FILE",
    ];

    for var in vars {
        unsafe {
            env::remove_var(var);
        }
    }
}

#[test]
#[serial]
fn env_overrides_ini_values() {
    clear_env();
    let dir = tempdir().expect("temp dir");
    let auth_path = dir.path().join("auth.ini");
    fs::write(
        &auth_path,
        "[default]\nusername = file_user\nremote_key = FILEKEY\n\n[work]\nusername = work_user\nremote_key = WORKKEY\n",
    )
    .expect("write ini");

    unsafe {
        env::set_var("CHECKVIST_PROFILE", "work");
        env::set_var("CHECKVIST_BASE_URL", "https://example.test");
        env::set_var("CHECKVIST_USERNAME", "env_user");
        env::set_var("CHECKVIST_REMOTE_KEY", "ENVKEY");
        env::set_var("CHECKVIST_AUTH_FILE", auth_path.to_str().unwrap());
        env::set_var(
            "CHECKVIST_TOKEN_FILE",
            dir.path().join("token").to_str().unwrap(),
        );
    }

    let loader = ConfigLoader::new();
    let config = loader
        .load(None, None, None, None, MissingAuthHint::AuthStatus)
        .expect("config loads");

    assert_eq!(config.profile, "work");
    assert_eq!(config.base_url, "https://example.test");
    assert_eq!(config.username, "env_user");
    assert_eq!(config.remote_key, "ENVKEY");
    assert_eq!(config.auth_file, auth_path);
    assert_eq!(config.token_file, dir.path().join("token"));
}

#[test]
#[serial]
fn cli_overrides_take_top_priority() {
    clear_env();
    let dir = tempdir().expect("temp dir");
    let auth_path = dir.path().join("auth.ini");
    fs::write(
        &auth_path,
        "[default]\nusername = file_user\nremote_key = FILEKEY\n\n[cli_profile]\nusername = cli_user\nremote_key = CLIKEY\n",
    )
    .expect("write ini");

    unsafe {
        env::remove_var("CHECKVIST_USERNAME");
        env::remove_var("CHECKVIST_REMOTE_KEY");
    }

    let loader = ConfigLoader::new();
    let config = loader
        .load(
            Some("cli_profile".into()),
            Some("https://cli.example".into()),
            Some(auth_path.clone()),
            Some(dir.path().join("token_cli")),
            MissingAuthHint::AuthStatus,
        )
        .expect("config loads");

    assert_eq!(config.profile, "cli_profile");
    assert_eq!(config.base_url, "https://cli.example");
    assert_eq!(config.auth_file, auth_path);
    assert_eq!(config.token_file, dir.path().join("token_cli"));
    assert_eq!(config.username, "cli_user");
    assert_eq!(config.remote_key, "CLIKEY");
}

#[test]
#[serial]
fn missing_credentials_results_in_auth_error() {
    clear_env();
    let dir = tempdir().expect("temp dir");
    let auth_path = dir.path().join("auth.ini");
    fs::write(&auth_path, "[default]\nusername = only_user\n").expect("write ini");

    let loader = ConfigLoader::new();
    let err = loader
        .load(
            None,
            None,
            Some(auth_path.clone()),
            Some(dir.path().join("token")),
            MissingAuthHint::AuthStatus,
        )
        .expect_err("should fail without remote key");

    assert_eq!(err.kind(), checkvist_cli::error::ErrorKind::Auth);
}

#[test]
#[serial]
fn token_store_round_trip() {
    let dir = tempdir().expect("temp dir");
    let token_path = dir.path().join("token");

    assert!(token_store::read_token(&token_path).unwrap().is_none());

    token_store::write_token(&token_path, "ABC123").expect("write token");
    let read_back = token_store::read_token(&token_path).unwrap();
    assert_eq!(read_back.as_deref(), Some("ABC123"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&token_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}
