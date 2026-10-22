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
