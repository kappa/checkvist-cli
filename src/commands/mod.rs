use crate::api::{CheckvistApi, Order};
use crate::cfg::{ConfigLoader, ResolvedConfig};
use crate::cli::{
    AuthCommand, AuthLoginArgs, Cli, Commands, ListsArgs, ListsSubcommand, NotesArgs,
    NotesSubcommand, TaskStatus, TasksCommand,
};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::output::{print_auth_status, print_list, print_lists, print_notes, print_tasks};
use crate::token_store;
use crate::{cfg, cli};
use clap::CommandFactory;
use std::io::{self, Write};

pub mod backup;
pub mod request;

pub fn dispatch(cli: Cli) -> AppResult<()> {
    let loader = ConfigLoader::new();
    let profile_override = if cli.profile == "default" {
        None
    } else {
        Some(cli.profile.clone())
    };
    let base_url_override = if cli.base_url == "https://checkvist.com" {
        None
    } else {
        Some(cli.base_url.clone())
    };

    let command = match cli.command {
        None => {
            return Err(AppError::new(
                ErrorKind::Argument,
                cli::Cli::command().render_help().to_string(),
            ));
        }
        Some(command) => command,
    };

    if let Commands::Auth(AuthCommand::Login(args)) = &command {
        let resolved = loader.resolve(
            profile_override,
            base_url_override,
            cli.auth_file.clone(),
            cli.token_file.clone(),
        );
        return run_login(&resolved, args);
    }

    let missing_auth_hint = match &command {
        Commands::Auth(AuthCommand::Status(_)) => cfg::MissingAuthHint::AuthLogin,
        _ => cfg::MissingAuthHint::AuthStatus,
    };

    let config = loader.load(
        profile_override,
        base_url_override,
        cli.auth_file.clone(),
        cli.token_file.clone(),
        missing_auth_hint,
    )?;
    let api = CheckvistApi::new(config.base_url.clone())?;

    match command {
        Commands::Lists(args) => handle_lists(cli.format, args, &api, &config),
        Commands::Tasks(cmd) => handle_tasks(cli.format, cmd, &api, &config),
        Commands::Backup(args) => backup::run_backup(args, &api, &config),
        Commands::Notes(args) => handle_notes(cli.format, args, &api, &config),
        Commands::Auth(AuthCommand::Status(args)) => {
            let token = ensure_token(&api, &config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let user = request::with_token_retry(
                &api,
                &config.token_file,
                token,
                |api, token| api.auth_status(token),
                login,
            )?;

            print_auth_status(&user, args.format)?;
            Ok(())
        }
        Commands::Auth(AuthCommand::Login(_)) => unreachable!("handled above"),
    }
}

pub(super) fn ensure_token(
    api: &CheckvistApi,
    config: &crate::cfg::AuthConfig,
) -> AppResult<String> {
    if let Some(token) = token_store::read_token(&config.token_file)? {
        return Ok(token);
    }

    let token = api.login(
        &config.username,
        &config.remote_key,
        config.token2fa.as_deref(),
    )?;
    token_store::write_token(&config.token_file, &token)?;
    Ok(token)
}

fn parse_order(raw: Option<&str>) -> AppResult<Option<Order>> {
    match raw {
        Some("id:asc") => Ok(Some(Order::IdAsc)),
        Some("id:desc") => Ok(Some(Order::IdDesc)),
        Some("updated_at:asc") => Ok(Some(Order::UpdatedAtAsc)),
        Some("updated_at:desc") => Ok(Some(Order::UpdatedAtDesc)),
        Some(other) => Err(AppError::new(
            ErrorKind::Argument,
            format!("invalid order: {}", other),
        )),
        None => Ok(None),
    }
}

fn handle_lists(
    format: crate::cli::OutputFormat,
    args: ListsArgs,
    api: &CheckvistApi,
    config: &crate::cfg::AuthConfig,
) -> AppResult<()> {
    let command = args
        .command
        .unwrap_or_else(|| ListsSubcommand::Get(args.list.clone()));

    match command {
        ListsSubcommand::Get(args) => {
            let order = parse_order(args.order.as_deref())?;
            let skip_stats = if args.with_stats {
                false
            } else if args.skip_stats {
                true
            } else {
                true
            };

            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let lists = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| {
                    api.get_checklists(token, Some(args.archived), order, Some(skip_stats))
                },
                login,
            )?;
            print_lists(&lists, format)?;
            Ok(())
        }
        ListsSubcommand::Create(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let list = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.create_checklist(token, &args.name),
                login,
            )?;
            print_list(&list, format)?;
            Ok(())
        }
        ListsSubcommand::Delete(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let tasks = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.get_tasks(token, args.list_id),
                login,
            )?;

            if !tasks.is_empty() {
                return Err(AppError::new(
                    ErrorKind::Argument,
                    "cannot delete non-empty list",
                ));
            }

            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.delete_checklist(token, args.list_id),
                login,
            )?;
            Ok(())
        }
        ListsSubcommand::Update(args) => {
            let archived = if args.archive {
                Some(true)
            } else if args.unarchive {
                Some(false)
            } else {
                None
            };
            let public = if args.public {
                Some(true)
            } else if args.private {
                Some(false)
            } else {
                None
            };

            if archived.is_none() && public.is_none() {
                return Err(AppError::new(
                    ErrorKind::Argument,
                    "no updates provided (use --archive/--unarchive/--public/--private)",
                ));
            }

            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let list = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.update_checklist(token, args.list_id, archived, public),
                login,
            )?;
            print_list(&list, format)?;
            Ok(())
        }
        ListsSubcommand::Show(args) => {
            if args.tasks {
                return handle_tasks(
                    format,
                    TasksCommand::Get(crate::cli::TasksGetArgs {
                        list_id: args.list_id,
                    }),
                    api,
                    config,
                );
            }

            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let list = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.get_checklist(token, args.list_id),
                login,
            )?;
            print_list(&list, format)?;
            Ok(())
        }
    }
}

