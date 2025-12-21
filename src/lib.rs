pub mod api;
pub mod cfg;
pub mod cli;
pub mod commands;
pub mod error;
pub mod log;
pub mod output;
pub mod token_store;

use crate::commands::dispatch;
use crate::error::{AppError, ErrorKind};

pub fn run() -> Result<(), AppError> {
    let cli = match cli::parse_from_env()? {
        cli::ParseOutcome::Cli(cli) => cli,
        cli::ParseOutcome::Help => return Ok(()),
    };
    log::init(cli.verbose);
    dispatch(cli)
}

pub fn exit_code(kind: ErrorKind) -> i32 {
    match kind {
        ErrorKind::Argument => 2,
        ErrorKind::Auth => 3,
        ErrorKind::Network => 4,
        ErrorKind::ApiData => 5,
        ErrorKind::Local => 6,
    }
}
