pub mod api;
pub mod cfg;
pub mod cli;
pub mod commands;
pub mod error;
pub mod log;
pub mod output;
pub mod token_store;

use crate::cli::Cli;
use crate::commands::dispatch;
use crate::error::{AppError, ErrorKind};
use clap::Parser;

pub fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
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
