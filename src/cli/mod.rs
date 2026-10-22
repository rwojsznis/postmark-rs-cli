//! The command line: the global flags, the command tree, and the one
//! dispatcher every leaf goes through (decision 11).

pub(crate) mod commands;
pub mod exit;
pub mod out;

use crate::cli::commands::Leaf;
use crate::cli::commands::server::ServerCommand;
use crate::cli::exit::ExitStatus;
use crate::client::{Executor, parse_base_url};
use crate::error::Error;
use crate::output::{self, Format};
use crate::request::GetRequest;
use crate::spec::EndpointSpec;
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use std::time::Duration;

const VERSION: &str = match option_env!("POSTMARK_CLI_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

// No `Debug`: `Global` holds the tokens, and a derived `Debug` would print
// them (decision 6).
#[derive(Parser)]
#[command(
    name = "postmark-cli",
    about = "Unofficial, read-only Postmark API client for LLM agents.",
    version = VERSION
)]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

/// The flags of decision 7. `global = true`, so they work before or after
/// the subcommand.
#[derive(Args)]
struct Global {
    /// The server token, for server-level endpoints.
    #[arg(
        long,
        env = "POSTMARK_SERVER_TOKEN",
        hide_env_values = true,
        global = true,
        value_name = "TOKEN"
    )]
    server_token: Option<String>,

    /// The account token, for servers, domains, sender signatures and data
    /// removals.
    #[arg(
        long,
        env = "POSTMARK_ACCOUNT_TOKEN",
        hide_env_values = true,
        global = true,
        value_name = "TOKEN"
    )]
    account_token: Option<String>,

    /// The API root. Must be http or https.
    // `hide_env_values` only for the help text: without it, clap prints
    // `[env: POSTMARK_BASE_URL=]` when the variable is unset.
    #[arg(
        long,
        env = "POSTMARK_BASE_URL",
        hide_env_values = true,
        default_value = postmark::POSTMARK_API_URL,
        global = true,
        value_name = "URL"
    )]
    base_url: String,

    /// Seconds to wait for each response.
    #[arg(
        long,
        default_value_t = 30,
        value_parser = clap::value_parser!(u64).range(1..=3600),
        global = true,
        value_name = "SECONDS"
    )]
    timeout: u64,

    /// The stdout layout.
    #[arg(long, value_enum, default_value_t = Format::Json, global = true)]
    format: Format,

    /// Print the request instead of sending it. Needs no token.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Log each request and its outcome on stderr, as JSON lines.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// The server that owns the server token.
    #[command(subcommand)]
    Server(ServerCommand),
}

impl Command {
    fn leaf(&self) -> &dyn Leaf {
        match self {
            Command::Server(command) => command.leaf(),
        }
    }
}

/// Parses the command line and runs the one command it names.
///
/// clap exits by itself on `--help`, `--version` and a usage error.
///
/// # Errors
///
/// Any error of decision 5. `main` prints it and exits with its code.
pub fn run() -> Result<ExitStatus, Error> {
    let cli = Cli::parse();
    let leaf = cli.command.leaf();
    let request = leaf.request()?;
    cli.global.execute(leaf.spec(), request)
}

impl Global {
    fn execute(self, spec: &EndpointSpec, request: GetRequest) -> Result<ExitStatus, Error> {
        let base_url = parse_base_url(&self.base_url)?;
        if self.dry_run {
            let url = request
                .url(&base_url)
                .map_err(|e| Error::config(format!("cannot build the request URL: {e}")))?;
            output::print(
                &json!({"method": "GET", "url": url.as_str(), "token": spec.token.as_str()}),
                self.format,
            );
            return Ok(ExitStatus::Success);
        }
        let executor = Executor {
            base_url,
            timeout: Duration::from_secs(self.timeout),
            server_token: self.server_token,
            account_token: self.account_token,
            verbose: self.verbose,
        };
        // One current-thread runtime: postmark-rs is async, and nothing here
        // runs concurrently (decision 14).
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::transport(format!("cannot start the async runtime: {e}")))?;
        let value = runtime.block_on(executor.get(spec, request))?;
        output::print(&value, self.format);
        Ok(ExitStatus::Success)
    }
}
