use crate::error::{AppError, AppResult, ErrorKind};
use crate::commands::request::format_lists;
use crate::cli::OutputFormat;
use serde_json::Value;
use std::io::{self, Write};

pub fn print_lists(lists: &[Value], format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for line in format_lists(lists) {
                writeln!(handle, "{}", line).map_err(|err| {
                    AppError::new(ErrorKind::Local, format!("write error: {}", err))
                })?;
            }
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let obj = serde_json::json!({"lists": lists});
            serde_json::to_writer(&mut handle, &obj).map_err(|err| {
                AppError::new(ErrorKind::Local, format!("write error: {}", err))
            })?;
        }
    }
    Ok(())
}

pub fn print_auth_status(user: &Value, format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let email = user
                .get("user")
                .and_then(|u| u.get("email"))
                .and_then(|e| e.as_str())
                .unwrap_or("unknown");
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "ok\t{}", email).map_err(|err| {
                AppError::new(ErrorKind::Local, format!("write error: {}", err))
            })?;
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer(&mut handle, user).map_err(|err| {
                AppError::new(ErrorKind::Local, format!("write error: {}", err))
            })?;
        }
    }
    Ok(())
}
