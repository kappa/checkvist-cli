use crate::cli::OutputFormat;
use crate::commands::request::{format_list_line, format_lists, format_notes, format_task_tree};
use crate::error::{AppError, AppResult, ErrorKind};
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
            serde_json::to_writer(&mut handle, &obj)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
    }
    Ok(())
}

pub fn print_list(list: &Value, format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let line = format_list_line(list).ok_or_else(|| {
                AppError::new(ErrorKind::ApiData, "missing id or name in checklist")
            })?;
            writeln!(handle, "{}", line)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer(&mut handle, list)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
    }
    Ok(())
}

pub fn print_tasks(tasks: &[Value], format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for line in format_task_tree(tasks) {
                writeln!(handle, "{}", line).map_err(|err| {
                    AppError::new(ErrorKind::Local, format!("write error: {}", err))
                })?;
            }
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let obj = serde_json::json!({"tasks": tasks});
            serde_json::to_writer(&mut handle, &obj)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
    }
    Ok(())
}

pub fn print_auth_status(user: &Value, format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let user_obj = user.get("user");
            let email = user_obj
                .and_then(|u| u.get("email"))
                .and_then(|e| e.as_str());
            let login = user_obj
                .and_then(|u| u.get("login"))
                .and_then(|e| e.as_str());
            let name = user_obj
                .and_then(|u| u.get("name"))
                .and_then(|e| e.as_str());
            let id = user_obj.and_then(|u| u.get("id")).and_then(|e| e.as_i64());
            let label = email
                .or(login)
                .or(name)
                .map(str::to_string)
                .or_else(|| id.map(|id| format!("user {}", id)))
                .unwrap_or_else(|| "authenticated".to_string());
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "ok\t{}", label)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer(&mut handle, user)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
    }
    Ok(())
}

pub fn print_notes(notes: &[Value], format: OutputFormat) -> AppResult<()> {
    match format {
        OutputFormat::Text => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for line in format_notes(notes) {
                writeln!(handle, "{}", line).map_err(|err| {
                    AppError::new(ErrorKind::Local, format!("write error: {}", err))
                })?;
            }
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let obj = serde_json::json!({"notes": notes});
            serde_json::to_writer(&mut handle, &obj)
                .map_err(|err| AppError::new(ErrorKind::Local, format!("write error: {}", err)))?;
        }
    }
    Ok(())
}