fn handle_tasks(
    format: crate::cli::OutputFormat,
    command: TasksCommand,
    api: &CheckvistApi,
    config: &crate::cfg::AuthConfig,
) -> AppResult<()> {
    match command {
        TasksCommand::Get(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let tasks = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.get_tasks(token, args.list_id),
                login,
            )?;
            print_tasks(&tasks, format)?;
            Ok(())
        }
        TasksCommand::Create(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let task = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.create_task(token, args.list_id, &args.content, args.parent_id),
                login,
            )?;
            print_tasks(&[task], format)?;
            Ok(())
        }
        TasksCommand::Update(args) => {
            if args.content.is_none() && args.status.is_none() && args.parent_id.is_none() {
                return Err(AppError::new(
                    ErrorKind::Argument,
                    "no updates provided for task",
                ));
            }
            let status = args.status.map(|s| match s {
                TaskStatus::Open => "open",
                TaskStatus::Done => "done",
            });
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let task = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| {
                    api.update_task(
                        token,
                        args.list_id,
                        args.task_id,
                        args.content.as_deref(),
                        status,
                        args.parent_id,
                    )
                },
                login,
            )?;
            print_tasks(&[task], format)?;
            Ok(())
        }
        TasksCommand::Remove(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.delete_task(token, args.list_id, args.task_id),
                login,
            )?;
            Ok(())
        }
    }
}

