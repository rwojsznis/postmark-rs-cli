//! The one request type the CLI can build (decisions 1 and 2).
//!
//! `GetRequest` implements `postmark::Endpoint`, so postmark-rs sends it, but
//! its method is always GET and nothing can change that: no field holds a
//! method and no constructor takes one. Its response is `serde_json::Value`,
//! because postmark-rs's typed responses drop unknown fields and fail when
//! Postmark leaves an empty property out (decision 2).

use crate::spec::{EndpointSpec, placeholder};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use std::borrow::Cow;
use std::fmt;
use url::Url;

/// What `url::Url::path_segments_mut` encodes for an `http`/`https` URL
/// (`SPECIAL_PATH_SEGMENT` in `url`'s parser), which is how postmark-rs
/// encodes a path param. A copy, because `url` does not export it, and the
/// way to reach it through `Url` is a fallible parse of a constant URL. A
/// test holds the two equal.
const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%')
    .add(b'\\');

/// A GET request with its path already encoded and its query pairs in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRequest {
    path: String,
    query: Vec<(String, String)>,
}

/// Why a request could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    /// A spec got a different number of path params than it has
    /// placeholders. A bug in a leaf command, not a user error: clap makes
    /// every positional required, and the catalog tests build every spec. It
    /// is an error and not a panic so that a release cannot crash.
    ParamCount {
        path: &'static str,
        expected: usize,
        got: usize,
    },
    /// A path param that would change which endpoint is called. URL parsing
    /// resolves a `.` or `..` segment, even percent-encoded, and postmark-rs
    /// parses the path again when it joins the base URL. So
    /// `bounces dump ..` would call `/bounces/dump`, and an empty param
    /// would call `/bounces//dump`. No Postmark ID or alias has either form.
    BadParam { name: &'static str, value: String },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::ParamCount {
                path,
                expected,
                got,
            } => write!(f, "{path} takes {expected} path params, got {got}"),
            RequestError::BadParam { name, value } => write!(
                f,
                "{name} cannot be {value:?}: an empty, \".\" or \"..\" path segment changes the endpoint"
            ),
        }
    }
}

impl std::error::Error for RequestError {}

impl GetRequest {
    /// Fills each `{...}` in `spec.path` with the next param, encoded as one
    /// path segment: a `/` in a template alias stays inside the segment.
    ///
    /// # Errors
    ///
    /// [`RequestError::ParamCount`] when `path_params` does not match the
    /// placeholders, and [`RequestError::BadParam`] for a param that would
    /// change the endpoint.
    pub fn new(spec: &EndpointSpec, path_params: &[&str]) -> Result<Self, RequestError> {
        let count_error = || RequestError::ParamCount {
            path: spec.path,
            expected: spec.placeholders().len(),
            got: path_params.len(),
        };
        let mut params = path_params.iter();
        let mut path = String::with_capacity(spec.path.len());
        for part in spec.path.split('/').skip(1) {
            path.push('/');
            let Some(name) = placeholder(part) else {
                path.push_str(part);
                continue;
            };
            let &value = params.next().ok_or_else(count_error)?;
            if matches!(value, "" | "." | "..") {
                return Err(RequestError::BadParam {
                    name,
                    value: value.to_owned(),
                });
            }
            path.extend(utf8_percent_encode(value, PATH_SEGMENT));
        }
        if params.next().is_some() {
            return Err(count_error());
        }
        Ok(GetRequest {
            path,
            query: Vec::new(),
        })
    }

    /// Adds `key=value`.
    #[must_use]
    pub fn query(mut self, key: &str, value: impl Into<String>) -> Self {
        self.query.push((key.to_owned(), value.into()));
        self
    }

