//! Every error path: one JSON line on stderr, nothing on stdout, the exit code
//! of decision 5, and never the token (decision 6).

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::{delay_and_then, status_code};
use serde_json::json;
use std::time::Duration;

fn server_get(server_base: &str, extra: &[&str]) -> Run {
    let mut args = vec!["server", "get", "--base-url", server_base];
    args.extend_from_slice(extra);
    run(&args, &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)])
}

/// The checks every error shares.
fn assert_error(out: &Run, code: i32, kind: &str) -> serde_json::Value {
    assert_eq!(out.code, code, "{out:?}");
    assert_eq!(out.stdout, "", "an error prints nothing on stdout");
    assert_eq!(out.stderr.lines().count(), 1, "one line: {}", out.stderr);
    assert_no_secret(out);
    let error = out.error();
    assert_eq!(error["kind"], kind, "{error}");
    assert!(error["message"].is_string(), "{error}");
    error
}

#[test]
fn a_missing_token_is_a_config_error_that_names_the_env_var_and_the_flag() {
    // No expectations: a request would fail the test when `server` drops.
    let server = Server::run();
    let base = base_url(&server);

    let out = run(
        &["server", "get", "--base-url", &base],
        // The other token does not stand in for the missing one.
        &[("POSTMARK_ACCOUNT_TOKEN", SENTINEL_TOKEN)],
    );

    let error = assert_error(&out, 2, "config");
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("POSTMARK_SERVER_TOKEN"), "{message}");
    assert!(message.contains("--server-token"), "{message}");
    assert_eq!(
        error["request"],
        json!({"method": "GET", "url": format!("{base}server")})
    );
    assert!(error.get("status").is_none());
}

#[test]
fn an_empty_token_counts_as_missing() {
    let server = Server::run();
    let out = run(
        &["server", "get", "--base-url", &base_url(&server)],
        &[("POSTMARK_SERVER_TOKEN", "")],
    );
    assert_error(&out, 2, "config");
}

#[test]
fn a_token_that_cannot_be_a_header_is_a_config_error() {
    let server = Server::run();
    let token = format!("{SENTINEL_TOKEN}\n");
    let out = run(
        &["server", "get", "--base-url", &base_url(&server)],
        &[("POSTMARK_SERVER_TOKEN", &token)],
    );
    let error = assert_error(&out, 2, "config");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("POSTMARK_SERVER_TOKEN"),
        "{error}"
    );
}

#[test]
fn an_api_error_carries_the_status_the_error_code_and_the_parsed_body() {
    let server = Server::run();
    let body = r#"{"ErrorCode":10,"Message":"No Account or Server API tokens were supplied in the HTTP headers."}"#;
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(401).body(body)));
    let base = base_url(&server);

    let out = server_get(&base, &[]);

    let error = assert_error(&out, 1, "api");
    assert_eq!(error["status"], 401);
    assert_eq!(error["error_code"], 10);
    assert_eq!(
        error["body"],
        serde_json::from_str::<serde_json::Value>(body).unwrap()
    );
    assert_eq!(
        error["message"],
        "Postmark returned 401: No Account or Server API tokens were supplied in the HTTP headers."
    );
    assert_eq!(
        error["request"],
        json!({"method": "GET", "url": format!("{base}server")})
    );
    // The key order of decision 5.
    let keys: Vec<_> = error.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        ["kind", "message", "status", "error_code", "body", "request"]
    );
}

#[test]
fn a_body_that_is_not_json_is_kept_as_a_string_of_at_most_4_kib() {
    let server = Server::run();
    let html = format!("<html>{}</html>", "é".repeat(5000));
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(500).body(html)));

    let out = server_get(&base_url(&server), &[]);

    let error = assert_error(&out, 1, "api");
    assert_eq!(error["status"], 500);
    assert_eq!(error["error_code"], serde_json::Value::Null);
    assert_eq!(error["message"], "Postmark returned 500");
    let body = error["body"].as_str().unwrap();
    assert!(body.starts_with("<html>é"), "{body}");
    assert!(body.len() <= 4096, "{} bytes", body.len());
    assert!(
        body.len() > 4000,
        "cut at 4 KiB, not before: {} bytes",
        body.len()
    );
}

