//! The one list of endpoint commands, and everything generated from it: each
//! leaf's `--help` footer and the `commands` catalog (decision 10). `args` in
//! the catalog come from clap's own `Command`, so they cannot drift from the
//! parser.

use crate::cli::commands::{
    bounces, clicks, inbound, opens, outbound, server, servers, streams, suppressions, templates,
};
use crate::spec::{EndpointSpec, Pagination};
use clap::Arg;
use serde_json::{Value, json};

type Entry = (&'static [&'static str], &'static EndpointSpec);

static REGISTRY: &[Entry] = &[
    (&["server", "get"], &server::GET),
    (&["servers", "list"], &servers::LIST),
    (&["servers", "get"], &servers::GET),
    (&["outbound", "search"], &outbound::SEARCH),
    (&["outbound", "get"], &outbound::GET),
    (&["outbound", "dump"], &outbound::DUMP),
    (&["inbound", "search"], &inbound::SEARCH),
    (&["inbound", "get"], &inbound::GET),
    (&["opens", "list"], &opens::LIST),
    (&["opens", "for-message"], &opens::FOR_MESSAGE),
    (&["clicks", "list"], &clicks::LIST),
    (&["clicks", "for-message"], &clicks::FOR_MESSAGE),
    (&["bounces", "delivery-stats"], &bounces::DELIVERY_STATS),
    (&["bounces", "list"], &bounces::LIST),
    (&["bounces", "get"], &bounces::GET),
    (&["bounces", "dump"], &bounces::DUMP),
    (&["templates", "list"], &templates::LIST),
    (&["templates", "get"], &templates::GET),
    (&["streams", "list"], &streams::LIST),
    (&["streams", "get"], &streams::GET),
    (&["suppressions", "dump"], &suppressions::DUMP),
];

/// Every endpoint command: its path in the command tree, and its spec.
#[must_use]
pub fn registry() -> &'static [Entry] {
    REGISTRY
}

/// The footer of a leaf's `--help`, from its spec.
pub(crate) fn leaf_help(spec: &EndpointSpec) -> String {
    let token = spec.token;
    let mut lines = vec![
        format!("Endpoint: GET {}", spec.path),
        format!(
            "Token:    {} ({}, env {})",
            token.as_str(),
            token.header(),
            token.env_var()
        ),
    ];
    match spec.pagination {
        Pagination::None => {}
        Pagination::OffsetCount {
            max_count,
            max_window: None,
        } => lines.push(format!(
            "Paging:   --count max {max_count}; --all to fetch every page"
        )),
        Pagination::OffsetCount {
            max_count,
            max_window: Some(window),
        } => lines.push(format!(
            "Paging:   --count max {max_count}, --count + --offset max {window}; --all to fetch every page"
        )),
        Pagination::Cursor { max_count } => lines.push(format!(
            "Paging:   --count max {max_count}; --all to follow PaginationKey"
        )),
    }
    lines.push(format!("Example:  {}", spec.example));
    lines.join("\n")
}

/// The `commands` output: `{"version", "global_args", "commands"}`.
pub(crate) fn catalog(version: &str) -> Value {
    let mut cmd = crate::cli::command();
    // Building propagates the global flags and settles each arg's value
    // parser, which `possible_values` reads.
    cmd.build();
    let global: Vec<Value> = cmd
        .get_arguments()
        .filter(|arg| arg.is_global_set())
        .map(arg_json)
        .collect();
    let commands: Vec<Value> = REGISTRY
        .iter()
        .map(|(path, spec)| {
            let leaf = path
                .iter()
                .try_fold(&cmd, |cmd, name| cmd.find_subcommand(name));
            let args: Vec<Value> = leaf
                .into_iter()
                .flat_map(clap::Command::get_arguments)
                .filter(|arg| !arg.is_global_set() && !is_builtin(arg))
                .map(arg_json)
                .collect();
            json!({
                "command": path.join(" "),
                "summary": spec.summary,
                "method": "GET",
                "path": spec.path,
                "token": spec.token.as_str(),
                "pagination": pagination_json(spec.pagination),
                "list_key": spec.list_key,
                "example": spec.example,
                "args": args,
            })
        })
        .collect();
    json!({"version": version, "global_args": global, "commands": commands})
}

/// `--help` and `--version`: clap adds them to every command.
fn is_builtin(arg: &Arg) -> bool {
    matches!(arg.get_id().as_str(), "help" | "version")
}

fn arg_json(arg: &Arg) -> Value {
    let help = arg.get_long_help().or_else(|| arg.get_help());
    let possible: Vec<String> = arg
        .get_possible_values()
        .iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect();
    json!({
        "id": arg.get_id().as_str(),
        "long": arg.get_long().map(|long| format!("--{long}")),
        "positional": arg.is_positional(),
        "required": arg.is_required_set(),
        "takes_value": arg.get_action().takes_values(),
        "help": help.map(ToString::to_string),
        "possible_values": possible,
        "default": arg.get_default_values().first().map(|v| v.to_string_lossy()),
        "env": arg.get_env().map(|env| env.to_string_lossy()),
    })
}

fn pagination_json(pagination: Pagination) -> Value {
    match pagination {
        Pagination::None => json!({"type": "none"}),
        Pagination::OffsetCount {
            max_count,
            max_window: None,
        } => json!({"type": "offset", "max_count": max_count}),
        Pagination::OffsetCount {
            max_count,
            max_window: Some(window),
        } => json!({"type": "offset", "max_count": max_count, "max_window": window}),
        Pagination::Cursor { max_count } => json!({"type": "cursor", "max_count": max_count}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::TokenKind;

    static SPEC: EndpointSpec = EndpointSpec {
        path: "/bounces",
        token: TokenKind::Server,
        summary: "",
        pagination: Pagination::OffsetCount {
            max_count: 500,
            max_window: Some(10_000),
        },
        list_key: Some("Bounces"),
        example: "postmark-cli bounces list",
    };

    #[test]
    fn the_help_footer_names_the_endpoint_token_paging_and_example() {
        assert_eq!(
            leaf_help(&SPEC),
            "Endpoint: GET /bounces\n\
             Token:    server (X-Postmark-Server-Token, env POSTMARK_SERVER_TOKEN)\n\
             Paging:   --count max 500, --count + --offset max 10000; --all to fetch every page\n\
             Example:  postmark-cli bounces list"
        );
        let cursor = EndpointSpec {
            pagination: Pagination::Cursor { max_count: 500 },
            ..SPEC
        };
        assert!(
            leaf_help(&cursor)
                .contains("Paging:   --count max 500; --all to follow PaginationKey\n")
        );
        let plain = EndpointSpec {
            pagination: Pagination::None,
            ..SPEC
        };
        assert!(!leaf_help(&plain).contains("Paging:"));
    }

    #[test]
    fn pagination_is_described_with_its_limits() {
        assert_eq!(pagination_json(Pagination::None), json!({"type": "none"}));
        assert_eq!(
            pagination_json(SPEC.pagination),
            json!({"type": "offset", "max_count": 500, "max_window": 10_000})
        );
        assert_eq!(
            pagination_json(Pagination::Cursor { max_count: 500 }),
            json!({"type": "cursor", "max_count": 500})
        );
    }
}
