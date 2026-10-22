//! `bounces`: delivery stats, the bounce list, one bounce, its dump.

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

fn passes_through(args: &[&str], path: &str, query: &str, body: &str) -> Run {
    let server = Server::run();
    server.expect(
        expect_get(path, query, Token::Server).respond_with(status_code(200).body(body.to_owned())),
    );

    let out = cli(&server, args);

    assert_eq!(out.code, 0, "{args:?}: {out:?}");
    assert_eq!(out.json(), serde_json::from_str::<Value>(body).unwrap());
    assert_no_secret(&out);
    out
}

const STATS: &str = r#"{"InactiveMails":192,"Bounces":[{"Name":"All","Count":253},{"Type":"HardBounce","Name":"Hard bounce","Count":195}]}"#;

#[test]
fn delivery_stats_passes_the_body_through() {
    passes_through(&["bounces", "delivery-stats"], "/deliverystats", "", STATS);
}

#[test]
fn delivery_stats_in_jsonl_is_the_whole_body_on_one_line() {
    let server = Server::run();
    server.expect(
        expect_get("/deliverystats", "", Token::Server).respond_with(status_code(200).body(STATS)),
    );

    let out = cli(&server, &["bounces", "delivery-stats", "--format", "jsonl"]);

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, format!("{STATS}\n"));
    assert_eq!(
        out.stderr, "",
        "no list_key_missing warning: it has no list key"
    );
}

#[test]
fn list_sends_its_filters() {
    passes_through(
        &[
            "bounces",
            "list",
            "--type",
            "hardbounce",
            "--inactive",
            "false",
            "--email-filter",
            "example.com",
            "--message-id",
            "m1",
            "--from-date",
            "2026-10-01",
        ],
        "/bounces",
        "count=50&offset=0&type=HardBounce&inactive=false&emailFilter=example.com\
         &messageID=m1&fromdate=2026-10-01",
        r#"{"TotalCount":1,"Bounces":[{"ID":42,"Type":"HardBounce"}]}"#,
    );
}

#[test]
fn list_without_inactive_sends_no_inactive() {
    passes_through(
        &["bounces", "list", "--type", "SMTPAPIERROR"],
        "/bounces",
        "count=50&offset=0&type=SMTPApiError",
        r#"{"TotalCount":0,"Bounces":[]}"#,
    );
}

#[test]
fn get_and_dump_take_the_bounce_id() {
    passes_through(
        &["bounces", "get", "42"],
        "/bounces/42",
        "",
        r#"{"ID":42,"Content":"smtp; 550 5.1.1"}"#,
    );
    passes_through(
        &["bounces", "dump", "42"],
        "/bounces/42/dump",
        "",
        r#"{"Body":""}"#,
    );
}

#[test]
fn bad_values_exit_2_before_any_request() {
    let server = Server::run();
    for args in [
        &["bounces", "get", "abc"][..],
        &["bounces", "dump", "-1"],
        &["bounces", "list", "--type", "hard"],
        &["bounces", "list", "--inactive", "yes"],
        &["bounces", "list", "--inactive"],
    ] {
        let out = cli(&server, args);
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
        assert_eq!(out.stdout, "");
    }
}
