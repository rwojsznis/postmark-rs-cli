//! `webhooks`, `inbound-rules` and `bulk`, the one cursor-paginated list.

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
fn webhooks_list_sends_message_stream_in_pascal_case() {
    passes_through(
        &["webhooks", "list", "--message-stream", "outbound"],
        "/webhooks",
        "MessageStream=outbound",
        r#"{"Webhooks":[{"ID":1,"Url":"https://example.com/hook"}]}"#,
    );
}

#[test]
fn webhooks_get_and_stats_take_the_id() {
    passes_through(
        &["webhooks", "get", "1234567"],
        "/webhooks/1234567",
        "",
        r#"{"ID":1234567}"#,
    );
    passes_through(
        &["webhooks", "stats", "1234567"],
        "/webhooks/1234567/statistics",
        "",
        r#"{"WebhookId":1234567,"Metrics":{"AverageRetryResponseTimeMs":null}}"#,
    );
}

#[test]
fn inbound_rules_list_pages() {
    passes_through(
        &["inbound-rules", "list", "--count", "10"],
        "/triggers/inboundrules",
        "count=10&offset=0",
        r#"{"TotalCount":1,"InboundRules":[{"ID":3,"Rule":"someone@example.com"}]}"#,
    );
}

#[test]
fn bulk_get_takes_the_request_id() {
    passes_through(
        &["bulk", "get", "f24af63c-533d-4b7a-ad65-4a7b3202d3a7"],
        "/email/bulk/f24af63c-533d-4b7a-ad65-4a7b3202d3a7",
        "",
        r#"{"Id":"f24af63c-533d-4b7a-ad65-4a7b3202d3a7","Status":"Accepted"}"#,
    );
}

#[test]
fn bulk_list_sends_count_and_the_key_as_given() {
    passes_through(
        &[
            "bulk",
            "list",
            "--count",
            "20",
            "--pagination-key",
            "eyJT+/=",
        ],
        "/email/bulk",
        "count=20&paginationKey=eyJT%2B%2F%3D",
        r#"{"Requests":[{"Id":"a"}],"PaginationKey":"next"}"#,
    );
}

/// Three pages that follow `PaginationKey`; the last has none.
fn three_pages(server: &Server) {
    let pages = [
        (
            "count=500",
            json!({"Requests": [{"Id": "a"}, {"Id": "b"}], "PaginationKey": "k1"}),
        ),
        (
            "count=500&paginationKey=k1",
            json!({"Requests": [{"Id": "c"}], "PaginationKey": "k+/=2"}),
        ),
        (
            "count=500&paginationKey=k%2B%2F%3D2",
            json!({"Requests": [{"Id": "d"}]}),
        ),
    ];
    for (query, body) in pages {
        server.expect(
            expect_get("/email/bulk", query, Token::Server)
                .respond_with(status_code(200).body(body.to_string())),
        );
    }
}

#[test]
fn bulk_list_all_follows_the_keys_and_drops_the_last_one() {
    let server = Server::run();
    three_pages(&server);

    let out = cli(&server, &["bulk", "list", "--all"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.json(),
        json!({"Requests": [{"Id": "a"}, {"Id": "b"}, {"Id": "c"}, {"Id": "d"}]})
    );
}

#[test]
fn bulk_list_all_in_jsonl_streams_the_requests() {
    let server = Server::run();
    three_pages(&server);

    let out = cli(&server, &["bulk", "list", "--all", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.stdout,
        "{\"Id\":\"a\"}\n{\"Id\":\"b\"}\n{\"Id\":\"c\"}\n{\"Id\":\"d\"}\n"
    );
}

#[test]
fn bulk_without_approval_is_an_api_error() {
    let server = Server::run();
    server.expect(
        expect_get("/email/bulk", "count=50", Token::Server).respond_with(
            status_code(422).body(r#"{"ErrorCode":14,"Message":"Bulk API is not enabled."}"#),
        ),
    );

    let out = cli(&server, &["bulk", "list"]);

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(out.error()["error_code"], 14);
    assert_eq!(out.stdout, "");
}

#[test]
fn bad_values_exit_2_before_any_request() {
    let server = Server::run();
    for args in [
        &["webhooks", "get", "abc"][..],
        &["webhooks", "stats", "1", "--message-stream", "x"],
        &["bulk", "list", "--offset", "1"],
        &["bulk", "list", "--count", "501"],
    ] {
        let out = cli(&server, args);
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
    }
}
