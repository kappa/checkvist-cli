use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Text
    }
}

#[derive(Debug, Parser)]
#[command(name = "checkvist-cli")]
#[command(about = "Checkvist CLI", long_about = None)]
pub struct Cli {
    #[arg(
        long,
        default_value = "text",
        value_enum,
        value_name = "FORMAT",
        global = true
    )]
    pub format: OutputFormat,

    #[arg(long, default_value = "default", value_name = "PROFILE", global = true)]
    pub profile: String,

    #[arg(
        long,
        default_value = "https://checkvist.com",
        value_name = "BASE_URL",
        global = true
    )]
    pub base_url: String,

    #[arg(long, value_name = "AUTH_FILE", global = true)]
    pub auth_file: Option<PathBuf>,

    #[arg(long, value_name = "TOKEN_FILE", global = true)]
    pub token_file: Option<PathBuf>,

    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(name = "lists", alias = "list")]
    Lists(ListsArgs),
    #[command(subcommand)]
    Auth(AuthCommand),
    #[command(subcommand, name = "tasks", alias = "task")]
    Tasks(TasksCommand),
}

#[derive(Debug, Clone, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ListsArgs {
    #[command(flatten)]
    pub list: ListsGetArgs,

    #[command(subcommand)]
    pub command: Option<ListsSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ListsSubcommand {
    /// List all checklists
    Get(ListsGetArgs),
    /// Create a new checklist
    Create(ListsCreateArgs),
    /// Delete an existing checklist (only if empty)
    Delete(ListsDeleteArgs),
    /// Update checklist metadata
    Update(ListsUpdateArgs),
    /// Get metadata (or tasks) for a single checklist
    Show(ListsShowArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ListsGetArgs {
    #[arg(long)]
    pub archived: bool,

    #[arg(long, value_name = "ORDER")]
    pub order: Option<String>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub with_stats: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ListsCreateArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Debug, Clone, Args)]
pub struct ListsDeleteArgs {
    #[arg(value_name = "LIST_ID")]
    pub list_id: i64,
}

#[derive(Debug, Clone, Args)]
pub struct ListsUpdateArgs {
    #[arg(value_name = "LIST_ID")]
    pub list_id: i64,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "unarchive")]
    pub archive: bool,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "archive")]
    pub unarchive: bool,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "public")]
    pub private: bool,

    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "private")]
    pub public: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ListsShowArgs {
    #[arg(value_name = "LIST_ID")]
    pub list_id: i64,

    #[arg(long, action = ArgAction::SetTrue)]
    pub tasks: bool,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Status(AuthStatusArgs),
}

#[derive(Debug, Args)]
pub struct AuthStatusArgs {
    #[arg(long, value_enum, default_value = "text", value_name = "FORMAT")]
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
pub enum TasksCommand {
    /// Show tasks for a list
    Get(TasksGetArgs),
    /// Create a task in a list
    Create(TasksCreateArgs),
    /// Update a task in a list
    Update(TasksUpdateArgs),
    /// Remove a task from a list
    Remove(TasksRemoveArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TasksGetArgs {
    #[arg(long = "list-id", alias = "list", value_name = "LIST_ID")]
    pub list_id: i64,
}

#[derive(Debug, Clone, Args)]
pub struct TasksCreateArgs {
    #[arg(long = "list-id", alias = "list", value_name = "LIST_ID")]
    pub list_id: i64,

    #[arg(long, value_name = "CONTENT")]
    pub content: String,

    #[arg(long, value_name = "PARENT_ID")]
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Args)]
pub struct TasksUpdateArgs {
    #[arg(long = "list-id", alias = "list", value_name = "LIST_ID")]
    pub list_id: i64,

    #[arg(long, value_name = "TASK_ID")]
    pub task_id: i64,

    #[arg(long, value_name = "CONTENT")]
    pub content: Option<String>,

    #[arg(long, value_name = "STATUS", value_enum)]
    pub status: Option<TaskStatus>,

    #[arg(long, value_name = "PARENT_ID")]
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Args)]
pub struct TasksRemoveArgs {
    #[arg(long = "list-id", alias = "list", value_name = "LIST_ID")]
    pub list_id: i64,

    #[arg(long, value_name = "TASK_ID")]
    pub task_id: i64,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TaskStatus {
    Open,
    Done,
}
