//! The `postmark-cli` binary.
//!
//! Everything is in the library, under `cli`: the flags, the dispatch, the exit
//! codes and the four printing macros. This file only turns the result into the
//! process exit code.

// A panicking library is a bug: this crate returns `Result` everywhere, and an
// agent reads the one JSON error line on stderr, not a backtrace. Declared here
// rather than in `Cargo.toml`, because a manifest `[lints]` table also covers
// `tests/`, where a panic *is* the failure report. `clippy.toml` exempts the
// unit tests in `src/`.
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::ExitCode;

fn main() -> ExitCode {
    postmark_cli::cli::run().into()
}
