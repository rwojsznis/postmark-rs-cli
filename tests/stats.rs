//! `stats`: twelve endpoints with the same four filters.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::Value;

const COMMANDS: [(&str, &str); 12] = [
    ("overview", "/stats/outbound"),
    ("sends", "/stats/outbound/sends"),
    ("bounces", "/stats/outbound/bounces"),
    ("spam", "/stats/outbound/spam"),
    ("tracked", "/stats/outbound/tracked"),
    ("opens", "/stats/outbound/opens"),
    ("opens-platforms", "/stats/outbound/opens/platforms"),
    ("opens-email-clients", "/stats/outbound/opens/emailclients"),
    ("clicks", "/stats/outbound/clicks"),
    (
        "clicks-browser-families",
        "/stats/outbound/clicks/browserfamilies",
    ),
    ("clicks-platforms", "/stats/outbound/clicks/platforms"),
    ("clicks-location", "/stats/outbound/clicks/location"),
];

fn cli(server: &Server, args: &[&str]) -> Run {
    let base = base_url(server);
    let mut all = args.to_vec();
    all.extend_from_slice(&["--base-url", &base]);
    run(&all, &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)])
}

#[test]
fn every_stats_command_sends_the_shared_filters_and_passes_the_body_through() {
    // Dynamic keys, as the email client and browser family stats have.
    let body = r#"{"Days":[{"Date":"2026-01-01","Apple Mail":3}],"Apple Mail":3}"#;
    for (command, path) in COMMANDS {
        let server = Server::run();
        server.expect(
            expect_get(
                path,
                "tag=welcome&fromdate=2026-01-01&todate=2026-01-31&messagestream=outbound",
                Token::Server,
            )
            .respond_with(status_code(200).body(body)),
        );

        let out = cli(
            &server,
            &[
                "stats",
                command,
                "--tag",
                "welcome",
                "--from-date",
                "2026-01-01",
                "--to-date",
                "2026-01-31",
                "--message-stream",
                "outbound",
                "--format",
                "jsonl",
            ],
        );

        assert_eq!(out.code, 0, "{command}: {out:?}");
        assert_eq!(out.stdout, format!("{body}\n"), "{command}");
        assert_eq!(out.stderr, "", "{command}");
        assert_no_secret(&out);
    }
}

#[test]
fn stats_without_filters_send_no_query() {
    let server = Server::run();
    server.expect(
        expect_get("/stats/outbound", "", Token::Server)
            .respond_with(status_code(200).body(r#"{"Sent":615,"BounceRate":0.5}"#)),
    );

    let out = cli(&server, &["stats", "overview"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.json(),
        serde_json::from_str::<Value>(r#"{"Sent":615,"BounceRate":0.5}"#).unwrap()
    );
}

#[test]
fn a_date_with_a_time_exits_2_before_any_request() {
    let server = Server::run();
    let out = cli(
        &server,
        &["stats", "sends", "--from-date", "2026-01-01T00:00:00"],
    );
    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(out.stdout, "");
}

#[test]
fn stats_not_on_the_plan_is_an_api_error() {
    let server = Server::run();
    server.expect(
        expect_get("/stats/outbound", "", Token::Server).respond_with(
            status_code(422).body(r#"{"ErrorCode":614,"Message":"Stats API is not available."}"#),
        ),
    );

    let out = cli(&server, &["stats", "overview"]);

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(out.error()["error_code"], 614);
}