fn handle_notes(
    format: crate::cli::OutputFormat,
    args: NotesArgs,
    api: &CheckvistApi,
    config: &crate::cfg::AuthConfig,
) -> AppResult<()> {
    let command = match args.command {
        Some(cmd) => cmd,
        None => {
            let note_args = args.note.ok_or_else(|| {
                AppError::new(ErrorKind::Argument, "list and task are required for notes")
            })?;
            NotesSubcommand::List(note_args)
        }
    };

    match command {
        NotesSubcommand::List(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let notes = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.get_notes(token, args.list_id, args.task_id),
                login,
            )?;
            print_notes(&notes, format)?;
            Ok(())
        }
        NotesSubcommand::Create(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let note = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.create_note(token, args.list_id, args.task_id, &args.text),
                login,
            )?;
            print_notes(&[note], format)?;
            Ok(())
        }
        NotesSubcommand::Update(args) => {
            if args.text.is_none() {
                return Err(AppError::new(
                    ErrorKind::Argument,
                    "no updates provided for note",
                ));
            }
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let note = request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| {
                    api.update_note(
                        token,
                        args.list_id,
                        args.task_id,
                        args.note_id,
                        args.text.as_deref(),
                    )
                },
                login,
            )?;
            print_notes(&[note], format)?;
            Ok(())
        }
        NotesSubcommand::Remove(args) => {
            let token = ensure_token(api, config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            request::with_token_retry(
                api,
                &config.token_file,
                token,
                |api, token| api.delete_note(token, args.list_id, args.task_id, args.note_id),
                login,
            )?;
            Ok(())
        }
    }
}

fn run_login(resolved: &ResolvedConfig, _args: &AuthLoginArgs) -> AppResult<()> {
    let base = resolved.base_url.trim_end_matches('/');
    let login_url = format!("{}/auth/login", base);
    let api_key_url = format!("{}/auth/profile", base);

    println!(
        "We'll create credentials for profile \"{}\" in {}",
        resolved.profile,
        resolved.auth_file.display()
    );
    println!("1) Open and sign in: {login_url}");
    println!("2) Copy your Remote API key from: {api_key_url}");
    println!("3) Paste your login name and Remote API key below.");
    println!("(Press Enter to leave 2FA token empty if you don't use it.)\n");

    let mut stdout = io::stdout();
    let stdin = io::stdin();

    print!("Checkvist login/email: ");
    stdout.flush().ok();
    let mut username = String::new();
    stdin.read_line(&mut username).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read username: {}", err),
        )
    })?;

    print!("Remote API key: ");
    stdout.flush().ok();
    let mut remote_key = String::new();
    stdin.read_line(&mut remote_key).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read remote key: {}", err),
        )
    })?;

    print!("2FA token (optional): ");
    stdout.flush().ok();
    let mut token2fa = String::new();
    stdin.read_line(&mut token2fa).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read 2FA token: {}", err),
        )
    })?;

    let username = username.trim().to_string();
    let remote_key = remote_key.trim().to_string();
    let token2fa = token2fa.trim().to_string();
    let token2fa = (!token2fa.is_empty()).then_some(token2fa);

    cfg::write_auth_config(
        &resolved.auth_file,
        &resolved.profile,
        &username,
        &remote_key,
        token2fa.as_deref(),
    )?;

    println!(
        "Saved credentials for profile \"{}\" to {}",
        resolved.profile,
        resolved.auth_file.display()
    );
    println!("Verifying with `checkvist auth status`...");
    let api = CheckvistApi::new(resolved.base_url.clone())?;
    let config = crate::cfg::AuthConfig {
        base_url: resolved.base_url.clone(),
        profile: resolved.profile.clone(),
        auth_file: resolved.auth_file.clone(),
        token_file: resolved.token_file.clone(),
        username,
        remote_key,
        token2fa,
    };
    let token = api.login(
        &config.username,
        &config.remote_key,
        config.token2fa.as_deref(),
    )?;
    token_store::write_token(&config.token_file, &token)?;
    let login = || {
        api.login(
            &config.username,
            &config.remote_key,
            config.token2fa.as_deref(),
        )
    };
    let user = request::with_token_retry(
        &api,
        &config.token_file,
        token,
        |api, token| api.auth_status(token),
        login,
    )?;
    print_auth_status(&user, cli::OutputFormat::Text)?;

    Ok(())
}
