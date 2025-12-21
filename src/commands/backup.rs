use super::ensure_token;
use super::request;
use crate::api::{ChecklistsResponse, CheckvistApi};
use crate::cfg::AuthConfig;
use crate::cli::BackupArgs;
use crate::error::{AppError, AppResult, ErrorKind};
use bzip2::Compression;
use bzip2::write::BzEncoder;
use chrono::Local;
use std::fs::File;
use std::path::PathBuf;
use tar::Builder;

pub fn run_backup(args: BackupArgs, api: &CheckvistApi, config: &AuthConfig) -> AppResult<()> {
    let output_dir = if args.output.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        args.output.clone()
    };
    std::fs::create_dir_all(&output_dir).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!(
                "unable to create output directory {}: {}",
                output_dir.display(),
                err
            ),
        )
    })?;

    let file_name = if args.date {
        format!("checkvist-{}.tar.bz2", Local::now().format("%d-%m-%Y"))
    } else {
        "checkvist.tar.bz2".to_string()
    };
    let output_path = output_dir.join(file_name);

    let lists = fetch_checklists(api, config, None)?;
    let archived_lists = fetch_checklists(api, config, Some(true))?;

    let file = File::create(&output_path).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!(
                "unable to create backup file {}: {}",
                output_path.display(),
                err
            ),
        )
    })?;
    let encoder = BzEncoder::new(file, Compression::best());
    let mut builder = Builder::new(encoder);

    append_str(&mut builder, "content.json", &lists.raw)?;
    append_str(&mut builder, "content_archived.json", &archived_lists.raw)?;

    for list in lists.items.iter().chain(archived_lists.items.iter()) {
        let id = list
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| AppError::new(ErrorKind::ApiData, "missing id in checklist"))?;
        let name = list
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::new(ErrorKind::ApiData, "missing name in checklist"))?;

        if !args.no_log {
            println!("Fetching {}", name);
        }

        let opml = fetch_opml(api, config, id)?;
        let entry_name = format!("{}.opml", sanitize_entry_name(name));
        append_str(&mut builder, &entry_name, &opml)?;
    }

    let encoder = builder.into_inner().map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!(
                "failed to finish archive {}: {}",
                output_path.display(),
                err
            ),
        )
    })?;
    encoder.finish().map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to finalize backup: {}", err),
        )
    })?;

    Ok(())
}

fn fetch_checklists(
    api: &CheckvistApi,
    config: &AuthConfig,
    archived: Option<bool>,
) -> AppResult<ChecklistsResponse> {
    let token = ensure_token(api, config)?;
    let login = || login(api, config);
    request::with_token_retry(
        api,
        &config.token_file,
        token,
        |api, token| api.get_checklists_raw(token, archived, None, None),
        login,
    )
}

fn fetch_opml(api: &CheckvistApi, config: &AuthConfig, list_id: i64) -> AppResult<String> {
    let token = ensure_token(api, config)?;
    let login = || login(api, config);
    request::with_token_retry(
        api,
        &config.token_file,
        token,
        |api, token| api.get_checklist_opml(token, list_id),
        login,
    )
}

fn login(api: &CheckvistApi, config: &AuthConfig) -> AppResult<String> {
    api.login(
        &config.username,
        &config.remote_key,
        config.token2fa.as_deref(),
    )
}

fn append_str(builder: &mut Builder<BzEncoder<File>>, name: &str, contents: &str) -> AppResult<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.as_bytes().len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, name, contents.as_bytes())
        .map_err(|err| {
            AppError::new(
                ErrorKind::Local,
                format!("unable to write {name} to archive: {err}"),
            )
        })
}

fn sanitize_entry_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    if sanitized.is_empty() {
        "untitled".to_string()
    } else {
        sanitized
    }
}
