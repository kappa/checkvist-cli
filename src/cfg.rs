use crate::error::{AppError, AppResult, ErrorKind};
use config::{File, FileFormat};
use dirs::home_dir;
use std::env;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub base_url: String,
    pub profile: String,
    pub auth_file: PathBuf,
    pub token_file: PathBuf,
    pub username: String,
    pub remote_key: String,
    pub token2fa: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub base_url: String,
    pub profile: String,
    pub auth_file: PathBuf,
    pub token_file: PathBuf,
}

#[derive(Debug, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve(
        &self,
        profile_override: Option<String>,
        base_url_override: Option<String>,
        auth_file_override: Option<PathBuf>,
        token_file_override: Option<PathBuf>,
    ) -> ResolvedConfig {
        let profile = profile_override
            .or_else(|| env::var("CHECKVIST_PROFILE").ok())
            .unwrap_or_else(|| "default".to_string());

        let base_url = base_url_override
            .or_else(|| env::var("CHECKVIST_BASE_URL").ok())
            .unwrap_or_else(|| "https://checkvist.com".to_string());

        let auth_file = auth_file_override
            .or_else(|| env::var("CHECKVIST_AUTH_FILE").ok().map(PathBuf::from))
            .unwrap_or_else(default_auth_file);

        let token_file = token_file_override
            .or_else(|| env::var("CHECKVIST_TOKEN_FILE").ok().map(PathBuf::from))
            .unwrap_or_else(default_token_file);

        ResolvedConfig {
            base_url,
            profile,
            auth_file,
            token_file,
        }
    }

    pub fn load(
        &self,
        profile_override: Option<String>,
        base_url_override: Option<String>,
        auth_file_override: Option<PathBuf>,
        token_file_override: Option<PathBuf>,
    ) -> AppResult<AuthConfig> {
        let resolved = self.resolve(
            profile_override,
            base_url_override,
            auth_file_override,
            token_file_override,
        );

        let mut builder = config::Config::builder();
        builder = builder.add_source(
            File::from(resolved.auth_file.clone())
                .format(FileFormat::Ini)
                .required(true),
        );

        let settings = match builder.build() {
            Ok(settings) => settings,
            Err(config::ConfigError::NotFound(_)) => {
                return Err(missing_auth_error(&resolved, "auth file not found"));
            }
            Err(err) => {
                return Err(AppError::new(
                    ErrorKind::Local,
                    format!(
                        "failed to load auth config {}: {}",
                        resolved.auth_file.display(),
                        err
                    ),
                ));
            }
        };

        let username = env::var("CHECKVIST_USERNAME")
            .ok()
            .or_else(|| {
                settings
                    .get_string(&format!("{}.username", resolved.profile))
                    .ok()
            })
            .ok_or_else(|| missing_auth_error(&resolved, "missing username"))?;

        let remote_key = env::var("CHECKVIST_REMOTE_KEY")
            .ok()
            .or_else(|| {
                settings
                    .get_string(&format!("{}.remote_key", resolved.profile))
                    .ok()
            })
            .ok_or_else(|| missing_auth_error(&resolved, "missing remote_key"))?;

        let token2fa = env::var("CHECKVIST_TOKEN2FA").ok().or_else(|| {
            settings
                .get_string(&format!("{}.token2fa", resolved.profile))
                .ok()
        });

        Ok(AuthConfig {
            base_url: resolved.base_url,
            profile: resolved.profile,
            auth_file: resolved.auth_file,
            token_file: resolved.token_file,
            username,
            remote_key,
            token2fa,
        })
    }
}

fn default_auth_file() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".checkvist")
        .join("auth.ini")
}

fn default_token_file() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".checkvist")
        .join("token")
}

fn missing_auth_error(resolved: &ResolvedConfig, reason: &str) -> AppError {
    let base = resolved.base_url.trim_end_matches('/');
    let login_url = format!("{}/auth/login", base);
    let api_key_url = format!("{}/auth/profile", base);

    AppError::new(
        ErrorKind::Auth,
        format!(
            "Authentication data for profile \"{}\" is missing ({reason}).\n\
Save your Checkvist login and Remote API key in {} or run `checkvist login` to create it.\n\
Sign in at {login_url} and copy your Remote API key from {api_key_url}.",
            resolved.profile,
            resolved.auth_file.display(),
        ),
    )
}

pub fn write_auth_config(
    auth_file: &PathBuf,
    profile: &str,
    username: &str,
    remote_key: &str,
    token2fa: Option<&str>,
) -> AppResult<()> {
    if let Some(parent) = auth_file.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            AppError::new(
                ErrorKind::Local,
                format!(
                    "unable to create auth directory {}: {}",
                    parent.display(),
                    err
                ),
            )
        })?;
    }

    let mut contents = format!("[{profile}]\nusername = {username}\nremote_key = {remote_key}\n");

    if let Some(token2fa) = token2fa {
        if !token2fa.trim().is_empty() {
            contents.push_str(&format!("token2fa = {}\n", token2fa.trim()));
        }
    }

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = options.open(auth_file).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!(
                "unable to open auth file {} for writing: {}",
                auth_file.display(),
                err
            ),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file
            .metadata()
            .map_err(|err| {
                AppError::new(
                    ErrorKind::Local,
                    format!(
                        "unable to get metadata for {}: {}",
                        auth_file.display(),
                        err
                    ),
                )
            })?
            .permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(auth_file, perms).ok();
    }

    file.write_all(contents.as_bytes()).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("unable to write auth file {}: {}", auth_file.display(), err),
        )
    })?;

    Ok(())
}
