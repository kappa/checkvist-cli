pub mod api;
pub mod cfg;
pub mod cli;
pub mod error;
pub mod token_store;

use crate::cli::Cli;
use crate::error::{AppError, ErrorKind};
use clap::Parser;

pub fn run() -> Result<(), AppError> {
    let _cli = Cli::parse();
    // Milestone M2: parsing only; command execution will be added in later milestones.
    Ok(())
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
