use crate::error::{AppError, AppResult, ErrorKind};
use lexopt::{Arg, Parser};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Text
    }
}

#[derive(Debug)]
pub struct Cli {
    pub format: OutputFormat,
    pub profile: String,
    pub base_url: String,
    pub auth_file: Option<PathBuf>,
    pub token_file: Option<PathBuf>,
    pub verbose: u8,
    pub command: Option<Commands>,
}

#[derive(Debug)]
pub enum Commands {
    Lists(ListsArgs),
    Auth(AuthCommand),
    Tasks(TasksCommand),
    Backup(BackupArgs),
    Notes(NotesArgs),
}

#[derive(Debug, Clone)]
pub struct ListsArgs {
    pub list: ListsGetArgs,
    pub command: Option<ListsSubcommand>,
}

#[derive(Debug, Clone)]
pub enum ListsSubcommand {
    Get(ListsGetArgs),
    Create(ListsCreateArgs),
    Delete(ListsDeleteArgs),
    Update(ListsUpdateArgs),
    Show(ListsShowArgs),
}

#[derive(Debug, Clone)]
pub struct ListsGetArgs {
    pub archived: bool,
    pub order: Option<String>,
    pub with_stats: bool,
    pub skip_stats: bool,
}

#[derive(Debug, Clone)]
pub struct ListsCreateArgs {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ListsDeleteArgs {
    pub list_id: i64,
}

#[derive(Debug, Clone)]
pub struct ListsUpdateArgs {
    pub list_id: i64,
    pub archive: bool,
    pub unarchive: bool,
    pub private: bool,
    pub public: bool,
}

#[derive(Debug, Clone)]
pub struct ListsShowArgs {
    pub list_id: i64,
    pub tasks: bool,
}

#[derive(Debug)]
pub enum AuthCommand {
    Status(AuthStatusArgs),
    Login(AuthLoginArgs),
}

#[derive(Debug)]
pub struct AuthStatusArgs {
    pub format: OutputFormat,
}

#[derive(Debug)]
pub struct AuthLoginArgs {}

#[derive(Debug)]
pub enum TasksCommand {
    Get(TasksGetArgs),
    Create(TasksCreateArgs),
    Update(TasksUpdateArgs),
    Remove(TasksRemoveArgs),
}

#[derive(Debug, Clone)]
pub struct TasksGetArgs {
    pub list_id: i64,
}

#[derive(Debug, Clone)]
pub struct TasksCreateArgs {
    pub list_id: i64,
    pub content: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TasksUpdateArgs {
    pub list_id: i64,
    pub task_id: i64,
    pub content: Option<String>,
    pub status: Option<TaskStatus>,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TasksRemoveArgs {
    pub list_id: i64,
    pub task_id: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum TaskStatus {
    Open,
    Done,
}

#[derive(Debug, Clone)]
pub struct BackupArgs {
    pub output: PathBuf,
    pub no_log: bool,
}

#[derive(Debug, Clone)]
pub struct NotesArgs {
    pub note: Option<NotesGetArgs>,
    pub command: Option<NotesSubcommand>,
}

#[derive(Debug, Clone)]
pub enum NotesSubcommand {
    List(NotesGetArgs),
    Create(NotesCreateArgs),
    Update(NotesUpdateArgs),
    Remove(NotesRemoveArgs),
}

#[derive(Debug, Clone)]
pub struct NotesGetArgs {
    pub list_id: i64,
    pub task_id: i64,
}

#[derive(Debug, Clone)]
pub struct NotesCreateArgs {
    pub list_id: i64,
    pub task_id: i64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct NotesUpdateArgs {
    pub list_id: i64,
    pub task_id: i64,
    pub note_id: i64,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NotesRemoveArgs {
    pub list_id: i64,
    pub task_id: i64,
    pub note_id: i64,
}

pub enum ParseOutcome {
    Cli(Cli),
    Help,
}

pub fn parse_from_env() -> AppResult<ParseOutcome> {
    let mut cli = Cli {
        format: OutputFormat::Text,
        profile: "default".to_string(),
        base_url: "https://checkvist.com".to_string(),
        auth_file: None,
        token_file: None,
        verbose: 0,
        command: None,
    };

    let mut parser = Parser::from_env();
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_main();
                return Ok(ParseOutcome::Help);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                cli.format = parse_output_format(parser.value().map_err(map_lexopt_error)?)?;
            }
            Arg::Long("profile") => {
                cli.profile = parse_string(parser.value().map_err(map_lexopt_error)?)?;
            }
            Arg::Long("base-url") => {
                cli.base_url = parse_string(parser.value().map_err(map_lexopt_error)?)?;
            }
            Arg::Long("auth-file") => {
                cli.auth_file = Some(PathBuf::from(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?));
            }
            Arg::Long("token-file") => {
                cli.token_file = Some(PathBuf::from(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?));
            }
            Arg::Value(command) => {
                let command_outcome = match parse_command(command, &mut parser, &mut cli) {
                    Ok(outcome) => outcome,
                    Err(err) if err.kind() == ErrorKind::Argument && err.message() == "help printed" => {
                        return Ok(ParseOutcome::Help);
                    }
                    Err(err) => return Err(err),
                };
                match command_outcome {
                    CommandOutcome::Command(cmd) => cli.command = Some(cmd),
                    CommandOutcome::Help => return Ok(ParseOutcome::Help),
                }
                break;
            }
            _ => return Err(unexpected_argument(arg, USAGE_MAIN)),
        }
    }

    if cli.command.is_none() {
        return Err(usage_error("missing command", USAGE_MAIN));
    }

    Ok(ParseOutcome::Cli(cli))
}

fn parse_command(
    command: OsString,
    parser: &mut Parser,
    cli: &mut Cli,
) -> AppResult<CommandOutcome> {
    let name = parse_string(command)?;
    match name.as_str() {
        "lists" | "list" => parse_lists(parser, cli),
        "auth" => parse_auth(parser, cli),
        "tasks" | "task" => parse_tasks(parser, cli),
        "backup" => parse_backup(parser, cli),
        "notes" | "note" => parse_notes(parser, cli),
        _ => Err(usage_error(format!("unknown command: {name}"), USAGE_MAIN)),
    }
}

enum CommandOutcome {
    Command(Commands),
    Help,
}

fn parse_lists(parser: &mut Parser, cli: &mut Cli) -> AppResult<CommandOutcome> {
    let mut default_args = ListsGetArgs {
        archived: false,
        order: None,
        with_stats: false,
        skip_stats: false,
    };
    let mut default_used = false;

    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists();
                return Ok(CommandOutcome::Help);
            }
            Arg::Long("archived") => {
                default_args.archived = true;
                default_used = true;
            }
            Arg::Long("order") => {
                default_args.order = Some(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
                default_used = true;
            }
            Arg::Long("with-stats") => {
                default_args.with_stats = true;
                default_used = true;
            }
            Arg::Long("skip-stats") => {
                default_args.skip_stats = true;
                default_used = true;
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            Arg::Value(value) => {
                if default_used {
                    return Err(usage_error(
                        "options cannot be combined with a subcommand",
                        USAGE_LISTS,
                    ));
                }
                let sub = parse_string(value)?;
                return parse_lists_subcommand(parser, cli, &sub);
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS)),
        }
    }

