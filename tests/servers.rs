//! `servers list` and `servers get`: the account token, and pagination.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::{Value, json};
use std::ops::Range;

fn page(ids: Range<u32>, total: u32) -> String {
    let servers: Vec<_> = ids
        .map(|id| json!({"ID": id, "Name": format!("s{id}")}))
        .collect();
    json!({"TotalCount": total, "Servers": servers}).to_string()
}

fn servers(server: &Server, args: &[&str]) -> Run {
    let base = base_url(server);
    let mut all = vec!["servers"];
    all.extend_from_slice(args);
    all.extend_from_slice(&["--base-url", &base]);
    run(
        &all,
        &[
            ("POSTMARK_ACCOUNT_TOKEN", SENTINEL_TOKEN),
            // Set, and still never sent: these endpoints need the account token.
            ("POSTMARK_SERVER_TOKEN", "server-token-must-not-be-sent"),
        ],
    )
}

fn ids(items: &[Value]) -> Vec<u64> {
    items.iter().map(|i| i["ID"].as_u64().unwrap()).collect()
}

#[test]
fn list_sends_the_default_page_with_the_account_token() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=50&offset=0", Token::Account)
            .respond_with(status_code(200).body(page(0..2, 2))),
    );

    let out = servers(&server, &["list"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.json(),
        serde_json::from_str::<Value>(&page(0..2, 2)).unwrap()
    );
    assert_no_secret(&out);
}

#[test]
fn list_sends_the_name_filter() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=50&offset=0&name=foo", Token::Account)
            .respond_with(status_code(200).body(page(0..1, 1))),
    );

    let out = servers(&server, &["list", "--name", "foo"]);

    assert_eq!(out.code, 0, "{out:?}");
}

#[test]
fn a_count_over_the_maximum_is_a_config_error_before_any_request() {
    let server = Server::run();

    let out = servers(&server, &["list", "--count", "501"]);

    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(out.error()["kind"], "config");
    assert!(out.error()["message"].as_str().unwrap().contains("500"));
    assert_no_secret(&out);
}

#[test]
fn jsonl_prints_one_line_per_server() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=50&offset=0", Token::Account)
            .respond_with(status_code(200).body(page(0..3, 3))),
    );

    let out = servers(&server, &["list", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.stdout,
        "{\"ID\":0,\"Name\":\"s0\"}\n{\"ID\":1,\"Name\":\"s1\"}\n{\"ID\":2,\"Name\":\"s2\"}\n"
    );
}

