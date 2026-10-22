//! For the same inputs, our path and query match postmark-rs's request types
//! wherever postmark-rs has the endpoint (decision 2). postmark-rs is the
//! oracle for encoding, not for coverage: only the params both sides support
//! are compared.

use postmark::Endpoint;
use postmark_cli::cli::parse_request;
use std::collections::BTreeMap;

/// Our path and query for a command line, without sending anything.
fn ours(argv: &[&str]) -> (String, Vec<(String, String)>) {
    let mut words = vec!["postmark-cli"];
    words.extend_from_slice(argv);
    let (_, request) =
        parse_request(&words).unwrap_or_else(|e| panic!("{argv:?}: {}", e.message()));
    split(&request.path_and_query())
}

fn theirs(request: &impl Endpoint) -> (String, Vec<(String, String)>) {
    split(&request.endpoint())
}

fn split(path_and_query: &str) -> (String, Vec<(String, String)>) {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    let pairs = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    (path.to_owned(), pairs)
}

/// The query pairs whose key both sides send, as a multiset.
fn shared(
    ours: &[(String, String)],
    theirs: &[(String, String)],
) -> (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>) {
    let keys = |pairs: &[(String, String)]| -> Vec<String> {
        pairs.iter().map(|(k, _)| k.clone()).collect()
    };
    let (our_keys, their_keys) = (keys(ours), keys(theirs));
    let pick = |pairs: &[(String, String)], other: &[String]| {
        let mut set: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, v) in pairs.iter().filter(|(k, _)| other.contains(k)) {
            set.entry(k.clone()).or_default().push(v.clone());
        }
        set
    };
    (pick(ours, &their_keys), pick(theirs, &our_keys))
}

fn assert_parity(argv: &[&str], expected: &impl Endpoint) {
    let (our_path, our_query) = ours(argv);
    let (their_path, their_query) = theirs(expected);
    assert_eq!(our_path, their_path, "{argv:?}");
    let (a, b) = shared(&our_query, &their_query);
    assert_eq!(a, b, "{argv:?}");
}

#[test]
fn server_get() {
    assert_parity(
        &["server", "get"],
        &postmark::api::server::GetCurrentServerRequest::builder().build(),
    );
}

#[test]
fn servers_list() {
    use postmark::api::server::ListServersRequest;
    assert_parity(
        &["servers", "list"],
        &ListServersRequest::builder().count(50).offset(0).build(),
    );
    assert_parity(
        &[
            "servers", "list", "--count", "7", "--offset", "3", "--name", "x",
        ],
        &ListServersRequest::builder().count(7).offset(3).build(),
    );
}

#[test]
fn servers_get() {
    assert_parity(
        &["servers", "get", "123"],
        &postmark::api::server::GetServerRequest::builder()
            .server_id(123)
            .build(),
    );
}

/// The seed from the request core: a template alias with `/` and spaces is one
/// encoded segment on both sides. No `templates` command exists yet, so this
/// builds the request from a spec directly.
#[test]
fn a_template_alias_is_one_segment() {
    use postmark::api::templates::{GetTemplateRequest, TemplateIdOrAlias};
    use postmark_cli::request::GetRequest;
    use postmark_cli::spec::{EndpointSpec, Pagination, TokenKind};

    static SPEC: EndpointSpec = EndpointSpec {
        path: "/templates/{templateIdOrAlias}",
        token: TokenKind::Server,
        summary: "",
        pagination: Pagination::None,
        list_key: None,
        example: "",
    };
    let alias = "folder/name with space";
    let ours = GetRequest::new(&SPEC, &[alias]).unwrap();
    let theirs = GetTemplateRequest::builder()
        .id(TemplateIdOrAlias::Alias(alias.to_owned()))
        .build();
    assert_eq!(ours.endpoint(), theirs.endpoint());
}

#[test]
fn outbound_search() {
    use postmark::api::messages::OutboundSearchRequest;
    assert_parity(
        &["outbound", "search"],
        &OutboundSearchRequest::builder().count(50).offset(0).build(),
    );
    assert_parity(
        &[
            "outbound",
            "search",
            "--count",
            "20",
            "--offset",
            "40",
            "--recipient",
            "user@example.com",
            "--tag",
            "one two",
            "--message-stream",
            "outbound",
        ],
        &OutboundSearchRequest::builder()
            .count(20)
            .offset(40)
            .recipient("user@example.com".to_owned())
            .tag("one two".to_owned())
            .message_stream("outbound".to_owned())
            .build(),
    );
}

#[test]
fn outbound_get_and_dump() {
    use postmark::api::messages::{OutboundDetailsRequest, OutboundDumpRequest};
    let id = "07311c54-0687-4ab9-b034-b54b5bad88ba";
    assert_parity(
        &["outbound", "get", id],
        &OutboundDetailsRequest::new(id.to_owned()),
    );
    assert_parity(
        &["outbound", "dump", id],
        &OutboundDumpRequest::new(id.to_owned()),
    );
}