    validate_lists_get(&default_args)?;
    Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
        list: default_args.clone(),
        command: Some(ListsSubcommand::Get(default_args)),
    })))
}

fn parse_lists_subcommand(
    parser: &mut Parser,
    cli: &mut Cli,
    sub: &str,
) -> AppResult<CommandOutcome> {
    match sub {
        "get" => {
            let args = parse_lists_get(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
                list: args.clone(),
                command: Some(ListsSubcommand::Get(args)),
            })))
        }
        "create" => {
            let args = parse_lists_create(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
                list: ListsGetArgs::default(),
                command: Some(ListsSubcommand::Create(args)),
            })))
        }
        "delete" => {
            let args = parse_lists_delete(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
                list: ListsGetArgs::default(),
                command: Some(ListsSubcommand::Delete(args)),
            })))
        }
        "update" => {
            let args = parse_lists_update(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
                list: ListsGetArgs::default(),
                command: Some(ListsSubcommand::Update(args)),
            })))
        }
        "show" => {
            let args = parse_lists_show(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Lists(ListsArgs {
                list: ListsGetArgs::default(),
                command: Some(ListsSubcommand::Show(args)),
            })))
        }
        _ => Err(usage_error(format!("unknown lists command: {sub}"), USAGE_LISTS)),
    }
}

