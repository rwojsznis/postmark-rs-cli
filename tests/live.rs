//! Smoke tests against the real Postmark API. Opt-in: every test is
//! `#[ignore]`, CI never runs them, and each one skips when its token is not
//! set.
//!
//! ```bash
//! POSTMARK_LIVE_SERVER_TOKEN=… POSTMARK_LIVE_ACCOUNT_TOKEN=… \
//!   mise exec -- cargo test --test live -- --ignored
//! ```
//!
//! They are safe to run on a production account: every command is a GET
//! (decision 1), and the lists ask for one item.

mod common;

use common::cli;
use serde_json::Value;

const SERVER: &str = "POSTMARK_LIVE_SERVER_TOKEN";
const ACCOUNT: &str = "POSTMARK_LIVE_ACCOUNT_TOKEN";

/// Runs the command with the live token in the env var the CLI reads, and
/// returns the parsed stdout. `None` when the live token is not set.
fn live(token_var: &str, args: &[&str]) -> Option<Value> {
    let Ok(token) = std::env::var(token_var) else {
        eprintln!("skipped: set {token_var} to run this test");
        return None;
    };
    let cli_var = if token_var == SERVER {
        "POSTMARK_SERVER_TOKEN"
    } else {
        "POSTMARK_ACCOUNT_TOKEN"
    };
    let out = cli().args(args).env(cli_var, &token).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(!stdout.contains(&token), "{args:?}: the token is on stdout");
    assert!(!stderr.contains(&token), "{args:?}: the token is on stderr");
    assert_eq!(out.status.code(), Some(0), "{args:?}: {stderr}");
    Some(serde_json::from_str(&stdout).unwrap())
}

fn assert_list(body: &Value, key: &str) {
    assert!(body[key].is_array(), "no {key:?} array: {body}");
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn server_get() {
    if let Some(body) = live(SERVER, &["server", "get"]) {
        assert!(body["ID"].is_u64(), "{body}");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn outbound_search() {
    if let Some(body) = live(SERVER, &["outbound", "search", "--count", "1"]) {
        assert_list(&body, "Messages");
    }
}

/// The docs' response table says `Messages`; the example says
/// `InboundMessages`. This is the test that settles it.
#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn inbound_search_lists_inbound_messages() {
    if let Some(body) = live(SERVER, &["inbound", "search", "--count", "1"]) {
        assert_list(&body, "InboundMessages");
    }
}

/// The docs' table spells them `Count` and `Offset`; we send lowercase.
#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn templates_list_takes_lowercase_count_and_offset() {
    if let Some(body) = live(SERVER, &["templates", "list", "--count", "1"]) {
        assert_list(&body, "Templates");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn streams_list() {
    if let Some(body) = live(SERVER, &["streams", "list"]) {
        assert_list(&body, "MessageStreams");
    }
}

/// A plan without the Stats API answers 422 with error code 614, which this
/// test reports as a failure: read its stderr.
#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn stats_overview() {
    if let Some(body) = live(SERVER, &["stats", "overview"]) {
        assert!(body["Sent"].is_number(), "{body}");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn bounces_delivery_stats() {
    if let Some(body) = live(SERVER, &["bounces", "delivery-stats"]) {
        assert_list(&body, "Bounces");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_SERVER_TOKEN"]
fn webhooks_list() {
    if let Some(body) = live(SERVER, &["webhooks", "list"]) {
        assert_list(&body, "Webhooks");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_ACCOUNT_TOKEN"]
fn servers_list() {
    if let Some(body) = live(ACCOUNT, &["servers", "list", "--count", "1"]) {
        assert_list(&body, "Servers");
    }
}

#[test]
#[ignore = "live: needs POSTMARK_LIVE_ACCOUNT_TOKEN"]
fn domains_list() {
    if let Some(body) = live(ACCOUNT, &["domains", "list", "--count", "1"]) {
        assert_list(&body, "Domains");
    }
}
