//! The shared harness: run the real binary against a mock Postmark.

// Each test file compiles this module on its own and uses part of it, so the
// rest is dead code in that file.
#![allow(dead_code)]

use httptest::matchers::{all_of, contains, eq, key, not, request};
use httptest::{Expectation, ExpectationBuilder, Server};
use std::process::Command;

/// A token value that must never appear in any output (decision 6).
pub(crate) const SENTINEL_TOKEN: &str = "sentinel-token-7f3a";

/// The binary, with every `POSTMARK_*` variable removed so that the
/// developer's own tokens cannot leak into a test.
///
/// Never `env_clear()`: it removes `LLVM_PROFILE_FILE`, and cargo-llvm-cov
/// then loses the coverage of the spawned binary (decision 12).
pub(crate) fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_postmark-cli"));
    for var in [
        "POSTMARK_SERVER_TOKEN",
        "POSTMARK_ACCOUNT_TOKEN",
        "POSTMARK_BASE_URL",
    ] {
        cmd.env_remove(var);
    }
    cmd
}

#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Run {
    /// stdout as one JSON value.
    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {:?}", self.stdout))
    }

    /// Every stderr line as JSON. Decision 5: errors, warnings and the
    /// `--verbose` lines are all one JSON value per line.
    pub(crate) fn stderr_lines(&self) -> Vec<serde_json::Value> {
        self.stderr
            .lines()
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|e| panic!("stderr line is not JSON ({e}): {line:?}"))
            })
            .collect()
    }

    /// The `error` object of the one error line, which comes last.
    pub(crate) fn error(&self) -> serde_json::Value {
        let lines = self.stderr_lines();
        let last = lines.last().expect("an error line on stderr");
        last.get("error")
            .unwrap_or_else(|| panic!("the last stderr line is not an error: {last}"))
            .clone()
    }
}

pub(crate) fn run(args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut cmd = cli();
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("the binary runs");
    Run {
        code: out.status.code().expect("an exit code, not a signal"),
        stdout: String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    }
}

/// Asserts that the sentinel token is in neither stream.
pub(crate) fn assert_no_secret(run: &Run) {
    assert!(
        !run.stdout.contains(SENTINEL_TOKEN),
        "stdout has the token: {}",
        run.stdout
    );
    assert!(
        !run.stderr.contains(SENTINEL_TOKEN),
        "stderr has the token: {}",
        run.stderr
    );
}

/// The `--base-url` for a mock server.
pub(crate) fn base_url(server: &Server) -> String {
    server.url_str("/")
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Token {
    Server,
    Account,
}

impl Token {
    fn header(self) -> &'static str {
        match self {
            Token::Server => "x-postmark-server-token",
            Token::Account => "x-postmark-account-token",
        }
    }

    fn other(self) -> Token {
        match self {
            Token::Server => Token::Account,
            Token::Account => Token::Server,
        }
    }
}

/// Expects one GET on `path` with exactly `query` (encoded, as on the wire;
/// `""` for none), the sentinel in `token`'s header, `Accept:
/// application/json`, and no header for the other token.
pub(crate) fn expect_get(path: &str, query: &str, token: Token) -> ExpectationBuilder {
    Expectation::matching(all_of![
        request::method_path("GET", path.to_owned()),
        request::query(eq(query.to_owned())),
        request::headers(contains((token.header(), SENTINEL_TOKEN))),
        request::headers(contains(("accept", "application/json"))),
        request::headers(not(contains(key(token.other().header())))),
    ])
}