fn parse_lists_get(parser: &mut Parser, cli: &mut Cli) -> AppResult<ListsGetArgs> {
    let mut args = ListsGetArgs::default();
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists_get();
                return Err(help_return());
            }
            Arg::Long("archived") => args.archived = true,
            Arg::Long("order") => {
                args.order = Some(parse_string(parser.value().map_err(map_lexopt_error)?)?);
            }
            Arg::Long("with-stats") => args.with_stats = true,
            Arg::Long("skip-stats") => args.skip_stats = true,
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS_GET)),
        }
    }
    validate_lists_get(&args)?;
    Ok(args)
}

fn parse_lists_create(parser: &mut Parser, cli: &mut Cli) -> AppResult<ListsCreateArgs> {
    let mut name: Option<String> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists_create();
                return Err(help_return());
            }
            Arg::Value(value) => {
                if name.is_some() {
                    return Err(usage_error("unexpected extra argument", USAGE_LISTS_CREATE));
                }
                name = Some(parse_string(value)?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS_CREATE)),
        }
    }
    let name = name.ok_or_else(|| usage_error("name is required", USAGE_LISTS_CREATE))?;
    Ok(ListsCreateArgs { name })
}

fn parse_lists_delete(parser: &mut Parser, cli: &mut Cli) -> AppResult<ListsDeleteArgs> {
    let mut list_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists_delete();
                return Err(help_return());
            }
            Arg::Value(value) => {
                if list_id.is_some() {
                    return Err(usage_error("unexpected extra argument", USAGE_LISTS_DELETE));
                }
                list_id = Some(parse_i64(value, "list id")?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS_DELETE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_LISTS_DELETE))?;
    Ok(ListsDeleteArgs { list_id })
}

fn parse_lists_update(parser: &mut Parser, cli: &mut Cli) -> AppResult<ListsUpdateArgs> {
    let mut list_id: Option<i64> = None;
    let mut archive = false;
    let mut unarchive = false;
    let mut private = false;
    let mut public = false;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists_update();
                return Err(help_return());
            }
            Arg::Value(value) => {
                if list_id.is_some() {
                    return Err(usage_error("unexpected extra argument", USAGE_LISTS_UPDATE));
                }
                list_id = Some(parse_i64(value, "list id")?);
            }
            Arg::Long("archive") => archive = true,
            Arg::Long("unarchive") => unarchive = true,
            Arg::Long("private") => private = true,
            Arg::Long("public") => public = true,
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS_UPDATE)),
        }
    }

    if archive && unarchive {
        return Err(usage_error(
            "cannot use --archive with --unarchive",
            USAGE_LISTS_UPDATE,
        ));
    }
    if private && public {
        return Err(usage_error(
            "cannot use --private with --public",
            USAGE_LISTS_UPDATE,
        ));
    }

    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_LISTS_UPDATE))?;
    Ok(ListsUpdateArgs {
        list_id,
        archive,
        unarchive,
        private,
        public,
    })
}

fn parse_lists_show(parser: &mut Parser, cli: &mut Cli) -> AppResult<ListsShowArgs> {
    let mut list_id: Option<i64> = None;
    let mut tasks = false;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_lists_show();
                return Err(help_return());
            }
            Arg::Value(value) => {
                if list_id.is_some() {
                    return Err(usage_error("unexpected extra argument", USAGE_LISTS_SHOW));
                }
                list_id = Some(parse_i64(value, "list id")?);
            }
            Arg::Long("tasks") => tasks = true,
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_LISTS_SHOW)),
        }
    }

    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_LISTS_SHOW))?;
    Ok(ListsShowArgs { list_id, tasks })
}

fn parse_auth(parser: &mut Parser, cli: &mut Cli) -> AppResult<CommandOutcome> {
    if let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_auth();
                return Ok(CommandOutcome::Help);
            }
            Arg::Value(value) => {
                let sub = parse_string(value)?;
                return match sub.as_str() {
                    "status" => {
                        let args = parse_auth_status(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Auth(AuthCommand::Status(
                            args,
                        ))))
                    }
                    "login" => {
                        parse_auth_login(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Auth(AuthCommand::Login(
                            AuthLoginArgs {},
                        ))))
                    }
                    _ => Err(usage_error(format!("unknown auth command: {sub}"), USAGE_AUTH)),
                };
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
                return Err(usage_error("auth subcommand is required", USAGE_AUTH));
            }
            other => return Err(unexpected_argument(other, USAGE_AUTH)),
        }
    }

    Err(usage_error("auth subcommand is required", USAGE_AUTH))
}