#[test]
fn jsonl_prints_a_body_without_the_list_whole_and_warns() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=50&offset=0", Token::Account)
            .respond_with(status_code(200).body(r#"{"TotalCount":0}"#)),
    );

    let out = servers(&server, &["list", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "{\"TotalCount\":0}\n");
    assert_eq!(out.stderr_lines()[0]["warning"]["kind"], "list_key_missing");
}

/// Three pages of 2, 2 and 1 servers.
fn three_pages(server: &Server) {
    for (offset, ids) in [(0, 0..2), (2, 2..4), (4, 4..5)] {
        server.expect(
            expect_get(
                "/servers",
                &format!("count=2&offset={offset}"),
                Token::Account,
            )
            .respond_with(status_code(200).body(page(ids, 5))),
        );
    }
}

#[test]
fn all_in_json_merges_the_pages_into_one_response() {
    let server = Server::run();
    three_pages(&server);

    let out = servers(&server, &["list", "--all", "--count", "2"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stderr, "");
    let body = out.json();
    assert_eq!(body["TotalCount"], 5);
    assert_eq!(ids(body["Servers"].as_array().unwrap()), [0, 1, 2, 3, 4]);
    let keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["TotalCount", "Servers"]);
}

#[test]
fn all_in_jsonl_prints_every_server_on_its_own_line() {
    let server = Server::run();
    three_pages(&server);

    let out = servers(
        &server,
        &["list", "--all", "--count", "2", "--format", "jsonl"],
    );

    assert_eq!(out.code, 0, "{out:?}");
    let lines: Vec<Value> = out
        .stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(ids(&lines), [0, 1, 2, 3, 4]);
}

#[test]
fn limit_stops_at_exactly_n_servers() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=5&offset=0", Token::Account)
            .respond_with(status_code(200).body(page(0..5, 100))),
    );
    server.expect(
        expect_get("/servers", "count=2&offset=5", Token::Account)
            .respond_with(status_code(200).body(page(5..7, 100))),
    );

    let out = servers(&server, &["list", "--limit", "7", "--count", "5"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        ids(out.json()["Servers"].as_array().unwrap()),
        [0, 1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn all_prints_a_first_page_without_the_list_whole_and_warns() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=500&offset=0", Token::Account)
            .respond_with(status_code(200).body(r#"{"Odd":true}"#)),
    );

    let out = servers(&server, &["list", "--all"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.json(), json!({"Odd": true}));
    assert_eq!(out.stderr_lines()[0]["warning"]["kind"], "list_key_missing");
}

#[test]
fn all_in_jsonl_prints_a_page_without_the_list_whole_and_warns() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=500&offset=0", Token::Account)
            .respond_with(status_code(200).body(r#"{"Odd":true}"#)),
    );

    let out = servers(&server, &["list", "--all", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "{\"Odd\":true}\n");
    assert_eq!(out.stderr_lines()[0]["warning"]["kind"], "list_key_missing");
}

#[test]
fn an_error_on_a_later_page_fails_the_run() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=2&offset=0", Token::Account)
            .respond_with(status_code(200).body(page(0..2, 5))),
    );
    server.expect(
        expect_get("/servers", "count=2&offset=2", Token::Account)
            .respond_with(status_code(500).body("down")),
    );

    let out = servers(&server, &["list", "--all", "--count", "2"]);

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(out.stdout, "");
    assert_eq!(out.error()["status"], 500);
}

#[test]
fn dry_run_with_all_prints_the_first_page_request() {
    let out = run(&["servers", "list", "--all", "--dry-run"], &[]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.json(),
        json!({
            "method": "GET",
            "url": "https://api.postmarkapp.com/servers?count=500&offset=0",
            "token": "account"
        })
    );
}

#[test]
fn get_puts_the_server_id_in_the_path() {
    let server = Server::run();
    server.expect(
        expect_get("/servers/123", "", Token::Account)
            .respond_with(status_code(200).body(r#"{"ID":123,"Name":"x"}"#)),
    );

    let out = servers(&server, &["get", "123"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.json(), json!({"ID": 123, "Name": "x"}));
}

#[test]
fn get_with_an_id_that_is_not_a_number_is_a_usage_error() {
    let server = Server::run();

    let out = servers(&server, &["get", "abc"]);

    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(out.stdout, "");
}

#[test]
fn a_missing_account_token_names_its_env_var() {
    let server = Server::run();

    let out = run(
        &["servers", "get", "1", "--base-url", &base_url(&server)],
        &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)],
    );

    assert_eq!(out.code, 2, "{out:?}");
    let message = out.error()["message"].as_str().unwrap().to_owned();
    assert!(message.contains("POSTMARK_ACCOUNT_TOKEN"), "{message}");
    assert!(message.contains("--account-token"), "{message}");
    assert_no_secret(&out);
}

#[test]
fn limit_reached_on_the_first_page_prints_that_page_cut_to_n() {
    let server = Server::run();
    server.expect(
        expect_get("/servers", "count=2&offset=0", Token::Account)
            .respond_with(status_code(200).body(page(0..5, 5))),
    );

    let out = servers(&server, &["list", "--limit", "2"]);

    assert_eq!(out.code, 0, "{out:?}");
    let body = out.json();
    assert_eq!(body["TotalCount"], 5);
    assert_eq!(ids(body["Servers"].as_array().unwrap()), [0, 1]);
}
