use crate::error::{AppError, AppResult, ErrorKind};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub fn read_token(path: &Path) -> AppResult<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let mut file = File::open(path).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("unable to open token file {}: {}", path.display(), err),
        )
    })?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("unable to read token file {}: {}", path.display(), err),
        )
    })?;

    let token = contents.trim();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token.to_string()))
    }
}

pub fn write_token(path: &Path, token: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            AppError::new(
                ErrorKind::Local,
                format!(
                    "unable to create token directory {}: {}",
                    parent.display(),
                    err
                ),
            )
        })?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    let mut file = options.open(path).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("unable to open token file {}: {}", path.display(), err),
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
                    format!("unable to get metadata for {}: {}", path.display(), err),
                )
            })?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms).ok();
    }

    file.write_all(token.as_bytes()).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("unable to write token file {}: {}", path.display(), err),
        )
    })?;

    Ok(())
}