fn parse_auth_status(parser: &mut Parser, cli: &mut Cli) -> AppResult<AuthStatusArgs> {
    let mut format = OutputFormat::Text;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_auth_status();
                return Err(help_return());
            }
            Arg::Long("format") => {
                format = parse_output_format(parser.value().map_err(map_lexopt_error)?)?;
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_AUTH_STATUS)),
        }
    }
    Ok(AuthStatusArgs { format })
}

fn parse_auth_login(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_auth_login();
                return Err(help_return());
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_AUTH_LOGIN)),
        }
    }
    Ok(())
}

fn parse_tasks(parser: &mut Parser, cli: &mut Cli) -> AppResult<CommandOutcome> {
    if let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_tasks();
                return Ok(CommandOutcome::Help);
            }
            Arg::Value(value) => {
                let sub = parse_string(value)?;
                return match sub.as_str() {
                    "get" => {
                        let args = parse_tasks_get(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Tasks(TasksCommand::Get(
                            args,
                        ))))
                    }
                    "create" => {
                        let args = parse_tasks_create(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Tasks(TasksCommand::Create(
                            args,
                        ))))
                    }
                    "update" => {
                        let args = parse_tasks_update(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Tasks(TasksCommand::Update(
                            args,
                        ))))
                    }
                    "remove" => {
                        let args = parse_tasks_remove(parser, cli)?;
                        Ok(CommandOutcome::Command(Commands::Tasks(TasksCommand::Remove(
                            args,
                        ))))
                    }
                    _ => Err(usage_error(format!("unknown tasks command: {sub}"), USAGE_TASKS)),
                };
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
                return Err(usage_error("tasks subcommand is required", USAGE_TASKS));
            }
            other => return Err(unexpected_argument(other, USAGE_TASKS)),
        }
    }

    Err(usage_error("tasks subcommand is required", USAGE_TASKS))
}

fn parse_tasks_get(parser: &mut Parser, cli: &mut Cli) -> AppResult<TasksGetArgs> {
    let mut list_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_tasks_get();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_TASKS_GET)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_TASKS_GET))?;
    Ok(TasksGetArgs { list_id })
}

fn parse_tasks_create(parser: &mut Parser, cli: &mut Cli) -> AppResult<TasksCreateArgs> {
    let mut list_id: Option<i64> = None;
    let mut content: Option<String> = None;
    let mut parent_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_tasks_create();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("content") => {
                content = Some(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Long("parent-id") => {
                parent_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "parent id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_TASKS_CREATE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_TASKS_CREATE))?;
    let content = content.ok_or_else(|| usage_error("content is required", USAGE_TASKS_CREATE))?;
    Ok(TasksCreateArgs {
        list_id,
        content,
        parent_id,
    })
}

fn parse_tasks_update(parser: &mut Parser, cli: &mut Cli) -> AppResult<TasksUpdateArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    let mut content: Option<String> = None;
    let mut status: Option<TaskStatus> = None;
    let mut parent_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_tasks_update();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Long("content") => {
                content = Some(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Long("status") => {
                status = Some(parse_task_status(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Long("parent-id") => {
                parent_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "parent id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_TASKS_UPDATE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_TASKS_UPDATE))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_TASKS_UPDATE))?;
    Ok(TasksUpdateArgs {
        list_id,
        task_id,
        content,
        status,
        parent_id,
    })
}

fn parse_tasks_remove(parser: &mut Parser, cli: &mut Cli) -> AppResult<TasksRemoveArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_tasks_remove();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_TASKS_REMOVE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_TASKS_REMOVE))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_TASKS_REMOVE))?;
    Ok(TasksRemoveArgs { list_id, task_id })
}

fn parse_backup(parser: &mut Parser, cli: &mut Cli) -> AppResult<CommandOutcome> {
    let mut output = PathBuf::from(".");
    let mut no_log = false;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_backup();
                return Ok(CommandOutcome::Help);
            }
            Arg::Long("output") => {
                output = PathBuf::from(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Long("nolog") => no_log = true,
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_BACKUP)),
        }
    }
    Ok(CommandOutcome::Command(Commands::Backup(BackupArgs { output, no_log })))
}

