//! `opens` and `clicks`: tracking data for sent messages.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::{Value, json};

fn cli(server: &Server, args: &[&str]) -> Run {
    let base = base_url(server);
    let mut all = args.to_vec();
    all.extend_from_slice(&["--base-url", &base]);
    run(&all, &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)])
}

fn page(key: &str, ids: std::ops::Range<u32>, total: u32) -> String {
    let items: Vec<_> = ids.map(|id| json!({"MessageID": id})).collect();
    json!({"TotalCount": total, key: items}).to_string()
}

fn passes_through(args: &[&str], path: &str, query: &str, body: &str) {
    let server = Server::run();
    server.expect(
        expect_get(path, query, Token::Server).respond_with(status_code(200).body(body.to_owned())),
    );

    let out = cli(&server, args);

    assert_eq!(out.code, 0, "{args:?}: {out:?}");
    assert_eq!(out.json(), serde_json::from_str::<Value>(body).unwrap());
    assert_no_secret(&out);
}

#[test]
fn opens_list_sends_its_filters() {
    passes_through(
        &[
            "opens",
            "list",
            "--recipient",
            "a@example.com",
            "--platform",
            "mobile",
            "--os-name",
            "OS X 10.9 Mavericks",
            "--city",
            "New York",
        ],
        "/messages/outbound/opens",
        "count=50&offset=0&recipient=a%40example.com&os_name=OS+X+10.9+Mavericks\
         &platform=mobile&city=New+York",
        &page("Opens", 0..1, 1),
    );
}

#[test]
fn opens_for_message_pages_one_message() {
    passes_through(
        &["opens", "for-message", "m1", "--count", "1"],
        "/messages/outbound/opens/m1",
        "count=1&offset=0",
        &page("Opens", 0..1, 1),
    );
}

#[test]
fn clicks_list_sends_its_filters() {
    passes_through(
        &[
            "clicks",
            "list",
            "--client-name",
            "Gmail",
            "--message-stream",
            "broadcast",
        ],
        "/messages/outbound/clicks",
        "count=50&offset=0&messagestream=broadcast&client_name=Gmail",
        &page("Clicks", 0..2, 2),
    );
}

#[test]
fn clicks_for_message_pages_one_message() {
    passes_through(
        &["clicks", "for-message", "m1"],
        "/messages/outbound/clicks/m1",
        "count=50&offset=0",
        &page("Clicks", 0..0, 0),
    );
}

#[test]
fn opens_list_all_stops_at_the_10000_window() {
    let server = Server::run();
    server.expect(
        expect_get(
            "/messages/outbound/opens",
            "count=2&offset=9998",
            Token::Server,
        )
        .respond_with(status_code(200).body(page("Opens", 0..2, 50_000))),
    );

    let out = cli(
        &server,
        &["opens", "list", "--all", "--count", "2", "--offset", "9998"],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.stderr_lines()[0]["warning"]["kind"],
        "pagination_ceiling"
    );
}

#[test]
fn clicks_list_all_has_no_window() {
    let server = Server::run();
    for (offset, ids) in [(9998, 0..2), (10_000, 2..4), (10_002, 4..5)] {
        server.expect(
            expect_get(
                "/messages/outbound/clicks",
                &format!("count=2&offset={offset}"),
                Token::Server,
            )
            .respond_with(status_code(200).body(page("Clicks", ids, 10_003))),
        );
    }

    let out = cli(
        &server,
        &[
            "clicks", "list", "--all", "--count", "2", "--offset", "9998",
        ],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stderr, "");
    assert_eq!(out.json()["Clicks"].as_array().unwrap().len(), 5);
}
