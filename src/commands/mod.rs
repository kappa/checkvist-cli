use crate::api::{CheckvistApi, Order};
use crate::cfg::ConfigLoader;
use crate::cli::{
    AuthCommand, Cli, Commands, ListsArgs, ListsSubcommand, TaskStatus, TasksCommand,
};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::output::{print_auth_status, print_list, print_lists, print_tasks};
use crate::token_store;

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
    let config = loader.load(
        profile_override,
        base_url_override,
        cli.auth_file.clone(),
        cli.token_file.clone(),
    )?;

    let api = CheckvistApi::new(config.base_url.clone());

    match cli.command {
        Some(Commands::Lists(args)) => handle_lists(cli.format, args, &api, &config),
        Some(Commands::Tasks(cmd)) => handle_tasks(cli.format, cmd, &api, &config),
        Some(Commands::Auth(AuthCommand::Status(args))) => {
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
        None => Err(AppError::new(ErrorKind::Argument, "no command provided")),
    }
}

fn ensure_token(api: &CheckvistApi, config: &crate::cfg::AuthConfig) -> AppResult<String> {
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
    let order = parse_order(args.list.order.as_deref())?;
    let command = args
        .command
        .unwrap_or_else(|| ListsSubcommand::Get(args.list.clone()));

    match command {
        ListsSubcommand::Get(args) => {
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
                    api.get_checklists(token, Some(args.archived), order, Some(!args.with_stats))
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