fn parse_notes(parser: &mut Parser, cli: &mut Cli) -> AppResult<CommandOutcome> {
    let mut default_args = NotesGetArgs {
        list_id: 0,
        task_id: 0,
    };
    let mut default_seen = false;
    let mut default_valid = false;

    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_notes();
                return Ok(CommandOutcome::Help);
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                default_args.list_id = parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?;
                default_seen = true;
            }
            Arg::Long("task-id") | Arg::Long("task") => {
                default_args.task_id = parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?;
                default_seen = true;
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            Arg::Value(value) => {
                if default_seen {
                    return Err(usage_error(
                        "options cannot be combined with a subcommand",
                        USAGE_NOTES,
                    ));
                }
                let sub = parse_string(value)?;
                return parse_notes_subcommand(parser, cli, &sub);
            }
            other => return Err(unexpected_argument(other, USAGE_NOTES)),
        }
    }

    if default_seen {
        if default_args.list_id != 0 && default_args.task_id != 0 {
            default_valid = true;
        }
    }

    if !default_valid {
        return Err(usage_error("list and task are required for notes", USAGE_NOTES));
    }

    Ok(CommandOutcome::Command(Commands::Notes(NotesArgs {
        note: Some(default_args),
        command: None,
    })))
}

fn parse_notes_subcommand(
    parser: &mut Parser,
    cli: &mut Cli,
    sub: &str,
) -> AppResult<CommandOutcome> {
    match sub {
        "list" => {
            let args = parse_notes_get(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Notes(NotesArgs {
                note: Some(args.clone()),
                command: Some(NotesSubcommand::List(args)),
            })))
        }
        "create" => {
            let args = parse_notes_create(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Notes(NotesArgs {
                note: None,
                command: Some(NotesSubcommand::Create(args)),
            })))
        }
        "update" => {
            let args = parse_notes_update(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Notes(NotesArgs {
                note: None,
                command: Some(NotesSubcommand::Update(args)),
            })))
        }
        "remove" => {
            let args = parse_notes_remove(parser, cli)?;
            Ok(CommandOutcome::Command(Commands::Notes(NotesArgs {
                note: None,
                command: Some(NotesSubcommand::Remove(args)),
            })))
        }
        _ => Err(usage_error(format!("unknown notes command: {sub}"), USAGE_NOTES)),
    }
}

fn parse_notes_get(parser: &mut Parser, cli: &mut Cli) -> AppResult<NotesGetArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_notes_list();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") | Arg::Long("task") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_NOTES_LIST)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_NOTES_LIST))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_NOTES_LIST))?;
    Ok(NotesGetArgs { list_id, task_id })
}

fn parse_notes_create(parser: &mut Parser, cli: &mut Cli) -> AppResult<NotesCreateArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    let mut text: Option<String> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_notes_create();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") | Arg::Long("task") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Long("text") => {
                text = Some(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_NOTES_CREATE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_NOTES_CREATE))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_NOTES_CREATE))?;
    let text = text.ok_or_else(|| usage_error("text is required", USAGE_NOTES_CREATE))?;
    Ok(NotesCreateArgs {
        list_id,
        task_id,
        text,
    })
}

fn parse_notes_update(parser: &mut Parser, cli: &mut Cli) -> AppResult<NotesUpdateArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    let mut note_id: Option<i64> = None;
    let mut text: Option<String> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_notes_update();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") | Arg::Long("task") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Long("note-id") => {
                note_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "note id",
                )?);
            }
            Arg::Long("text") => {
                text = Some(parse_string(
                    parser.value().map_err(map_lexopt_error)?,
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_NOTES_UPDATE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_NOTES_UPDATE))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_NOTES_UPDATE))?;
    let note_id = note_id.ok_or_else(|| usage_error("note id is required", USAGE_NOTES_UPDATE))?;
    Ok(NotesUpdateArgs {
        list_id,
        task_id,
        note_id,
        text,
    })
}

