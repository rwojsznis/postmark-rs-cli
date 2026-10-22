//! A reader that quits early (`| head`) closes the pipe.
//!
//! Rust ignores `SIGPIPE`, so the failed write comes back as an error and
//! `println!` panics on it. That is not a failure: the output simply has
//! nowhere left to go. With `--all`, it also means there is no reason to fetch
//! another page (decision 9).

mod common;

use common::{SENTINEL_TOKEN, Token, base_url, cli, expect_get};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::json;
use std::io::Read;
use std::process::Stdio;

#[test]
fn a_closed_stdout_stops_the_pages_and_is_not_a_panic() {
    let server = Server::run();
    // Full pages of 500 out of 100,000: without the stop, 200 requests.
    let servers: Vec<_> = (0..500).map(|id| json!({"ID": id})).collect();
    let body = json!({"TotalCount": 100_000, "Servers": servers}).to_string();
    server.expect(
        expect_get("/servers", "count=500&offset=0", Token::Account)
            .times(1)
            .respond_with(status_code(200).body(body.clone())),
    );
    // Allowed, not required: the second page may already be on its way.
    server.expect(
        expect_get("/servers", "count=500&offset=500", Token::Account)
            .times(..=1)
            .respond_with(status_code(200).body(body)),
    );

    let mut child = cli()
        .args(["servers", "list", "--all", "--format", "jsonl"])
        .args(["--base-url", &base_url(&server)])
        .env("POSTMARK_ACCOUNT_TOKEN", SENTINEL_TOKEN)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    drop(child.stdout.take());
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr reads");
    let status = child.wait().expect("the child exits");

    assert!(
        !stderr.contains("panicked"),
        "panicked on a closed pipe:\n{stderr}"
    );
    assert_eq!(status.code(), Some(0), "{stderr}");
    // `server` checks on drop that no third page was asked for.
}