    /// Adds `key=value` when there is a value.
    #[must_use]
    pub fn query_opt(self, key: &str, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(value) => self.query(key, value),
            None => self,
        }
    }

    /// Sets `key=value`: in place when `key` is already there, so the order
    /// stays as the leaf built it; at the end otherwise. Pagination uses it
    /// to move `offset` and `paginationKey` from page to page.
    #[must_use]
    pub fn set_query(mut self, key: &str, value: impl Into<String>) -> Self {
        let value = value.into();
        match self.query.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.query.push((key.to_owned(), value)),
        }
        self
    }

    /// Adds `key=true` when `set`. A plain opt-in flag sends nothing when it is
    /// off, so the API applies its own default (decision 8).
    #[must_use]
    pub fn query_flag(self, key: &str, set: bool) -> Self {
        if set { self.query(key, "true") } else { self }
    }

    /// What postmark-rs puts after the base URL. The query is form-encoded,
    /// as postmark-rs encodes its own (`user%40example.com`, `one+two`).
    #[must_use]
    pub fn path_and_query(&self) -> String {
        if self.query.is_empty() {
            return self.path.clone();
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&self.query)
            .finish();
        format!("{}?{query}", self.path)
    }

    /// The URL that is really requested. It uses the same `join` as
    /// `PostmarkClient`, so `--dry-run`, `--verbose` and the errors show what
    /// goes on the wire.
    ///
    /// # Errors
    ///
    /// When the path does not join onto `base`, which only a `base` that
    /// cannot be a base URL causes. The CLI checks `--base-url` up front.
    pub fn url(&self, base: &Url) -> Result<Url, url::ParseError> {
        base.join(&self.path_and_query())
    }
}

impl postmark::Endpoint for GetRequest {
    type Request = ();
    type Response = serde_json::Value;