fn parse_notes_remove(parser: &mut Parser, cli: &mut Cli) -> AppResult<NotesRemoveArgs> {
    let mut list_id: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    let mut note_id: Option<i64> = None;
    while let Some(arg) = parser.next().map_err(map_lexopt_error)? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                print_help_notes_remove();
                return Err(help_return());
            }
            Arg::Long("list-id") | Arg::Long("list") => {
                list_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "list id",
                )?);
            }
            Arg::Long("task-id") | Arg::Long("task") => {
                task_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "task id",
                )?);
            }
            Arg::Long("note-id") => {
                note_id = Some(parse_i64(
                    parser.value().map_err(map_lexopt_error)?,
                    "note id",
                )?);
            }
            Arg::Short('v') | Arg::Long("verbose") => {
                cli.verbose = cli.verbose.saturating_add(1);
            }
            Arg::Long("format") => {
                parse_global_format(parser, cli)?;
            }
            Arg::Long("profile") => {
                parse_global_profile(parser, cli)?;
            }
            Arg::Long("base-url") => {
                parse_global_base_url(parser, cli)?;
            }
            Arg::Long("auth-file") => {
                parse_global_auth_file(parser, cli)?;
            }
            Arg::Long("token-file") => {
                parse_global_token_file(parser, cli)?;
            }
            other => return Err(unexpected_argument(other, USAGE_NOTES_REMOVE)),
        }
    }
    let list_id = list_id.ok_or_else(|| usage_error("list id is required", USAGE_NOTES_REMOVE))?;
    let task_id = task_id.ok_or_else(|| usage_error("task id is required", USAGE_NOTES_REMOVE))?;
    let note_id = note_id.ok_or_else(|| usage_error("note id is required", USAGE_NOTES_REMOVE))?;
    Ok(NotesRemoveArgs {
        list_id,
        task_id,
        note_id,
    })
}

fn validate_lists_get(args: &ListsGetArgs) -> AppResult<()> {
    if args.with_stats && args.skip_stats {
        return Err(usage_error(
            "cannot use --with-stats with --skip-stats",
            USAGE_LISTS_GET,
        ));
    }
    Ok(())
}

fn parse_global_format(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    cli.format = parse_output_format(parser.value().map_err(map_lexopt_error)?)?;
    Ok(())
}

fn parse_global_profile(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    cli.profile = parse_string(parser.value().map_err(map_lexopt_error)?)?;
    Ok(())
}

fn parse_global_base_url(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    cli.base_url = parse_string(parser.value().map_err(map_lexopt_error)?)?;
    Ok(())
}

fn parse_global_auth_file(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    cli.auth_file = Some(PathBuf::from(parse_string(
        parser.value().map_err(map_lexopt_error)?,
    )?));
    Ok(())
}

fn parse_global_token_file(parser: &mut Parser, cli: &mut Cli) -> AppResult<()> {
    cli.token_file = Some(PathBuf::from(parse_string(
        parser.value().map_err(map_lexopt_error)?,
    )?));
    Ok(())
}

fn parse_output_format(value: OsString) -> AppResult<OutputFormat> {
    match parse_string(value)?.as_str() {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => Err(usage_error(
            format!("invalid format: {other}"),
            USAGE_MAIN,
        )),
    }
}

fn parse_task_status(value: OsString) -> AppResult<TaskStatus> {
    match parse_string(value)?.as_str() {
        "open" => Ok(TaskStatus::Open),
        "done" => Ok(TaskStatus::Done),
        other => Err(usage_error(
            format!("invalid status: {other}"),
            USAGE_TASKS_UPDATE,
        )),
    }
}

fn parse_string(value: OsString) -> AppResult<String> {
    value
        .into_string()
        .map_err(|_| AppError::new(ErrorKind::Argument, "invalid UTF-8 in argument"))
}

fn parse_i64(value: OsString, label: &str) -> AppResult<i64> {
    let raw = parse_string(value)?;
    raw.parse::<i64>().map_err(|_| {
        AppError::new(
            ErrorKind::Argument,
            format!("invalid {label}: {raw}"),
        )
    })
}

fn unexpected_argument(arg: Arg, usage: &str) -> AppError {
    usage_error(format!("unexpected argument: {arg:?}"), usage)
}

