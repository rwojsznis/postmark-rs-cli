//! Sends one `GetRequest` through postmark-rs, with one token and a timeout.

use crate::errln;
use crate::error::{Error, ErrorKind, describe};
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, TokenKind};
use postmark::reqwest::{PostmarkClient, PostmarkClientError};
use postmark::{Query, QueryError};
use serde_json::{Value, json};
use std::fmt;
use std::time::{Duration, Instant};
use url::Url;

/// Checks `--base-url` once, before anything is sent.
///
/// # Errors
///
/// A `config` error when the value does not parse, or its scheme is not http
/// or https.
pub fn parse_base_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value)
        .map_err(|e| Error::config(format!("--base-url {value:?} is not a URL: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::config(format!(
            "--base-url {value:?} must be an http or https URL"
        )));
    }
    Ok(url)
}

/// Everything a request needs besides the request itself.
pub struct Executor {
    pub base_url: Url,
    pub timeout: Duration,
    pub server_token: Option<String>,
    pub account_token: Option<String>,
    /// Logs each request and its outcome on stderr (decision 7).
    pub verbose: bool,
}

// By hand, so that a debug print can never show a token (decision 6).
impl fmt::Debug for Executor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redact = |token: &Option<String>| token.as_ref().map(|_| "***");
        f.debug_struct("Executor")
            .field("base_url", &self.base_url.as_str())
            .field("timeout", &self.timeout)
            .field("server_token", &redact(&self.server_token))
            .field("account_token", &redact(&self.account_token))
            .field("verbose", &self.verbose)
            .finish()
    }
}

impl Executor {
    /// Sends `request` with the one token `spec` needs, so the other token
    /// never leaves the machine (decision 6).
    ///
    /// # Errors
    ///
    /// Every kind of decision 5. The error carries the request URL.
    pub async fn get(&self, spec: &EndpointSpec, request: GetRequest) -> Result<Value, Error> {
        let url = request
            .url(&self.base_url)
            .map_err(|e| Error::config(format!("cannot build the request URL: {e}")))?;
        let token = self.token(spec.token).map_err(|e| e.at(url.clone()))?;
        let base_url = self.base_url.as_str();
        let client = match spec.token {
            TokenKind::Server => PostmarkClient::builder()
                .base_url(base_url)
                .server_token(token)
                .build(),
            TokenKind::Account => PostmarkClient::builder()
                .base_url(base_url)
                .account_token(token)
                .build(),
        };

        if self.verbose {
            errln!(
                "{}",
                json!({"request": {"method": "GET", "url": url.as_str(), "token": spec.token.as_str()}})
            );
        }
        let started = Instant::now();
        let result = match tokio::time::timeout(self.timeout, request.execute(&client)).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(from_query_error(error)),
            Err(_) => Err(Error::new(ErrorKind::Timeout {
                seconds: self.timeout.as_secs(),
            })),
        };
        if self.verbose {
            let status = match &result {
                Ok(_) => json!("ok"),
                Err(error) => match error.kind() {
                    ErrorKind::Api { status, .. } => json!(status),
                    _ => json!(error.kind_name()),
                },
            };
            let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            errln!("{}", json!({"response": {"status": status, "ms": ms}}));
        }
        result.map_err(|e| e.at(url))
    }

    fn token(&self, kind: TokenKind) -> Result<&str, Error> {
        let (env, flag) = (kind.env_var(), kind.flag());
        let token = match kind {
            TokenKind::Server => &self.server_token,
            TokenKind::Account => &self.account_token,
        };
        let token = token
            .as_deref()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                Error::config(format!(
                    "this command needs the {} token: set {env} or pass {flag}",
                    kind.as_str()
                ))
            })?;
        // postmark-rs would fail on it later, as a transport error. It is the
        // user's input, so say so now, without echoing it.
        if http::HeaderValue::from_str(token).is_err() {
            return Err(Error::config(format!(
                "the {} token from {env} or {flag} is not a valid HTTP header value",
                kind.as_str()
            )));
        }
        Ok(token)
    }
}

fn from_query_error(error: QueryError<PostmarkClientError>) -> Error {
    match error {
        QueryError::Api {
            status,
            error_code,
            message,
            body,
        } => Error::api(status.as_u16(), error_code, message, &body),
        // reqwest's messages name the URL, never the headers, so no token.
        QueryError::Client { source } => Error::transport(describe(&source)),
        QueryError::Body { source } => Error::transport(describe(&source)),
        QueryError::Json { source } => Error::new(ErrorKind::Decode(describe(&source))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executor(server: Option<&str>, account: Option<&str>) -> Executor {
        Executor {
            base_url: Url::parse("https://api.postmarkapp.com/").unwrap(),
            timeout: Duration::from_secs(30),
            server_token: server.map(String::from),
            account_token: account.map(String::from),
            verbose: false,
        }
    }

    #[test]
    fn the_base_url_must_be_http_or_https() {
        assert!(parse_base_url("http://127.0.0.1:1/").is_ok());
        assert!(parse_base_url("https://api.postmarkapp.com/").is_ok());
        for bad in ["ftp://x", "not a url", "mailto:a@b"] {
            let error = parse_base_url(bad).unwrap_err();
            assert_eq!(error.kind_name(), "config", "{bad}");
        }
    }

    #[test]
    fn each_kind_reads_its_own_token() {
        let both = executor(Some("s"), Some("a"));
        assert_eq!(both.token(TokenKind::Server).unwrap(), "s");
        assert_eq!(both.token(TokenKind::Account).unwrap(), "a");
        let error = executor(Some("s"), None)
            .token(TokenKind::Account)
            .unwrap_err();
        assert_eq!(
            error.message(),
            "this command needs the account token: set POSTMARK_ACCOUNT_TOKEN or pass --account-token"
        );
    }

    #[test]
    fn debug_never_shows_a_token() {
        let debug = format!("{:?}", executor(Some("secret-s"), Some("secret-a")));
        assert!(!debug.contains("secret"), "{debug}");
        assert!(debug.contains("***"), "{debug}");
    }

    #[test]
    fn an_account_endpoint_sends_only_the_account_token() {
        use crate::spec::Pagination;
        use httptest::matchers::{all_of, contains, key, not, request};
        use httptest::responders::status_code;
        use httptest::{Expectation, Server};

        static SPEC: EndpointSpec = EndpointSpec {
            path: "/servers/{serverid}",
            token: TokenKind::Account,
            summary: "",
            pagination: Pagination::None,
            list_key: None,
            example: "",
        };
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of![
                request::method_path("GET", "/servers/7"),
                request::headers(contains(("x-postmark-account-token", "a"))),
                request::headers(not(contains(key("x-postmark-server-token")))),
            ])
            .respond_with(status_code(200).body(r#"{"ID":7}"#)),
        );
        let mut executor = executor(Some("s"), Some("a"));
        executor.base_url = Url::parse(&server.url_str("/")).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let value = runtime
            .block_on(executor.get(&SPEC, GetRequest::new(&SPEC, &["7"]).unwrap()))
            .unwrap();

        assert_eq!(value, json!({"ID": 7}));
    }

    #[test]
    fn a_body_error_is_a_transport_error() {
        let source = http::Request::builder().uri("\n").body(()).unwrap_err();
        let error = from_query_error(QueryError::Body { source });
        assert_eq!(error.kind_name(), "transport");
    }
}