    fn endpoint(&self) -> Cow<'static, str> {
        self.path_and_query().into()
    }

    // postmark-rs sends no body for a GET; this is only here because the
    // trait asks for one.
    fn body(&self) -> &Self::Request {
        &()
    }

    // The trait's default is POST.
    fn method(&self) -> http::Method {
        http::Method::GET
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Pagination, TokenKind};
    use postmark::Endpoint;

    fn spec(path: &'static str) -> EndpointSpec {
        EndpointSpec {
            path,
            token: TokenKind::Server,
            summary: "",
            pagination: Pagination::None,
            list_key: None,
            example: "",
        }
    }

    fn path(template: &'static str, params: &[&str]) -> String {
        GetRequest::new(&spec(template), params)
            .unwrap()
            .path_and_query()
    }

    #[test]
    fn a_path_without_placeholders_is_unchanged() {
        assert_eq!(path("/server", &[]), "/server");
    }

    #[test]
    fn placeholders_are_filled_in_order() {
        assert_eq!(path("/a/{x}/b/{y}/c", &["one", "two"]), "/a/one/b/two/c");
    }

    #[test]
    fn a_wrong_number_of_params_is_an_error() {
        assert_eq!(
            GetRequest::new(&spec("/a/{x}"), &[]),
            Err(RequestError::ParamCount {
                path: "/a/{x}",
                expected: 1,
                got: 0
            })
        );
        assert_eq!(
            GetRequest::new(&spec("/server"), &["extra"])
                .unwrap_err()
                .to_string(),
            "/server takes 0 path params, got 1"
        );
    }

    #[test]
    fn a_param_is_encoded_as_one_segment() {
        let encoded = |param| path("/templates/{templateIdOrAlias}", &[param]);
        // postmark-rs's own test: get_template_encodes_alias_path_segment.
        assert_eq!(
            encoded("folder/name with space"),
            "/templates/folder%2Fname%20with%20space"
        );
        assert_eq!(encoded("a+b"), "/templates/a+b");
        assert_eq!(encoded("a@b"), "/templates/a@b");
        assert_eq!(encoded("a?b"), "/templates/a%3Fb");
        assert_eq!(encoded("a#b"), "/templates/a%23b");
        assert_eq!(encoded("100%"), "/templates/100%25");
        assert_eq!(encoded("ü"), "/templates/%C3%BC");
        assert_eq!(encoded("📬"), "/templates/%F0%9F%93%AC");
    }

    #[test]
    fn a_param_that_would_change_the_endpoint_is_an_error() {
        for value in ["", ".", ".."] {
            let err = GetRequest::new(&spec("/bounces/{bounceid}/dump"), &[value]).unwrap_err();
            assert_eq!(
                err,
                RequestError::BadParam {
                    name: "bounceid",
                    value: value.to_owned()
                }
            );
        }
        assert_eq!(
            GetRequest::new(&spec("/bounces/{bounceid}/dump"), &[".."])
                .unwrap_err()
                .to_string(),
            r#"bounceid cannot be "..": an empty, "." or ".." path segment changes the endpoint"#
        );
        // Dots inside a value are an ordinary segment.
        assert_eq!(
            path("/bounces/{bounceid}/dump", &["..."]),
            "/bounces/.../dump"
        );
        assert_eq!(path("/templates/{alias}", &["a.b"]), "/templates/a.b");
    }

    #[test]
    fn the_segment_encoding_is_the_one_postmark_rs_gets_from_url() {
        let mut values: Vec<String> = (0x20u8..0x7f).map(|b| format!("a{}b", b as char)).collect();
        values.extend(["ü", "📬", "a\u{1}b", "a\u{7f}b"].map(String::from));
        for value in values {
            let mut url = Url::parse("https://postmark.local/").unwrap();
            url.path_segments_mut()
                .unwrap()
                .clear()
                .push("x")
                .push(&value);
            assert_eq!(path("/x/{v}", &[&value]), url.path(), "{value:?}");
        }
    }

    #[test]
    fn tabs_and_newlines_are_kept_where_url_would_drop_them() {
        // The URL parser deletes them from its input, so `url` makes "a\tb"
        // into "ab". Encoded, they survive postmark-rs's `join`.
        assert_eq!(path("/x/{v}", &["a\tb\r\n"]), "/x/a%09b%0D%0A");
        let req = GetRequest::new(&spec("/x/{v}"), &["a\tb"]).unwrap();
        let base = Url::parse("https://api.postmarkapp.com/").unwrap();
        assert_eq!(req.url(&base).unwrap().path(), "/x/a%09b");
    }

    #[test]
    fn every_ascii_character_makes_a_uri_postmark_rs_accepts() {
        // postmark-rs parses the path into an `http::Uri` before it sends the
        // request. A character `url` leaves raw and `http` rejects would fail
        // there as a transport error instead.
        for byte in 0x20u8..0x7f {
            let param = format!("a{}b", byte as char);
            let p = path("/templates/{templateIdOrAlias}", &[&param]);
            assert!(p.parse::<http::Uri>().is_ok(), "{param:?} gave {p:?}");
        }
    }

    #[test]
    fn query_pairs_keep_their_order_and_form_encoding() {
        let req = GetRequest::new(&spec("/messages/outbound"), &[])
            .unwrap()
            .query("count", "50")
            .query("offset", "0")
            .query("recipient", "user@example.com")
            .query("tag", "one two")
            .query("subject", "a+b&c=d");
        assert_eq!(
            req.path_and_query(),
            "/messages/outbound?count=50&offset=0&recipient=user%40example.com&tag=one+two&subject=a%2Bb%26c%3Dd"
        );
    }

    #[test]
    fn set_query_replaces_in_place_or_appends() {
        let req = GetRequest::new(&spec("/servers"), &[])
            .unwrap()
            .query("count", "50")
            .query("offset", "0")
            .query("name", "x")
            .set_query("offset", "500")
            .set_query("paginationKey", "a+b/c=");
        assert_eq!(
            req.path_and_query(),
            "/servers?count=50&offset=500&name=x&paginationKey=a%2Bb%2Fc%3D"
        );
    }

    #[test]
    fn an_absent_option_and_an_unset_flag_add_nothing() {
        let req = GetRequest::new(&spec("/message-streams"), &[])
            .unwrap()
            .query_opt("MessageStreamType", None::<&str>)
            .query_flag("IncludeArchivedStreams", false);
        assert_eq!(req.path_and_query(), "/message-streams");

        let req = req
            .query_opt("MessageStreamType", Some("Broadcasts"))
            .query_flag("IncludeArchivedStreams", true);
        assert_eq!(
            req.path_and_query(),
            "/message-streams?MessageStreamType=Broadcasts&IncludeArchivedStreams=true"
        );
    }

    #[test]
    fn the_url_joins_the_base_as_postmark_rs_does() {
        let req = GetRequest::new(&spec("/bounces/{bounceid}"), &["42"])
            .unwrap()
            .query("x", "y z");
        let api = Url::parse("https://api.postmarkapp.com/").unwrap();
        assert_eq!(
            req.url(&api).unwrap().as_str(),
            "https://api.postmarkapp.com/bounces/42?x=y+z"
        );
        let mock = Url::parse("http://127.0.0.1:1234/").unwrap();
        assert_eq!(
            req.url(&mock).unwrap().as_str(),
            "http://127.0.0.1:1234/bounces/42?x=y+z"
        );
    }

    #[test]
    fn the_endpoint_is_get_and_its_path_and_query() {
        let req = GetRequest::new(&spec("/server"), &[])
            .unwrap()
            .query("a", "1");
        assert_eq!(req.method(), http::Method::GET);
        assert_eq!(req.endpoint(), "/server?a=1");
        assert_eq!(req.body(), &());
    }
}
