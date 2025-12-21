use crate::error::{AppError, AppResult, ErrorKind};
use config::{File, FileFormat};
use dirs::home_dir;
use std::env;
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

#[derive(Debug, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn load(
        &self,
        profile_override: Option<String>,
        base_url_override: Option<String>,
        auth_file_override: Option<PathBuf>,
        token_file_override: Option<PathBuf>,
    ) -> AppResult<AuthConfig> {
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

        let mut builder = config::Config::builder();
        builder = builder.add_source(
            File::from(auth_file.clone())
                .format(FileFormat::Ini)
                .required(true),
        );

        let settings = builder.build().map_err(|err| {
            AppError::new(
                ErrorKind::Local,
                format!(
                    "failed to load auth config {}: {}",
                    auth_file.display(),
                    err
                ),
            )
        })?;

        let username = env::var("CHECKVIST_USERNAME")
            .ok()
            .or_else(|| settings.get_string(&format!("{}.username", profile)).ok())
            .ok_or_else(|| AppError::new(ErrorKind::Auth, "missing username"))?;

        let remote_key = env::var("CHECKVIST_REMOTE_KEY")
            .ok()
            .or_else(|| settings.get_string(&format!("{}.remote_key", profile)).ok())
            .ok_or_else(|| AppError::new(ErrorKind::Auth, "missing remote_key"))?;

        let token2fa = env::var("CHECKVIST_TOKEN2FA")
            .ok()
            .or_else(|| settings.get_string(&format!("{}.token2fa", profile)).ok());

        Ok(AuthConfig {
            base_url,
            profile,
            auth_file,
            token_file,
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
