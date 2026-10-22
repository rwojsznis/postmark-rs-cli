//! The command line: the global flags, the command tree, and the one
//! dispatcher every leaf goes through (decision 11).

pub(crate) mod args;
pub mod catalog;
pub(crate) mod commands;
pub mod exit;
pub mod out;
pub(crate) mod page;

use crate::cli::commands::Leaf;
use crate::cli::commands::clicks::ClicksCommand;
use crate::cli::commands::inbound::InboundCommand;
use crate::cli::commands::opens::OpensCommand;
use crate::cli::commands::outbound::OutboundCommand;
use crate::cli::commands::server::ServerCommand;
use crate::cli::commands::servers::ServersCommand;
use crate::cli::exit::ExitStatus;
use crate::client::{Executor, parse_base_url};
use crate::error::Error;
use crate::output::{self, Format};
use crate::paginate::{Paging, Plan, Stop, merged};
use crate::request::GetRequest;
use crate::spec::EndpointSpec;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use serde_json::json;
use std::time::Duration;

const VERSION: &str = match option_env!("POSTMARK_CLI_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

const AFTER_HELP: &str = "\
Read-only: every command sends GET requests only (one, or one per page with
--all). No command sends email or changes anything, so an agent can run any
of them without asking first.

Environment:
  POSTMARK_SERVER_TOKEN   the server token (--server-token)
  POSTMARK_ACCOUNT_TOKEN  the account token (--account-token)
  POSTMARK_BASE_URL       the API root (--base-url)

