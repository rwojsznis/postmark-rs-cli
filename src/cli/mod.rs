//! The command line: the global flags, the command tree, and the one
//! dispatcher every leaf goes through (decision 11).

pub(crate) mod commands;
pub mod exit;
pub mod out;
pub(crate) mod page;

use crate::cli::commands::Leaf;
use crate::cli::commands::server::ServerCommand;
use crate::cli::commands::servers::ServersCommand;
use crate::cli::exit::ExitStatus;
use crate::client::{Executor, parse_base_url};
use crate::error::Error;
use crate::output::{self, Format};
use crate::paginate::{Paging, Plan, Stop, merged};
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
}

impl Command {
    fn leaf(&self) -> &dyn Leaf {
        match self {
            Command::Server(command) => command.leaf(),
            Command::Servers(command) => command.leaf(),
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
    let plan = leaf
        .paging()
        .filter(Paging::active)
        .and_then(|paging| Plan::new(leaf.spec(), paging));
    cli.global.execute(leaf.spec(), request, plan.as_ref())
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
