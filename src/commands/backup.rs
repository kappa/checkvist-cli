use super::ensure_token;
use super::request;
use crate::api::CheckvistApi;
use crate::cfg::AuthConfig;
use crate::cli::BackupArgs;
use crate::error::{AppError, AppResult, ErrorKind};
use std::fs;
use std::path::PathBuf;

pub fn run_backup(args: BackupArgs, api: &CheckvistApi, config: &AuthConfig) -> AppResult<()> {
    let output_dir = if args.output.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        args.output.clone()
    };

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output_dir).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!(
                "unable to create output directory {}: {}",
                output_dir.display(),
                err
            ),
        )
    })?;

    // Fetch active and archived lists
    let token = ensure_token(api, config)?;
    let login = || api.login(&config.username, &config.remote_key, config.token2fa.as_deref());

    if !args.no_log {
        eprintln!("Fetching active checklists...");
    }
    let lists = request::with_token_retry(
        api,
        &config.token_file,
        token.clone(),
        |api, token| api.get_checklists(token, None, None, None),
        login,
    )?;

    if !args.no_log {
        eprintln!("Fetching archived checklists...");
    }
    let token = ensure_token(api, config)?;
    let login = || api.login(&config.username, &config.remote_key, config.token2fa.as_deref());
    let archived_lists = request::with_token_retry(
        api,
        &config.token_file,
        token,
        |api, token| api.get_checklists(token, Some(true), None, None),
        login,
    )?;

    // Export each list as OPML
    for list in lists.iter().chain(archived_lists.iter()) {
        let id = list
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| AppError::new(ErrorKind::ApiData, "missing id in checklist"))?;
        let name = list
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::new(ErrorKind::ApiData, "missing name in checklist"))?;

        if !args.no_log {
            eprintln!("Exporting: {}", name);
        }

        let token = ensure_token(api, config)?;
        let login = || api.login(&config.username, &config.remote_key, config.token2fa.as_deref());
        let opml = request::with_token_retry(
            api,
            &config.token_file,
            token,
            |api, token| api.get_checklist_opml(token, id),
            login,
        )?;

        let filename = format!("{}-{}.opml", id, sanitize_filename(name));
        let file_path = output_dir.join(&filename);

        fs::write(&file_path, opml).map_err(|err| {
            AppError::new(
                ErrorKind::Local,
                format!("unable to write {}: {}", file_path.display(), err),
            )
        })?;
    }

    if !args.no_log {
        eprintln!(
            "Backup complete: {} lists exported to {}",
            lists.len() + archived_lists.len(),
            output_dir.display()
        );
    }

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized
    }
}
