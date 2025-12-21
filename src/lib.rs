pub mod api;
pub mod cfg;
pub mod cli;
pub mod error;
pub mod token_store;
pub mod commands;
pub mod output;

use crate::cli::Cli;
use crate::error::{AppError, ErrorKind};
use crate::commands::dispatch;
use clap::Parser;

pub fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
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
