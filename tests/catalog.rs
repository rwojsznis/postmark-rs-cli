//! The registry, the clap tree, the fixture of all 44 GET endpoints, and
//! `docs/endpoints.md` must agree (decisions 10, 11 and 12).

mod common;

use common::run;
use postmark_cli::cli::catalog::registry;
use postmark_cli::cli::{command, parse_request};
use postmark_cli::spec::{EndpointSpec, Pagination};
use std::collections::BTreeSet;
use std::fmt::Write;

/// The endpoints that have no command yet. Each endpoint task removes its
/// paths; the list is empty once every GET endpoint has its command.
const NOT_YET_IMPLEMENTED: &[&str] = &[
    "/data-removals/{id}",
    "/domains",
    "/domains/{domainid}",
    "/email/bulk",
    "/email/bulk/{bulk-request-id}",
    "/message-streams",
    "/message-streams/{stream_ID}",
    "/message-streams/{stream_id}/suppressions/dump",
    "/senders",
    "/senders/{signatureid}",
    "/stats/outbound",
    "/stats/outbound/bounces",
    "/stats/outbound/clicks",
    "/stats/outbound/clicks/browserfamilies",
    "/stats/outbound/clicks/location",
    "/stats/outbound/clicks/platforms",
    "/stats/outbound/opens",
    "/stats/outbound/opens/emailclients",
    "/stats/outbound/opens/platforms",
    "/stats/outbound/sends",
    "/stats/outbound/spam",
    "/stats/outbound/tracked",
    "/templates",
    "/templates/{templateIdOrAlias}",
    "/triggers/inboundrules",
    "/webhooks",
    "/webhooks/{Id}",
    "/webhooks/{Id}/statistics",
];

/// Every leaf of the clap tree, as its command path.
fn leaves(cmd: &clap::Command, prefix: &[String], out: &mut Vec<Vec<String>>) {
    for sub in cmd.get_subcommands() {
        let mut path = prefix.to_vec();
        path.push(sub.get_name().to_owned());
        if sub.has_subcommands() {
            leaves(sub, &path, out);
        } else {
            out.push(path);
        }
    }
}

fn leaf<'a>(cmd: &'a clap::Command, path: &[&str]) -> &'a clap::Command {
    path.iter().fold(cmd, |cmd, name| {
        cmd.find_subcommand(name)
            .unwrap_or_else(|| panic!("no subcommand {name}"))
    })
}

#[test]
fn every_leaf_is_in_the_registry_and_every_entry_is_a_leaf() {
    let mut tree = Vec::new();
    leaves(&command(), &[], &mut tree);
    let tree: BTreeSet<Vec<String>> = tree.into_iter().filter(|p| p != &["commands"]).collect();
    let registered: BTreeSet<Vec<String>> = registry()
        .iter()
        .map(|(path, _)| path.iter().map(|s| (*s).to_owned()).collect())
        .collect();
    assert_eq!(tree, registered);
}

#[test]
fn no_path_and_no_command_is_registered_twice() {
    let paths: BTreeSet<_> = registry().iter().map(|(_, spec)| spec.path).collect();
    let names: BTreeSet<_> = registry().iter().map(|(name, _)| *name).collect();
    assert_eq!(paths.len(), registry().len());
    assert_eq!(names.len(), registry().len());
}

#[test]
fn the_registry_and_the_not_yet_list_together_are_the_44_endpoints() {
    let fixture: BTreeSet<&str> = include_str!("fixtures/get-endpoints.txt").lines().collect();
    assert_eq!(fixture.len(), 44);
    let registered: BTreeSet<&str> = registry().iter().map(|(_, spec)| spec.path).collect();
    let pending: BTreeSet<&str> = NOT_YET_IMPLEMENTED.iter().copied().collect();
    assert_eq!(
        pending.len(),
        NOT_YET_IMPLEMENTED.len(),
        "a duplicate in NOT_YET_IMPLEMENTED"
    );
    assert_eq!(
        registered.intersection(&pending).collect::<Vec<_>>(),
        Vec::<&&str>::new(),
        "implemented, so remove it from NOT_YET_IMPLEMENTED"
    );
    assert_eq!(
        registered.union(&pending).copied().collect::<BTreeSet<_>>(),
        fixture
    );
}