fn usage_error(message: impl Into<String>, usage: &str) -> AppError {
    AppError::new(
        ErrorKind::Argument,
        format!("{}\n\nUsage:\n  {}", message.into(), usage),
    )
}

fn help_return() -> AppError {
    AppError::new(ErrorKind::Argument, "help printed")
}

fn map_lexopt_error(err: lexopt::Error) -> AppError {
    AppError::new(ErrorKind::Argument, err.to_string())
}

pub fn main_help() -> &'static str {
    HELP_MAIN
}

fn print_help_main() {
    println!("{HELP_MAIN}");
}

fn print_help_lists() {
    println!("{HELP_LISTS}");
}

fn print_help_lists_get() {
    println!("{HELP_LISTS_GET}");
}

fn print_help_lists_create() {
    println!("{HELP_LISTS_CREATE}");
}

fn print_help_lists_delete() {
    println!("{HELP_LISTS_DELETE}");
}

fn print_help_lists_update() {
    println!("{HELP_LISTS_UPDATE}");
}

fn print_help_lists_show() {
    println!("{HELP_LISTS_SHOW}");
}

fn print_help_auth() {
    println!("{HELP_AUTH}");
}

fn print_help_auth_status() {
    println!("{HELP_AUTH_STATUS}");
}

fn print_help_auth_login() {
    println!("{HELP_AUTH_LOGIN}");
}

fn print_help_tasks() {
    println!("{HELP_TASKS}");
}

fn print_help_tasks_get() {
    println!("{HELP_TASKS_GET}");
}

fn print_help_tasks_create() {
    println!("{HELP_TASKS_CREATE}");
}

fn print_help_tasks_update() {
    println!("{HELP_TASKS_UPDATE}");
}

fn print_help_tasks_remove() {
    println!("{HELP_TASKS_REMOVE}");
}

fn print_help_backup() {
    println!("{HELP_BACKUP}");
}

fn print_help_notes() {
    println!("{HELP_NOTES}");
}

fn print_help_notes_list() {
    println!("{HELP_NOTES_LIST}");
}

fn print_help_notes_create() {
    println!("{HELP_NOTES_CREATE}");
}

fn print_help_notes_update() {
    println!("{HELP_NOTES_UPDATE}");
}

fn print_help_notes_remove() {
    println!("{HELP_NOTES_REMOVE}");
}

const USAGE_MAIN: &str = "checkvist-cli [OPTIONS] <COMMAND>";
const USAGE_LISTS: &str = "checkvist-cli lists [OPTIONS] [COMMAND]";
const USAGE_LISTS_GET: &str = "checkvist-cli lists get [OPTIONS]";
const USAGE_LISTS_CREATE: &str = "checkvist-cli lists create <NAME>";
const USAGE_LISTS_DELETE: &str = "checkvist-cli lists delete <LIST_ID>";
const USAGE_LISTS_UPDATE: &str = "checkvist-cli lists update <LIST_ID> [OPTIONS]";
const USAGE_LISTS_SHOW: &str = "checkvist-cli lists show <LIST_ID> [OPTIONS]";
const USAGE_AUTH: &str = "checkvist-cli auth <COMMAND>";
const USAGE_AUTH_STATUS: &str = "checkvist-cli auth status [OPTIONS]";
const USAGE_AUTH_LOGIN: &str = "checkvist-cli auth login";
const USAGE_TASKS: &str = "checkvist-cli tasks <COMMAND>";
const USAGE_TASKS_GET: &str = "checkvist-cli tasks get --list <LIST_ID>";
const USAGE_TASKS_CREATE: &str = "checkvist-cli tasks create --list <LIST_ID> --content <CONTENT>";
const USAGE_TASKS_UPDATE: &str =
    "checkvist-cli tasks update --list <LIST_ID> --task-id <TASK_ID> [OPTIONS]";
const USAGE_TASKS_REMOVE: &str = "checkvist-cli tasks remove --list <LIST_ID> --task-id <TASK_ID>";
const USAGE_BACKUP: &str = "checkvist-cli backup [OPTIONS]";
const USAGE_NOTES: &str = "checkvist-cli notes [OPTIONS] [COMMAND]";
const USAGE_NOTES_LIST: &str = "checkvist-cli notes list --list <LIST_ID> --task <TASK_ID>";
const USAGE_NOTES_CREATE: &str =
    "checkvist-cli notes create --list <LIST_ID> --task <TASK_ID> --text <TEXT>";