Exit codes:
  0  success, also when stdout closes early
  1  Postmark answered with an error status (error kind \"api\")
  2  usage or configuration error; nothing was sent (\"config\")
  3  no usable answer: connection, TLS, timeout, or a body that is not JSON
     (\"transport\", \"timeout\", \"decode\")

stdout is Postmark's JSON, unchanged. Errors and warnings are one JSON line
each on stderr. Agents: --format jsonl prints one list item per line.

Run `postmark-cli commands` for a JSON catalog of every command.";

// No `Debug`: `Global` holds the tokens, and a derived `Debug` would print
// them (decision 6).
#[derive(Parser)]
#[command(
    name = "postmark-cli",
    about = "Unofficial, read-only Postmark API client for LLM agents.",
    after_help = AFTER_HELP,
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
#[command(next_help_heading = "Global options")]
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
    /// The servers of the account (account token).
    #[command(subcommand)]
    Servers(ServersCommand),
    /// Sent messages: search, details, raw source.
    #[command(subcommand)]
    Outbound(OutboundCommand),
    /// Received messages: search, details.
    #[command(subcommand)]
    Inbound(InboundCommand),
    /// Opens of sent messages: who, with which client, where.
    #[command(subcommand)]
    Opens(OpensCommand),
    /// Link clicks in sent messages: who, with which client, where.
    #[command(subcommand)]
    Clicks(ClicksCommand),
    /// Describe every command as JSON: endpoint, token, paging and flags.
    /// Needs no network and no token.
    Commands,
}

impl Command {
    /// The endpoint command, or `None` for `commands`.
    fn leaf(&self) -> Option<&dyn Leaf> {
        match self {
            Command::Server(command) => Some(command.leaf()),
            Command::Servers(command) => Some(command.leaf()),
            Command::Outbound(command) => Some(command.leaf()),
            Command::Inbound(command) => Some(command.leaf()),
            Command::Opens(command) => Some(command.leaf()),
            Command::Clicks(command) => Some(command.leaf()),
            Command::Commands => None,
        }
    }
}

/// The clap command, with each leaf's `--help` footer generated from its spec
/// (decision 10), so the help cannot drift from the code.
#[must_use]
pub fn command() -> clap::Command {
    let mut cmd = Cli::command();
    for (path, spec) in catalog::registry() {
        with_leaf_help(&mut cmd, path, catalog::leaf_help(spec));
    }
    cmd
}

fn with_leaf_help(cmd: &mut clap::Command, path: &[&str], help: String) {
    match path.split_first() {
        None => *cmd = std::mem::take(cmd).after_help(help),
        Some((name, rest)) => {
            if let Some(sub) = cmd.find_subcommand_mut(name) {
                with_leaf_help(sub, rest, help);
            }
        }
    }
}

/// The spec and request a command line would send, without sending it. For
/// the parity tests against postmark-rs.
///
/// # Errors
///
/// A `config` error when the line does not parse, names `commands`, or its
/// request cannot be built.
pub fn parse_request(argv: &[&str]) -> Result<(&'static EndpointSpec, GetRequest), Error> {
    let matches = command()
        .try_get_matches_from(argv)
        .map_err(|e| Error::config(e.to_string()))?;
    let cli = Cli::from_arg_matches(&matches).map_err(|e| Error::config(e.to_string()))?;
    let leaf = cli
        .command
        .leaf()
        .ok_or_else(|| Error::config("`commands` calls no endpoint"))?;
    Ok((leaf.spec(), leaf.request()?))
}

/// Parses the command line and runs the one command it names.
///
/// clap exits by itself on `--help`, `--version` and a usage error.
///
/// # Errors
///
/// Any error of decision 5. `main` prints it and exits with its code.
pub fn run() -> Result<ExitStatus, Error> {
    let matches = command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let Some(leaf) = cli.command.leaf() else {
        print_catalog(cli.global.format);
        return Ok(ExitStatus::Success);
    };
    let request = leaf.request()?;
    let plan = leaf
        .paging()
        .filter(Paging::active)
        .and_then(|paging| Plan::new(leaf.spec(), paging));
    cli.global.execute(leaf.spec(), request, plan.as_ref())
}

/// `commands`: json prints the whole catalog; jsonl one command per line.
fn print_catalog(format: Format) {
    output::print_response(&catalog::catalog(VERSION), Some("commands"), format);
}

impl Global {
    fn execute(
        self,
        spec: &EndpointSpec,
        request: GetRequest,
        plan: Option<&Plan>,
    ) -> Result<ExitStatus, Error> {
        let base_url = parse_base_url(&self.base_url)?;
        if self.dry_run {
            let request = plan.map_or(request.clone(), |plan| plan.first_request(&request));
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
        if let Some(plan) = plan {
            runtime.block_on(all_pages(&executor, spec, plan, &request, self.format))?;
        } else {
            let value = runtime.block_on(executor.get(spec, request))?;
            output::print_response(&value, spec.list_key, self.format);
        }
        Ok(ExitStatus::Success)
    }
}

/// `--all` and `--limit` (decision 9): jsonl streams each page's items as it
/// arrives; json prints one merged response at the end.
async fn all_pages(
    executor: &Executor,
    spec: &EndpointSpec,
    plan: &Plan,
    base: &GetRequest,
    format: Format,
) -> Result<(), Error> {
    let fetch = |request| executor.get(spec, request);
    let key = plan.list_key();
    match format {
        Format::Jsonl => {
            let paged = plan
                .run(base, fetch, |item| output::print(item, Format::Jsonl))
                .await?;
            warn_stop(&paged.stop, plan);
            if let Stop::NoList(body) = paged.stop {
                output::print(&body, Format::Jsonl);
            }
        }
        Format::Json => {
            let mut items = Vec::new();
            let paged = plan
                .run(base, fetch, |item| {
                    items.push(item.clone());
                    true
                })
                .await?;
            warn_stop(&paged.stop, plan);
            let body = match (paged.first, paged.stop) {
                // The very first page had no list: print it as it came.
                (None, Stop::NoList(body)) => body,
                (first, _) => merged(first.unwrap_or_default(), key, items),
            };
            output::print(&body, Format::Json);
        }
    }
    Ok(())
}

fn warn_stop(stop: &Stop, plan: &Plan) {
    match stop {
        Stop::Ceiling => output::warn(
            "pagination_ceiling",
            &format!(
                "stopped at the {} items Postmark lets this search reach (count + offset); \
                 narrow the search (with --from-date and --to-date, where the command has \
                 them) to see the rest",
                plan.window().unwrap_or_default()
            ),
        ),
        Stop::NoList(_) => output::warn_list_key_missing(plan.list_key()),
        Stop::Done | Stop::Limit | Stop::Closed => {}
    }
}