#[test]
fn each_spec_has_one_positional_per_placeholder() {
    let cmd = command();
    for (path, spec) in registry() {
        let positionals = leaf(&cmd, path)
            .get_arguments()
            .filter(|a| a.is_positional())
            .count();
        assert_eq!(positionals, spec.placeholders().len(), "{path:?}");
    }
}

#[test]
fn each_example_parses_into_its_own_command() {
    for (path, spec) in registry() {
        let words: Vec<&str> = spec.example.split_whitespace().collect();
        assert_eq!(words[0], "postmark-cli", "{path:?}");
        assert_eq!(&words[1..=path.len()], *path, "{path:?}: {}", spec.example);
        let (parsed, _) = parse_request(&words).unwrap_or_else(|e| {
            panic!("{path:?}: {} does not parse: {}", spec.example, e.message())
        });
        assert!(
            std::ptr::eq(parsed, *spec),
            "{path:?} parsed into {}",
            parsed.path
        );
    }
}

#[test]
fn each_paginated_spec_has_a_list_key() {
    for (path, spec) in registry() {
        if spec.pagination != Pagination::None {
            assert!(spec.list_key.is_some(), "{path:?}");
        }
    }
}

#[test]
fn each_leaf_help_names_its_endpoint_token_and_example() {
    for (path, spec) in registry() {
        let mut args = path.to_vec();
        args.push("--help");
        let out = run(&args, &[]);
        assert_eq!(out.code, 0, "{path:?}");
        assert!(
            out.stdout.contains(&format!("Endpoint: GET {}", spec.path)),
            "{path:?}:\n{}",
            out.stdout
        );
        assert!(out.stdout.contains("Token:"), "{path:?}");
        assert!(out.stdout.contains(spec.example), "{path:?}");
    }
}

#[test]
fn top_level_help_states_the_contract() {
    let out = run(&["--help"], &[]);
    assert_eq!(out.code, 0);
    for needle in [
        "read-only",
        "GET",
        "POSTMARK_SERVER_TOKEN",
        "POSTMARK_ACCOUNT_TOKEN",
        "POSTMARK_BASE_URL",
        "jsonl",
        "postmark-cli commands",
    ] {
        assert!(out.stdout.contains(needle), "{needle}:\n{}", out.stdout);
    }
    for code in ["0", "1", "2", "3"] {
        assert!(
            out.stdout.lines().any(|l| l.trim_start().starts_with(code)),
            "exit code {code}:\n{}",
            out.stdout
        );
    }
}

#[test]
fn the_catalog_lists_every_command_as_get() {
    let out = run(&["commands"], &[]);
    assert_eq!(out.code, 0, "{out:?}");
    let catalog = out.json();
    let keys: Vec<_> = catalog.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["version", "global_args", "commands"]);
    assert_eq!(catalog["version"], env!("CARGO_PKG_VERSION"));

    let commands = catalog["commands"].as_array().unwrap();
    assert_eq!(commands.len(), registry().len());
    for (entry, (path, spec)) in commands.iter().zip(registry()) {
        assert_eq!(entry["command"], path.join(" "));
        // Decision 1, asserted for every command.
        assert_eq!(entry["method"], "GET", "{entry}");
        assert_eq!(entry["path"], spec.path);
        assert_eq!(entry["token"], spec.token.as_str());
        assert_eq!(entry["summary"], spec.summary);
        assert_eq!(entry["example"], spec.example);
    }

    let global: Vec<_> = catalog["global_args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["long"].as_str().unwrap().to_owned())
        .collect();
    assert!(global.contains(&"--server-token".to_owned()), "{global:?}");
    assert!(global.contains(&"--format".to_owned()), "{global:?}");
    // Listed once, at the top, not per command.
    for entry in commands {
        for arg in entry["args"].as_array().unwrap() {
            assert_ne!(arg["long"], "--server-token", "{entry}");
        }
    }
}

