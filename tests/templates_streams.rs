//! `templates`, `streams` and `suppressions`.

mod common;

use common::{Run, SENTINEL_TOKEN, Token, assert_no_secret, base_url, expect_get, run};
use httptest::Server;
use httptest::responders::status_code;
use serde_json::Value;

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
fn templates_list_sends_lowercase_paging_and_its_filters() {
    passes_through(
        &[
            "templates",
            "list",
            "--template-type",
            "layout",
            "--layout-template",
            "base",
        ],
        "/templates",
        "count=50&offset=0&TemplateType=Layout&LayoutTemplate=base",
        r#"{"TotalCount":1,"Templates":[{"TemplateId":1,"Alias":"base"}]}"#,
    );
}

#[test]
fn templates_get_takes_an_id_or_an_encoded_alias() {
    passes_through(
        &["templates", "get", "12345"],
        "/templates/12345",
        "",
        r#"{"TemplateId":12345}"#,
    );
    passes_through(
        &["templates", "get", "folder/name with space"],
        "/templates/folder%2Fname%20with%20space",
        "",
        r#"{"Alias":"folder/name with space"}"#,
    );
}

#[test]
fn streams_list_sends_the_archived_flag_only_when_present() {
    let body = r#"{"MessageStreams":[{"ID":"outbound"}],"TotalCount":1}"#;
    passes_through(&["streams", "list"], "/message-streams", "", body);
    passes_through(
        &[
            "streams",
            "list",
            "--include-archived-streams",
            "--message-stream-type",
            "inbound",
        ],
        "/message-streams",
        "MessageStreamType=Inbound&IncludeArchivedStreams=true",
        body,
    );
}

#[test]
fn streams_get_returns_one_stream() {
    passes_through(
        &["streams", "get", "broadcast"],
        "/message-streams/broadcast",
        "",
        r#"{"ID":"broadcast","MessageStreamType":"Broadcasts"}"#,
    );
}

#[test]
fn suppressions_dump_sends_its_filters() {
    passes_through(
        &[
            "suppressions",
            "dump",
            "outbound",
            "--origin",
            "recipient",
            "--from-date",
            "2026-01-01",
        ],
        "/message-streams/outbound/suppressions/dump",
        "Origin=Recipient&fromdate=2026-01-01",
        r#"{"Suppressions":[{"EmailAddress":"a@example.com","SuppressionReason":"HardBounce"}]}"#,
    );
}

#[test]
fn flags_these_endpoints_do_not_take_exit_2() {
    let server = Server::run();
    for args in [
        &[
            "suppressions",
            "dump",
            "outbound",
            "--from-date",
            "2026-01-01T00:00:00",
        ][..],
        &["streams", "list", "--all"],
        &["streams", "list", "--count", "10"],
        &["suppressions", "dump", "outbound", "--offset", "1"],
        &["templates", "list", "--template-type", "html"],
    ] {
        let out = cli(&server, args);
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
        assert_eq!(out.stdout, "");
    }
}