#[test]
fn inbound_search() {
    use postmark::api::messages::InboundSearchRequest;
    // Not `to`: postmark-rs sends it, the docs do not have it.
    assert_parity(
        &[
            "inbound",
            "search",
            "--count",
            "5",
            "--offset",
            "10",
            "--mailbox-hash",
            "h@sh",
            "--subject",
            "Re: hi",
        ],
        &InboundSearchRequest::builder()
            .count(5)
            .offset(10)
            .mailbox_hash("h@sh".to_owned())
            .subject("Re: hi".to_owned())
            .build(),
    );
}

#[test]
fn inbound_get() {
    use postmark::api::messages::InboundDetailsRequest;
    assert_parity(
        &["inbound", "get", "abc"],
        &InboundDetailsRequest::new("abc".to_owned()),
    );
}

#[test]
fn opens_and_clicks_lists() {
    use postmark::api::messages::{MessageClicksRequest, MessageOpensRequest};
    let argv = |kind| {
        [
            kind,
            "list",
            "--count",
            "20",
            "--offset",
            "40",
            "--recipient",
            "a@example.com",
            "--tag",
            "t",
            "--message-stream",
            "s",
        ]
    };
    assert_parity(
        &argv("opens"),
        &MessageOpensRequest::builder()
            .count(20)
            .offset(40)
            .recipient("a@example.com".to_owned())
            .tag("t".to_owned())
            .message_stream("s".to_owned())
            .build(),
    );
    assert_parity(
        &argv("clicks"),
        &MessageClicksRequest::builder()
            .count(20)
            .offset(40)
            .recipient("a@example.com".to_owned())
            .tag("t".to_owned())
            .message_stream("s".to_owned())
            .build(),
    );
}

#[test]
fn opens_and_clicks_for_one_message() {
    use postmark::api::messages::{SingleMessageClicksRequest, SingleMessageOpensRequest};
    // A plain ID: postmark-rs encodes the ID, then encodes the path again.
    assert_parity(
        &["opens", "for-message", "m1", "--count", "3"],
        &SingleMessageOpensRequest::builder()
            .message_id("m1".to_owned())
            .count(3)
            .offset(0)
            .build(),
    );
    assert_parity(
        &["clicks", "for-message", "m1"],
        &SingleMessageClicksRequest::builder()
            .message_id("m1".to_owned())
            .count(50)
            .offset(0)
            .build(),
    );
}

#[test]
fn bounces() {
    use postmark::api::bounce::{
        GetBounceDumpRequest, GetBounceRequest, GetDeliveryStatsRequest,
        ListBouncesWithFiltersRequest,
    };
    assert_parity(&["bounces", "delivery-stats"], &GetDeliveryStatsRequest {});
    assert_parity(
        &[
            "bounces",
            "list",
            "--count",
            "25",
            "--offset",
            "75",
            "--type",
            "HardBounce",
            "--inactive",
            "true",
            "--email-filter",
            "a@b",
            "--tag",
            "t t",
            "--message-id",
            "m1",
            "--message-stream",
            "outbound",
            "--from-date",
            "2026-01-01",
            "--to-date",
            "2026-01-31T12:00:00",
        ],
        &ListBouncesWithFiltersRequest::builder()
            .count(25)
            .offset(75)
            .r#type("HardBounce".to_owned())
            .inactive(true)
            .email_filter("a@b".to_owned())
            .tag("t t".to_owned())
            .message_id("m1".to_owned())
            .message_stream("outbound".to_owned())
            .from_date("2026-01-01".to_owned())
            .to_date("2026-01-31T12:00:00".to_owned())
            .build(),
    );
    assert_parity(
        &["bounces", "get", "42"],
        &GetBounceRequest::builder().bounce_id(42).build(),
    );
    assert_parity(
        &["bounces", "dump", "42"],
        &GetBounceDumpRequest::builder().bounce_id(42).build(),
    );
}

#[test]
fn templates() {
    use postmark::api::templates::{GetTemplateRequest, ListTemplatesRequest, TemplateIdOrAlias};
    assert_parity(
        &["templates", "list", "--count", "10", "--offset", "5"],
        &ListTemplatesRequest::builder().count(10).offset(5).build(),
    );
    assert_parity(
        &["templates", "get", "folder/name with space"],
        &GetTemplateRequest::builder()
            .id(TemplateIdOrAlias::Alias(
                "folder/name with space".to_owned(),
            ))
            .build(),
    );
    assert_parity(
        &["templates", "get", "123"],
        &GetTemplateRequest::builder().id(123).build(),
    );
}