#[test]
fn the_catalog_args_come_from_clap() {
    let catalog = run(&["commands"], &[]).json();
    let entry = catalog["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["command"] == "servers list")
        .unwrap()
        .clone();
    assert_eq!(
        entry["pagination"],
        serde_json::json!({"type": "offset", "max_count": 500})
    );
    assert_eq!(entry["list_key"], "Servers");
    let name = entry["args"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "name")
        .unwrap();
    assert_eq!(name["long"], "--name");
    assert_eq!(name["positional"], false);
    assert_eq!(name["required"], false);
    assert_eq!(name["takes_value"], true);
    assert!(name["help"].as_str().unwrap().contains("`name`"));

    let get = catalog["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["command"] == "servers get")
        .unwrap()
        .clone();
    let id = &get["args"][0];
    assert_eq!(id["positional"], true);
    assert_eq!(id["required"], true);
    assert_eq!(id["long"], serde_json::Value::Null);

    let format = catalog["global_args"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "format")
        .unwrap()
        .clone();
    assert_eq!(
        format["possible_values"],
        serde_json::json!(["json", "jsonl"])
    );
    assert_eq!(format["default"], "json");
    let token = catalog["global_args"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "server_token")
        .unwrap()
        .clone();
    assert_eq!(token["env"], "POSTMARK_SERVER_TOKEN");
}

#[test]
fn the_catalog_in_jsonl_is_one_line_per_command() {
    let out = run(&["commands", "--format", "jsonl"], &[]);
    assert_eq!(out.code, 0);
    let lines: Vec<serde_json::Value> = out
        .stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), registry().len());
    assert!(lines.iter().all(|l| l["method"] == "GET"));
}

#[test]
fn the_catalog_needs_no_token_and_no_network() {
    // An unusable base URL would fail any request.
    let out = run(&["commands", "--base-url", "not a url"], &[]);
    assert_eq!(out.code, 0, "{out:?}");
}

/// postmark-rs's request type per path, and a note for the table.
const POSTMARK_RS: &[(&str, &str, &str)] = &[
    (
        "/bounces",
        "ListBouncesWithFiltersRequest",
        "Window: count + offset ≤ 10,000.",
    ),
    ("/bounces/{bounceid}", "GetBounceRequest", ""),
    (
        "/bounces/{bounceid}/dump",
        "GetBounceDumpRequest",
        "Dumps are kept for 30 days.",
    ),
    (
        "/data-removals/{id}",
        "GetDataRemovalStatusRequest",
        "Available on request only.",
    ),
    ("/deliverystats", "GetDeliveryStatsRequest", ""),
    ("/domains", "ListDomainsRequest", ""),
    ("/domains/{domainid}", "GetDomainRequest", ""),
    ("/email/bulk", "", "Cursor pagination with `PaginationKey`."),
    ("/email/bulk/{bulk-request-id}", "GetBulkStatusRequest", ""),
    (
        "/message-streams",
        "ListMessageStreamsRequest",
        "Not paginated.",
    ),
    (
        "/message-streams/{stream_ID}",
        "GetMessageStreamRequest",
        "",
    ),
    (
        "/message-streams/{stream_id}/suppressions/dump",
        "GetSuppressionRequest",
        "Not paginated.",
    ),
    (
        "/messages/inbound",
        "InboundSearchRequest",
        "Window: count + offset ≤ 10,000. Default `status=processed`.",
    ),
    (
        "/messages/inbound/{messageid}/details",
        "InboundDetailsRequest",
        "",
    ),
    (
        "/messages/outbound",
        "OutboundSearchRequest",
        "Window: count + offset ≤ 10,000.",
    ),
    (
        "/messages/outbound/{messageid}/details",
        "OutboundDetailsRequest",
        "",
    ),
    (
        "/messages/outbound/{messageid}/dump",
        "OutboundDumpRequest",
        "",
    ),
    ("/messages/outbound/clicks", "MessageClicksRequest", ""),
    (
        "/messages/outbound/clicks/{messageid}",
        "SingleMessageClicksRequest",
        "",
    ),
    (
        "/messages/outbound/opens",
        "MessageOpensRequest",
        "Window: count + offset ≤ 10,000.",
    ),
    (
        "/messages/outbound/opens/{messageid}",
        "SingleMessageOpensRequest",
        "",
    ),
    ("/senders", "ListSignaturesRequest", ""),
    ("/senders/{signatureid}", "GetSignatureRequest", ""),
    (
        "/server",
        "GetCurrentServerRequest",
        "The server that owns the token.",
    ),
    (
        "/servers",
        "ListServersRequest",
        "postmark-rs sends no `name`.",
    ),
    ("/servers/{serverid}", "GetServerRequest", ""),
    ("/stats/outbound", "GetOutboundOverviewRequest", ""),
    ("/stats/outbound/bounces", "GetBounceCountsRequest", ""),
    ("/stats/outbound/clicks", "GetClickCountsRequest", ""),
    (
        "/stats/outbound/clicks/browserfamilies",
        "GetBrowserUsageRequest",
        "",
    ),
    (
        "/stats/outbound/clicks/location",
        "GetClickLocationRequest",
        "",
    ),
    (
        "/stats/outbound/clicks/platforms",
        "GetBrowserPlatformUsageRequest",
        "",
    ),
    ("/stats/outbound/opens", "GetEmailOpenCountsRequest", ""),
    (
        "/stats/outbound/opens/emailclients",
        "GetEmailClientUsageRequest",
        "",
    ),
    (
        "/stats/outbound/opens/platforms",
        "GetEmailPlatformUsageRequest",
        "",
    ),
    ("/stats/outbound/sends", "GetSentCountsRequest", ""),
    ("/stats/outbound/spam", "GetSpamComplaintsRequest", ""),
    (
        "/stats/outbound/tracked",
        "GetTrackedEmailCountsRequest",
        "",
    ),
    ("/templates", "ListTemplatesRequest", ""),
    (
        "/templates/{templateIdOrAlias}",
        "GetTemplateRequest",
        "An ID or an alias.",
    ),
    (
        "/triggers/inboundrules",
        "ListInboundRuleTriggersRequest",
        "",
    ),
    ("/webhooks", "ListWebhooksRequest", "Not paginated."),
    ("/webhooks/{Id}", "GetWebhookRequest", ""),
    ("/webhooks/{Id}/statistics", "", "The last 24 hours."),
];

