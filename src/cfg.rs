use crate::error::{AppError, AppResult, ErrorKind};
use crate::{log::redact_sensitive, vlog};
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

#[derive(Debug, Clone, Copy)]
pub enum MissingAuthHint {
    AuthStatus,
    AuthLogin,
}

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
        vlog!(1, "Config resolution starting");

        let profile = profile_override
            .clone()
            .or_else(|| {
                env::var("CHECKVIST_PROFILE").ok().map(|v| {
                    vlog!(1, "  profile from CHECKVIST_PROFILE env: {}", v);
                    v
                })
            })
            .unwrap_or_else(|| {
                vlog!(1, "  profile from default: default");
                "default".to_string()
            });
        if profile_override.is_some() {
            vlog!(1, "  profile from CLI flag: {}", profile);
        }

        let base_url = base_url_override
            .clone()
            .or_else(|| {
                env::var("CHECKVIST_BASE_URL").ok().map(|v| {
                    vlog!(1, "  base_url from CHECKVIST_BASE_URL env: {}", v);
                    v
                })
            })
            .unwrap_or_else(|| {
                vlog!(1, "  base_url from default: https://checkvist.com");
                "https://checkvist.com".to_string()
            });
        if base_url_override.is_some() {
            vlog!(1, "  base_url from CLI flag: {}", base_url);
        }

        let auth_file = auth_file_override
            .clone()
            .or_else(|| {
                env::var("CHECKVIST_AUTH_FILE").ok().map(|v| {
                    let p = PathBuf::from(&v);
                    vlog!(1, "  auth_file from CHECKVIST_AUTH_FILE env: {}", p.display());
                    p
                })
            })
            .unwrap_or_else(|| {
                let p = default_auth_file();
                vlog!(1, "  auth_file from default: {}", p.display());
                p
            });
        if auth_file_override.is_some() {
            vlog!(1, "  auth_file from CLI flag: {}", auth_file.display());
        }

        let token_file = token_file_override
            .clone()
            .or_else(|| {
                env::var("CHECKVIST_TOKEN_FILE").ok().map(|v| {
                    let p = PathBuf::from(&v);
                    vlog!(1, "  token_file from CHECKVIST_TOKEN_FILE env: {}", p.display());
                    p
                })
            })
            .unwrap_or_else(|| {
                let p = default_token_file();
                vlog!(1, "  token_file from default: {}", p.display());
                p
            });
        if token_file_override.is_some() {
            vlog!(1, "  token_file from CLI flag: {}", token_file.display());
        }

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
        hint: MissingAuthHint,
    ) -> AppResult<AuthConfig> {
        let resolved = self.resolve(
            profile_override,
            base_url_override,
            auth_file_override,
            token_file_override,
        );

        vlog!(1, "Loading auth config from: {}", resolved.auth_file.display());
        vlog!(1, "Using profile: {}", resolved.profile);

        let mut builder = config::Config::builder();
        builder = builder.add_source(
            File::from(resolved.auth_file.clone())
                .format(FileFormat::Ini)
                .required(true),
        );

        let settings = match builder.build() {
            Ok(settings) => {
                vlog!(1, "Auth config file loaded successfully");
                settings
            }
            Err(config::ConfigError::NotFound(_)) => {
                vlog!(1, "Auth config file not found");
                return Err(missing_auth_error(&resolved, "auth file not found", hint));
            }
            Err(err) => {
                if is_missing_auth_file(&err) {
                    vlog!(1, "Auth config file not found");
                    return Err(missing_auth_error(&resolved, "auth file not found", hint));
                }
                vlog!(1, "Failed to parse auth config: {}", err);
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
            .map(|v| {
                vlog!(1, "  username from CHECKVIST_USERNAME env: {}", v);
                v
            })
            .or_else(|| {
                settings
                    .get_string(&format!("{}.username", resolved.profile))
                    .ok()
                    .map(|v| {
                        vlog!(1, "  username from INI file: {}", v);
                        v
                    })
            })
            .ok_or_else(|| missing_auth_error(&resolved, "missing username", hint))?;

        let remote_key = env::var("CHECKVIST_REMOTE_KEY")
            .ok()
            .map(|v| {
                vlog!(1, "  remote_key from CHECKVIST_REMOTE_KEY env: {}***", &v[..3.min(v.len())]);
                v
            })
            .or_else(|| {
                settings
                    .get_string(&format!("{}.remote_key", resolved.profile))
                    .ok()
                    .map(|v| {
                        vlog!(1, "  remote_key from INI file: {}", redact_sensitive(&v, 3));
                        v
                    })
            })
            .ok_or_else(|| missing_auth_error(&resolved, "missing remote_key", hint))?;

        let token2fa = env::var("CHECKVIST_TOKEN2FA")
            .ok()
            .map(|v| {
                vlog!(1, "  token2fa from CHECKVIST_TOKEN2FA env: {}", redact_sensitive(&v, 2));
                v
            })
            .or_else(|| {
                settings
                    .get_string(&format!("{}.token2fa", resolved.profile))
                    .ok()
                    .map(|v| {
                        vlog!(1, "  token2fa from INI file: {}", redact_sensitive(&v, 2));
                        v
                    })
            });

        vlog!(1, "Auth config loaded successfully");

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

fn missing_auth_error(
    resolved: &ResolvedConfig,
    reason: &str,
    hint: MissingAuthHint,
) -> AppError {
    let base = resolved.base_url.trim_end_matches('/');
    let login_url = format!("{}/auth/login", base);
    let api_key_url = format!("{}/auth/profile", base);

    let next_step = match hint {
        MissingAuthHint::AuthStatus => format!(
            "Save your Checkvist login and Remote API key in {} or run `checkvist auth status` for setup guidance.",
            resolved.auth_file.display(),
        ),
        MissingAuthHint::AuthLogin => format!(
            "Save your Checkvist login and Remote API key in {} or run `checkvist auth login` to create it.",
            resolved.auth_file.display(),
        ),
    };

    AppError::new(
        ErrorKind::Auth,
        format!(
            "Authentication data for profile \"{}\" is missing ({reason}).\n\
{next_step}\n\
Sign in at {login_url} and copy your Remote API key from {api_key_url}.",
            resolved.profile,
        ),
    )
}

fn is_missing_auth_file(err: &config::ConfigError) -> bool {
    match err {
        config::ConfigError::FileParse { cause, .. } => {
            if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
                io_err.kind() == std::io::ErrorKind::NotFound
            } else {
                err.to_string().to_lowercase().contains("not found")
            }
        }
        config::ConfigError::NotFound(_) => true,
        config::ConfigError::Foreign(_) | config::ConfigError::Message(_) => {
            err.to_string().to_lowercase().contains("not found")
        }
        _ => false,
    }
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