#[test]
fn message_streams() {
    use postmark::api::message_streams::{
        GetMessageStreamRequest, ListMessageStreamsRequest, MessageStreamType, StreamIdOrName,
    };
    assert_parity(
        &[
            "streams",
            "list",
            "--message-stream-type",
            "Broadcasts",
            "--include-archived-streams",
        ],
        &ListMessageStreamsRequest::builder()
            .message_stream_type(MessageStreamType::Broadcasts)
            .include_archived_streams(true)
            .build(),
    );
    assert_parity(
        &["streams", "get", "outbound"],
        &GetMessageStreamRequest::builder()
            .stream_id(StreamIdOrName::StreamId("outbound".to_owned()))
            .build(),
    );
}

#[test]
fn suppressions_dump() {
    use postmark::api::message_streams::{GetSuppressionRequest, StreamIdOrName};
    // A plain stream ID only: postmark-rs builds this path with `format!` and
    // does not encode the ID. It also has none of the filter params.
    assert_parity(
        &["suppressions", "dump", "outbound"],
        &GetSuppressionRequest::builder()
            .stream_id(StreamIdOrName::StreamId("outbound".to_owned()))
            .build(),
    );
}

#[test]
fn webhooks() {
    use postmark::api::webhooks::{GetWebhookRequest, ListWebhooksRequest};
    assert_parity(
        &["webhooks", "list", "--message-stream", "outbound"],
        &ListWebhooksRequest::builder()
            .message_stream("outbound")
            .build(),
    );
    assert_parity(
        &["webhooks", "get", "1234567"],
        &GetWebhookRequest::builder().webhook_id(1_234_567).build(),
    );
}

#[test]
fn inbound_rules() {
    use postmark::api::triggers::ListInboundRuleTriggersRequest;
    assert_parity(
        &["inbound-rules", "list", "--count", "30", "--offset", "60"],
        &ListInboundRuleTriggersRequest::builder()
            .count(30)
            .offset(60)
            .build(),
    );
}

#[test]
fn bulk_get() {
    use postmark::api::bulk::GetBulkStatusRequest;
    let id = "f24af63c-533d-4b7a-ad65-4a7b3202d3a7";
    assert_parity(
        &["bulk", "get", id],
        &GetBulkStatusRequest::builder()
            .bulk_request_id(id.to_owned())
            .build(),
    );
}

/// The stats endpoints, on the path and on `tag`, `fromdate` and `todate`.
///
/// Known difference: postmark-rs sends the stream as `MessageStream`; the docs'
/// parameter table spells it `messagestream`, and we follow the docs (decision
/// 8). The two keys differ, so `shared` leaves that pair out of the comparison.
#[test]
fn stats() {
    use postmark::api::stats::*;
    let query = StatsQuery {
        tag: Some("welcome".to_owned()),
        fromdate: Some("2026-01-01".to_owned()),
        todate: Some("2026-01-31".to_owned()),
        message_stream: Some("outbound".to_owned()),
    };
    let argv = |command| {
        [
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
        ]
    };
    let q = || query.clone();
    assert_parity(
        &argv("overview"),
        &GetOutboundOverviewRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("sends"),
        &GetSentCountsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("bounces"),
        &GetBounceCountsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("spam"),
        &GetSpamComplaintsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("tracked"),
        &GetTrackedEmailCountsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("opens"),
        &GetEmailOpenCountsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("opens-platforms"),
        &GetEmailPlatformUsageRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("opens-email-clients"),
        &GetEmailClientUsageRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("clicks"),
        &GetClickCountsRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("clicks-browser-families"),
        &GetBrowserUsageRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("clicks-platforms"),
        &GetBrowserPlatformUsageRequest::builder().query(q()).build(),
    );
    assert_parity(
        &argv("clicks-location"),
        &GetClickLocationRequest::builder().query(q()).build(),
    );
}

#[test]
fn account_api() {
    use postmark::api::data_removal::GetDataRemovalStatusRequest;
    use postmark::api::domains::{GetDomainRequest, ListDomainsRequest};
    use postmark::api::signatures::{GetSignatureRequest, ListSignaturesRequest};
    assert_parity(
        &["domains", "list", "--count", "7", "--offset", "14"],
        &ListDomainsRequest::builder().count(7).offset(14).build(),
    );
    assert_parity(
        &["domains", "get", "36735"],
        &GetDomainRequest::builder().domain_id(36735).build(),
    );
    assert_parity(
        &["senders", "list", "--count", "7", "--offset", "14"],
        &ListSignaturesRequest::builder().count(7).offset(14).build(),
    );
    assert_parity(
        &["senders", "get", "1"],
        &GetSignatureRequest::builder().signature_id(1).build(),
    );
    assert_parity(
        &["data-removals", "get", "1234"],
        &GetDataRemovalStatusRequest::builder()
            .data_removal_id(1234)
            .build(),
    );
}
