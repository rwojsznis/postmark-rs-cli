//! The Account API: `domains`, `senders` and `data-removals`, all with the
//! account token only.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::Value;

fn cli(server: &Server, args: &[&str]) -> Run {
    let base = base_url(server);
    let mut all = args.to_vec();
    all.extend_from_slice(&["--base-url", &base]);
    run(
        &all,
        &[
            ("POSTMARK_ACCOUNT_TOKEN", SENTINEL_TOKEN),
            // Set, and still never sent: `expect_get` asserts its absence.
            ("POSTMARK_SERVER_TOKEN", "server-token-must-not-be-sent"),
        ],
    )
}

#[test]
fn every_account_command_sends_only_the_account_token() {
    let cases = [
        (
            &["domains", "list"][..],
            "/domains",
            "count=50&offset=0",
            r#"{"TotalCount":1,"Domains":[{"ID":36735,"Name":"example.com"}]}"#,
        ),
        (
            &["domains", "get", "36735"],
            "/domains/36735",
            "",
            r#"{"ID":36735,"DKIMVerified":true,"SPFVerified":true}"#,
        ),
        (
            &["senders", "list", "--count", "100"],
            "/senders",
            "count=100&offset=0",
            r#"{"TotalCount":1,"SenderSignatures":[{"ID":1,"EmailAddress":"a@example.com"}]}"#,
        ),
        (
            &["senders", "get", "1"],
            "/senders/1",
            "",
            r#"{"ID":1,"Confirmed":true}"#,
        ),
        (
            &["data-removals", "get", "1234"],
            "/data-removals/1234",
            "",
            r#"{"ID":1234,"Status":"Pending"}"#,
        ),
    ];
    for (args, path, query, body) in cases {
        let server = Server::run();
        server.expect(
            expect_get(path, query, Token::Account).respond_with(status_code(200).body(body)),
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
}

#[test]
fn a_server_token_alone_is_not_enough() {
    let server = Server::run();

    let out = run(
        &["domains", "list", "--base-url", &base_url(&server)],
        &[("POSTMARK_SERVER_TOKEN", SENTINEL_TOKEN)],
    );

    assert_eq!(out.code, 2, "{out:?}");
    let message = out.error()["message"].as_str().unwrap().to_owned();
    assert!(message.contains("POSTMARK_ACCOUNT_TOKEN"), "{message}");
    assert_no_secret(&out);
}

#[test]
fn ids_must_be_numbers() {
    let server = Server::run();
    for args in [
        &["domains", "get", "example.com"][..],
        &["senders", "get", "a@example.com"],
        &["data-removals", "get", "x"],
    ] {
        let out = cli(&server, args);
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
    }
}