#[test]
fn a_success_body_that_is_not_json_is_a_decode_error() {
    let server = Server::run();
    server.expect(
        expect_get("/server", "", Token::Server).respond_with(status_code(200).body("<html>")),
    );
    let base = base_url(&server);

    let out = server_get(&base, &[]);

    let error = assert_error(&out, 3, "decode");
    assert_eq!(error["request"]["url"], format!("{base}server"));
    assert!(error.get("status").is_none());
}

#[test]
fn a_refused_connection_is_a_transport_error() {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let base = format!("http://127.0.0.1:{port}/");

    let out = server_get(&base, &[]);

    let error = assert_error(&out, 3, "transport");
    assert_eq!(error["request"]["url"], format!("{base}server"));
}

#[test]
fn no_answer_within_the_timeout_is_a_timeout_error() {
    let server = Server::run();
    server.expect(
        expect_get("/server", "", Token::Server).respond_with(delay_and_then(
            Duration::from_secs(2),
            status_code(200).body("{}"),
        )),
    );

    let out = server_get(&base_url(&server), &["--timeout", "1"]);

    let error = assert_error(&out, 3, "timeout");
    assert!(
        error["message"].as_str().unwrap().contains("1 s"),
        "{error}"
    );
}

#[test]
fn a_base_url_that_is_not_http_is_a_config_error() {
    for base in ["ftp://x", "not a url"] {
        let out = server_get(base, &[]);
        let error = assert_error(&out, 2, "config");
        assert!(
            error["message"].as_str().unwrap().contains("--base-url"),
            "{error}"
        );
        assert!(error.get("request").is_none(), "{error}");
    }
}

#[test]
fn dry_run_needs_no_token_and_sends_nothing() {
    let server = Server::run();
    let base = base_url(&server);

    let out = run(&["server", "get", "--dry-run", "--base-url", &base], &[]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stderr, "");
    assert_eq!(
        out.json(),
        json!({"method": "GET", "url": format!("{base}server"), "token": "server"})
    );
}

#[test]
fn dry_run_never_shows_the_token() {
    let out = run(
        &[
            "server",
            "get",
            "--dry-run",
            "--server-token",
            SENTINEL_TOKEN,
        ],
        &[],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.json()["url"], "https://api.postmarkapp.com/server");
    assert_no_secret(&out);
}

#[test]
fn verbose_logs_the_request_and_the_response_as_json_lines() {
    let server = Server::run();
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(200).body("{}")));
    let base = base_url(&server);

    let out = server_get(&base, &["-v"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_no_secret(&out);
    let lines = out.stderr_lines();
    assert_eq!(lines.len(), 2, "{}", out.stderr);
    assert_eq!(
        lines[0],
        json!({"request": {"method": "GET", "url": format!("{base}server"), "token": "server"}})
    );
    assert_eq!(lines[1]["response"]["status"], "ok");
    assert!(lines[1]["response"]["ms"].is_u64(), "{}", lines[1]);
}

#[test]
fn verbose_logs_the_status_of_a_failed_request_before_the_error() {
    let server = Server::run();
    server.expect(
        expect_get("/server", "", Token::Server)
            .respond_with(status_code(422).body(r#"{"ErrorCode":1,"Message":"x"}"#)),
    );

    let out = server_get(&base_url(&server), &["--verbose"]);

    assert_eq!(out.code, 1, "{out:?}");
    assert_no_secret(&out);
    let lines = out.stderr_lines();
    assert_eq!(lines.len(), 3, "{}", out.stderr);
    assert!(lines[0].get("request").is_some());
    assert_eq!(lines[1]["response"]["status"], 422);
    assert_eq!(lines[2]["error"]["kind"], "api");
}

#[test]
fn verbose_names_the_error_kind_when_there_is_no_status() {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let out = server_get(&format!("http://127.0.0.1:{port}/"), &["-v"]);

    assert_eq!(out.code, 3, "{out:?}");
    let lines = out.stderr_lines();
    assert_eq!(lines.len(), 3, "{}", out.stderr);
    assert_eq!(lines[1]["response"]["status"], "transport");
}
