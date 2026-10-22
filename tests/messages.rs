//! `outbound` and `inbound`: message search, details and raw source.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::{Value, json};

fn cli(server: &Server, args: &[&str]) -> Run {
    let base = base_url(server);
    let mut all = args.to_vec();
    all.extend_from_slice(&["--base-url", &base]);
    run(
        &all,
        &[
            ("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN),
            ("POSTMARK_ACCOUNT_TOKEN", "account-token-must-not-be-sent"),
        ],
    )
}

/// Expects one request and checks that stdout is the body, unchanged.
fn passes_through(args: &[&str], path: &str, query: &str, body: &str) {
    let server = Server::run();
    server.expect(
        expect_get(path, query, Token::Server).respond_with(status_code(200).body(body.to_owned())),
    );

    let out = cli(&server, args);

    assert_eq!(out.code, 0, "{args:?}: {out:?}");
    assert_eq!(
        out.json(),
        serde_json::from_str::<Value>(body).unwrap(),
        "{args:?}"
    );
    assert_no_secret(&out);
}

#[test]
fn outbound_search_sends_its_filters() {
    passes_through(
        &[
            "outbound",
            "search",
            "--recipient",
            "john@example.com",
            "--status",
            "SENT",
            "--from-date",
            "2026-10-01",
            "--to-date",
            "2026-10-02T12:00:00",
            "--metadata",
            "color=dark blue",
            "--count",
            "10",
        ],
        "/messages/outbound",
        "count=10&offset=0&recipient=john%40example.com&status=sent\
         &todate=2026-10-02T12%3A00%3A00&fromdate=2026-10-01&metadata_color=dark+blue",
        r#"{"TotalCount":1,"Messages":[{"MessageID":"m1","Subject":"Hi"}]}"#,
    );
}

#[test]
fn outbound_get_returns_the_details() {
    passes_through(
        &["outbound", "get", "07311c54-0687-4ab9-b034-b54b5bad88ba"],
        "/messages/outbound/07311c54-0687-4ab9-b034-b54b5bad88ba/details",
        "",
        r#"{"MessageID":"07311c54","MessageEvents":[{"Type":"Delivered"}],"Body":"raw"}"#,
    );
}

#[test]
fn outbound_dump_returns_an_empty_body_unchanged() {
    passes_through(
        &["outbound", "dump", "m1"],
        "/messages/outbound/m1/dump",
        "",
        r#"{"Body":""}"#,
    );
}

#[test]
fn inbound_search_sends_its_filters() {
    passes_through(
        &[
            "inbound",
            "search",
            "--mailbox-hash",
            "abc",
            "--status",
            "blocked",
        ],
        "/messages/inbound",
        "count=50&offset=0&mailboxhash=abc&status=blocked",
        r#"{"TotalCount":1,"InboundMessages":[{"MessageID":"i1"}]}"#,
    );
}

#[test]
fn inbound_get_returns_the_details() {
    passes_through(
        &["inbound", "get", "i1"],
        "/messages/inbound/i1/details",
        "",
        r#"{"MessageID":"i1","BlockedReason":"rule"}"#,
    );
}

#[test]
fn inbound_search_in_jsonl_prints_one_message_per_line() {
    let server = Server::run();
    server.expect(
        expect_get("/messages/inbound", "count=50&offset=0", Token::Server).respond_with(
            status_code(200).body(
                r#"{"TotalCount":2,"InboundMessages":[{"MessageID":"a"},{"MessageID":"b"}]}"#,
            ),
        ),
    );

    let out = cli(&server, &["inbound", "search", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "{\"MessageID\":\"a\"}\n{\"MessageID\":\"b\"}\n");
    assert_eq!(out.stderr, "");
}

#[test]
fn a_message_id_that_needs_encoding_stays_one_segment() {
    passes_through(
        &["outbound", "get", "a/b c?d"],
        "/messages/outbound/a%2Fb%20c%3Fd/details",
        "",
        "{}",
    );
}

#[test]
fn bad_flag_values_exit_2_before_any_request() {
    let server = Server::run();
    for args in [
        &["outbound", "search", "--status", "bogus"][..],
        &["inbound", "search", "--status", "sent"],
        &["outbound", "search", "--from-date", "2026-13-01"],
        &["inbound", "search", "--to-date", "2026-01-01 12:00:00"],
        &["outbound", "search", "--metadata", "=x"],
        &["outbound", "search", "--count", "500", "--offset", "9501"],
        &["outbound", "get", ".."],
    ] {
        let out = cli(&server, args);
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
        assert_eq!(out.stdout, "", "{args:?}");
        assert_no_secret(&out);
    }
}

#[test]
fn all_stops_at_the_10000_window_and_warns() {
    let server = Server::run();
    let messages: Vec<_> = (0..10).map(|i| json!({"MessageID": i})).collect();
    server.expect(
        expect_get("/messages/outbound", "count=10&offset=9990", Token::Server).respond_with(
            status_code(200).body(json!({"TotalCount": 20_000, "Messages": messages}).to_string()),
        ),
    );

    let out = cli(
        &server,
        &[
            "outbound", "search", "--all", "--count", "10", "--offset", "9990",
        ],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.json()["Messages"].as_array().unwrap().len(), 10);
    let warning = &out.stderr_lines()[0]["warning"];
    assert_eq!(warning["kind"], "pagination_ceiling");
    assert!(
        warning["message"].as_str().unwrap().contains("10000"),
        "{warning}"
    );
}