const USAGE_NOTES_UPDATE: &str =
    "checkvist-cli notes update --list <LIST_ID> --task <TASK_ID> --note-id <NOTE_ID> [OPTIONS]";
const USAGE_NOTES_REMOVE: &str =
    "checkvist-cli notes remove --list <LIST_ID> --task <TASK_ID> --note-id <NOTE_ID>";

const HELP_MAIN: &str = "\
checkvist-cli

Usage:
  checkvist-cli [OPTIONS] <COMMAND>

Options:
  --format <FORMAT>
  --profile <PROFILE>
  --base-url <BASE_URL>
  --auth-file <AUTH_FILE>
  --token-file <TOKEN_FILE>
  -v, --verbose

Commands:
  lists
  auth
  tasks
  backup
  notes
";

const HELP_LISTS: &str = "\
lists

Usage:
  checkvist-cli lists [OPTIONS] [COMMAND]
";

const HELP_LISTS_GET: &str = "\
lists get

Usage:
  checkvist-cli lists get [OPTIONS]

Options:
  --archived
  --order <ORDER>
  --skip-stats
  --with-stats
";

const HELP_LISTS_CREATE: &str = "\
lists create

Usage:
  checkvist-cli lists create <NAME>
";

const HELP_LISTS_DELETE: &str = "\
lists delete

Usage:
  checkvist-cli lists delete <LIST_ID>
";

const HELP_LISTS_UPDATE: &str = "\
lists update

Usage:
  checkvist-cli lists update <LIST_ID> [OPTIONS]
";

const HELP_LISTS_SHOW: &str = "\
lists show

Usage:
  checkvist-cli lists show <LIST_ID> [OPTIONS]
";

const HELP_AUTH: &str = "\
auth

Usage:
  checkvist-cli auth <COMMAND>
";

const HELP_AUTH_STATUS: &str = "\
auth status

Usage:
  checkvist-cli auth status [OPTIONS]

Options:
  --format <FORMAT>
";

const HELP_AUTH_LOGIN: &str = "\
auth login

Usage:
  checkvist-cli auth login
";

const HELP_TASKS: &str = "\
tasks

Usage:
  checkvist-cli tasks <COMMAND>
";

const HELP_TASKS_GET: &str = "\
tasks get

Usage:
  checkvist-cli tasks get --list <LIST_ID>
";

const HELP_TASKS_CREATE: &str = "\
tasks create

Usage:
  checkvist-cli tasks create --list <LIST_ID> --content <CONTENT>
";

const HELP_TASKS_UPDATE: &str = "\
tasks update

Usage:
  checkvist-cli tasks update --list <LIST_ID> --task-id <TASK_ID> [OPTIONS]
";

const HELP_TASKS_REMOVE: &str = "\
tasks remove

Usage:
  checkvist-cli tasks remove --list <LIST_ID> --task-id <TASK_ID>
";

const HELP_BACKUP: &str = "\
backup

Usage:
  checkvist-cli backup [OPTIONS]

Options:
  --output <DIR>
  --nolog
";

const HELP_NOTES: &str = "\
notes

Usage:
  checkvist-cli notes [OPTIONS] [COMMAND]
";

const HELP_NOTES_LIST: &str = "\
notes list

Usage:
  checkvist-cli notes list --list <LIST_ID> --task <TASK_ID>
";

const HELP_NOTES_CREATE: &str = "\
notes create

Usage:
  checkvist-cli notes create --list <LIST_ID> --task <TASK_ID> --text <TEXT>
";

const HELP_NOTES_UPDATE: &str = "\
notes update

Usage:
  checkvist-cli notes update --list <LIST_ID> --task <TASK_ID> --note-id <NOTE_ID> [OPTIONS]
";

const HELP_NOTES_REMOVE: &str = "\
notes remove

Usage:
  checkvist-cli notes remove --list <LIST_ID> --task <TASK_ID> --note-id <NOTE_ID>
";

impl Default for ListsGetArgs {
    fn default() -> Self {
        Self {
            archived: false,
            order: None,
            with_stats: false,
            skip_stats: false,
        }
    }
}