fn pagination(spec: &EndpointSpec) -> String {
    match spec.pagination {
        Pagination::None => "none".to_owned(),
        Pagination::OffsetCount {
            max_count,
            max_window: None,
        } => format!("offset, max {max_count}"),
        Pagination::OffsetCount {
            max_count,
            max_window: Some(window),
        } => format!("offset, max {max_count}, window {window}"),
        Pagination::Cursor { max_count } => format!("cursor, max {max_count}"),
    }
}

fn endpoints_table() -> String {
    let mut table = String::from(
        "# Endpoints\n\n\
         Every command, the endpoint it calls, and the token it sends. All are GET\n\
         (decision 1). `tests/catalog.rs` generates this table from the registry;\n\
         after a change, run `UPDATE_DOCS=1 cargo test --test catalog docs_endpoints`\n\
         and review the diff.\n\n\
         | Command | Method | Path | Token | Pagination | List key | postmark-rs | Notes |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for (path, spec) in registry() {
        let (_, theirs, notes) = POSTMARK_RS
            .iter()
            .find(|(p, _, _)| *p == spec.path)
            .unwrap_or_else(|| panic!("add {} to POSTMARK_RS", spec.path));
        let theirs = if theirs.is_empty() {
            "not in postmark-rs".to_owned()
        } else {
            format!("`{theirs}`")
        };
        writeln!(
            table,
            "| `{}` | GET | `{}` | {} | {} | {} | {} | {} |",
            path.join(" "),
            spec.path,
            spec.token.as_str(),
            pagination(spec),
            spec.list_key.map_or("—".to_owned(), |k| format!("`{k}`")),
            theirs,
            notes,
        )
        .unwrap();
    }
    table
}

#[test]
fn docs_endpoints() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/endpoints.md");
    let expected = endpoints_table();
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::write(path, &expected).unwrap();
    }
    let actual = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        actual == expected,
        "docs/endpoints.md is out of date; run `UPDATE_DOCS=1 cargo test --test catalog docs_endpoints`"
    );
}

#[test]
fn the_postmark_rs_map_covers_the_44_endpoints() {
    let fixture: BTreeSet<&str> = include_str!("fixtures/get-endpoints.txt").lines().collect();
    let mapped: BTreeSet<&str> = POSTMARK_RS.iter().map(|(p, _, _)| *p).collect();
    assert_eq!(mapped, fixture);
}

#[test]
fn parse_request_reports_what_it_cannot_build() {
    let error = parse_request(&["postmark-cli", "commands"]).unwrap_err();
    assert_eq!(error.kind_name(), "config");
    let error = parse_request(&["postmark-cli", "servers", "get", "abc"]).unwrap_err();
    assert_eq!(error.kind_name(), "config");
    let error = parse_request(&["postmark-cli", "servers", "list", "--count", "0"]).unwrap_err();
    assert_eq!(error.message(), "--count must be at least 1");
}
