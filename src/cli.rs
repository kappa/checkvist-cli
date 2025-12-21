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
    #[arg(long, default_value = "text", value_enum, value_name = "FORMAT")]
    pub format: OutputFormat,

    #[arg(long, default_value = "default", value_name = "PROFILE")]
    pub profile: String,

    #[arg(long, default_value = "https://checkvist.com", value_name = "BASE_URL")]
    pub base_url: String,

    #[arg(long, value_name = "AUTH_FILE")]
    pub auth_file: Option<PathBuf>,

    #[arg(long, value_name = "TOKEN_FILE")]
    pub token_file: Option<PathBuf>,

    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(subcommand)]
    Lists(ListsCommand),
    #[command(subcommand)]
    Auth(AuthCommand),
}

#[derive(Debug, Subcommand)]
pub enum ListsCommand {
    Get(ListsGetArgs),
}

#[derive(Debug, Args)]
pub struct ListsGetArgs {
    #[arg(long)]
    pub archived: bool,

    #[arg(long, value_name = "ORDER")]
    pub order: Option<String>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub skip_stats: bool,
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
