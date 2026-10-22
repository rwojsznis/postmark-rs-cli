//! `server get`: `GET /server` with the server token.

mod common;

use common::{SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;

// Keys out of alphabetical order, so that a reordering map would show.
const BODY: &str = r#"{"Name":"Staging","ID":11,"ApiTokens":["redacted"],"Color":"red","SmtpApiActivated":true,"InboundAddress":"x@inbound.postmarkapp.com","DeliveryType":"Live"}"#;

#[test]
fn prints_the_server_with_the_key_order_postmark_sent() {
    let server = Server::run();
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(200).body(BODY)));

    let out = run(
        &["server", "get", "--base-url", &base_url(&server)],
        &[
            ("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN),
            // Set, and still never sent: the endpoint needs the server token.
            ("POSTMARK_ACCOUNT_TOKEN", "account-token-must-not-be-sent"),
        ],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stderr, "");
    let expected: serde_json::Value = serde_json::from_str(BODY).unwrap();
    assert_eq!(out.json(), expected);
    let keys: Vec<_> = out.json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "Name",
            "ID",
            "ApiTokens",
            "Color",
            "SmtpApiActivated",
            "InboundAddress",
            "DeliveryType"
        ]
    );
    // Pretty, with a 2-space indent and a trailing newline (decision 4).
    assert!(out.stdout.starts_with("{\n  \"Name\": \"Staging\",\n"));
    assert!(out.stdout.ends_with("}\n"));
    assert_no_secret(&out);
}

#[test]
fn jsonl_prints_the_body_on_one_line() {
    let server = Server::run();
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(200).body(BODY)));

    let out = run(
        &[
            "server",
            "get",
            "--format",
            "jsonl",
            "--base-url",
            &base_url(&server),
        ],
        &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, format!("{BODY}\n"));
}

#[test]
fn global_flags_work_before_and_after_the_subcommand() {
    let server = Server::run();
    server.expect(
        expect_get("/server", "", Token::Server)
            .times(2)
            .respond_with(status_code(200).body(BODY)),
    );
    let base = base_url(&server);

    let before = run(
        &[
            "--server-token",
            SENTINEL_TOKEN,
            "--base-url",
            &base,
            "--format",
            "jsonl",
            "server",
            "get",
        ],
        &[],
    );
    let after = run(
        &[
            "server",
            "get",
            "--server-token",
            SENTINEL_TOKEN,
            "--base-url",
            &base,
            "--format",
            "jsonl",
        ],
        &[],
    );

    assert_eq!(before.code, 0, "{before:?}");
    assert_eq!(after.code, 0, "{after:?}");
    assert_eq!(before.stdout, format!("{BODY}\n"));
    assert_eq!(after.stdout, before.stdout);
}

#[test]
fn the_base_url_comes_from_the_environment_too() {
    let server = Server::run();
    server
        .expect(expect_get("/server", "", Token::Server).respond_with(status_code(200).body(BODY)));

    let out = run(
        &["server", "get"],
        &[
            ("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN),
            ("POSTMARK_BASE_URL", &base_url(&server)),
        ],
    );

    assert_eq!(out.code, 0, "{out:?}");
}

#[test]
fn help_names_the_env_var_and_never_shows_its_value() {
    let out = run(
        &["server", "get", "--help"],
        &[
            ("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN),
            ("POSTMARK_ACCOUNT_TOKEN", SENTINEL_TOKEN),
        ],
    );

    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        out.stdout.contains("POSTMARK_SERVER_TOKEN"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("POSTMARK_ACCOUNT_TOKEN"),
        "{}",
        out.stdout
    );
    assert_no_secret(&out);
}
