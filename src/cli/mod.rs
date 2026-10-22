//! The command line: the flags, the dispatch, and the printing macros.

pub mod exit;
pub mod out;

use crate::cli::exit::ExitStatus;
use clap::Parser;

const VERSION: &str = match option_env!("POSTMARK_CLI_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

// `arg_required_else_help` until the subcommands exist: with no arguments,
// clap then prints the help and exits 2, as it will for a missing subcommand.
#[derive(Debug, Parser)]
#[command(
    name = "postmark-cli",
    about = "Unofficial, read-only Postmark API client for LLM agents.",
    version = VERSION,
    arg_required_else_help = true
)]
struct Cli {}

/// Parses the command line and runs the one command it names.
///
/// clap exits by itself on `--help`, `--version` and a usage error.
#[must_use]
pub fn run() -> ExitStatus {
    let _cli = Cli::parse();
    ExitStatus::Success
}
