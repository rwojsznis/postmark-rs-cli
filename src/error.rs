//! Errors, their exit codes, and the one JSON line that reports them
//! (decision 5).
//!
//! An agent branches on `kind` and the exit code, so the mapping from a
//! failure to a kind is the contract: `api` means Postmark answered, so a retry
//! gets the same answer; `transport`, `timeout` and `decode` mean no usable
//! answer came back; `config` means nothing was sent.

use crate::cli::exit::ExitStatus;
use crate::request::RequestError;
use crate::spec::TokenKind;
use serde_json::{Map, Value, json};
use url::Url;

/// The longest non-JSON error body kept in the error line. An HTML error page
/// from a proxy is noise after the first few lines.
const BODY_LIMIT: usize = 4096;

/// What went wrong, and the request it went wrong for.
///
/// Boxed, because every fallible function returns it, and the unboxed size (a
/// `Url` and a JSON body) would make each `Result` that large.
#[derive(Debug)]
pub struct Error(Box<Inner>);

#[derive(Debug)]
struct Inner {
    kind: ErrorKind,
    /// Set whenever a request was built, so that the error line says what
    /// was asked for.
    request: Option<Url>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// Postmark answered with a non-2xx status.
    Api {
        status: u16,
        error_code: Option<i64>,
        message: Option<String>,
        /// What to do about a known error code, for an agent to act on.
        hint: Option<String>,
        /// The parsed body when it is JSON, otherwise the text, cut to 4 KiB.
        body: Value,
    },
    /// The command line or the configuration. Nothing was sent.
    Config(String),
    /// Connect, TLS, or any other failure before a response came back.
    Transport(String),
    /// No response within `--timeout`.
    Timeout { seconds: u64 },
    /// A 2xx response whose body is not JSON.
    Decode(String),
}

impl Error {
    #[must_use]
    pub fn new(kind: ErrorKind) -> Self {
        Error(Box::new(Inner {
            kind,
            request: None,
        }))
    }

    #[must_use]
    pub fn config(message: impl Into<String>) -> Self {
        Error::new(ErrorKind::Config(message.into()))
    }

    #[must_use]
    pub fn transport(message: impl Into<String>) -> Self {
        Error::new(ErrorKind::Transport(message.into()))
    }

    /// An API error from the raw response body, with the hint for its error
    /// code. `token` is the token the endpoint needs, which the token hints
    /// name.
    #[must_use]
    pub fn api(
        status: u16,
        error_code: Option<i64>,
        message: Option<String>,
        body: &[u8],
        token: TokenKind,
    ) -> Self {
        Error::new(ErrorKind::Api {
            status,
            error_code,
            message,
            hint: hint(status, error_code, token),
            body: body_value(body),
        })
    }

    /// Records the request this error belongs to.
    #[must_use]
    pub fn at(mut self, url: Url) -> Self {
        self.0.request = Some(url);
        self
    }

    #[must_use]
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }

    /// The `kind` in the JSON line.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self.0.kind {
            ErrorKind::Api { .. } => "api",
            ErrorKind::Config(_) => "config",
            ErrorKind::Transport(_) => "transport",
            ErrorKind::Timeout { .. } => "timeout",
            ErrorKind::Decode(_) => "decode",
        }
    }

    #[must_use]
    pub fn exit_status(&self) -> ExitStatus {
        match self.0.kind {
            ErrorKind::Api { .. } => ExitStatus::Api,
            ErrorKind::Config(_) => ExitStatus::Usage,
            ErrorKind::Transport(_) | ErrorKind::Timeout { .. } | ErrorKind::Decode(_) => {
                ExitStatus::Transport
            }
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        match &self.0.kind {
            ErrorKind::Api {
                status,
                message: Some(message),
                ..
            } => format!("Postmark returned {status}: {message}"),
            ErrorKind::Api { status, .. } => format!("Postmark returned {status}"),
            ErrorKind::Config(message)
            | ErrorKind::Transport(message)
            | ErrorKind::Decode(message) => message.clone(),
            ErrorKind::Timeout { seconds } => format!("no response within {seconds} s"),
        }
    }

    /// `{"error": {...}}`, in the key order of decision 5.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut error = Map::new();
        error.insert("kind".into(), self.kind_name().into());
        error.insert("message".into(), self.message().into());
        if let ErrorKind::Api {
            status,
            error_code,
            hint,
            body,
            ..
        } = &self.0.kind
        {
            if let Some(hint) = hint {
                error.insert("hint".into(), hint.clone().into());
            }
            error.insert("status".into(), (*status).into());
            error.insert("error_code".into(), (*error_code).into());
            error.insert("body".into(), body.clone());
        }
        if let Some(url) = &self.0.request {
            error.insert(
                "request".into(),
                json!({"method": "GET", "url": url.as_str()}),
            );
        }
        json!({ "error": error })
    }
}

impl From<RequestError> for Error {
    fn from(error: RequestError) -> Self {
        Error::config(error.to_string())
    }
}

/// What to do about the Postmark error codes an agent can act on (the Gotchas
/// of the API reference). One arm per code; anything else gets no hint.
fn hint(status: u16, error_code: Option<i64>, token: TokenKind) -> Option<String> {
    let hint = match (error_code, status) {
        (Some(10), _) | (None, 401) => {
            return Some(format!(
                "Check {} (or {}): this command needs the {} token, and the one sent is \
                 missing, not valid, or the other kind of token.",
                token.env_var(),
                token.flag(),
                token.as_str()
            ));
        }
        (Some(12), _) => {
            "The bulk request does not exist, or it belongs to another server: check the ID \
             and the server token."
        }
        (Some(13), _) => {
            "Pass PaginationKey back exactly as Postmark sent it: do not change, decode or pad it."
        }
        (Some(14), _) => "The account is not approved for the Bulk API: ask Postmark support.",
        (Some(600 | 800), _) => "Use --count 500 or less.",
        (Some(614), _) => {
            "The account's plan does not include this feature (the Stats API, for example), \
             or a limit is reached."
        }
        (Some(1500), _) => "--from-date cannot be more than one year ago.",
        _ => return None,
    };
    Some(hint.to_owned())
}

fn body_value(body: &[u8]) -> Value {
    if let Ok(value) = serde_json::from_slice(body) {
        return value;
    }
    let text = String::from_utf8_lossy(body);
    let mut end = text.len().min(BODY_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Value::String(text[..end].to_owned())
}

/// An error and its sources, joined with `": "`. A source whose text the
/// message already holds is skipped: postmark-rs's errors repeat their
/// source in their own `Display`.
#[must_use]
pub fn describe(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> Url {
        Url::parse("https://api.postmarkapp.com/server").unwrap()
    }

    #[test]
    fn each_kind_has_the_exit_code_of_decision_5() {
        let cases = [
            (
                Error::api(500, None, None, b"", TokenKind::Server),
                "api",
                ExitStatus::Api,
            ),
            (Error::config("x"), "config", ExitStatus::Usage),
            (Error::transport("x"), "transport", ExitStatus::Transport),
            (
                Error::new(ErrorKind::Timeout { seconds: 1 }),
                "timeout",
                ExitStatus::Transport,
            ),
            (
                Error::new(ErrorKind::Decode("x".into())),
                "decode",
                ExitStatus::Transport,
            ),
        ];
        for (error, name, status) in cases {
            assert_eq!(error.kind_name(), name);
            assert_eq!(error.exit_status(), status, "{name}");
        }
    }

    #[test]
    fn an_api_error_line_has_status_code_body_and_request() {
        let error = Error::api(
            422,
            Some(701),
            Some("Message not found".into()),
            br#"{"ErrorCode":701,"Message":"Message not found"}"#,
            TokenKind::Server,
        )
        .at(url());
        assert_eq!(
            error.to_json().to_string(),
            r#"{"error":{"kind":"api","message":"Postmark returned 422: Message not found","status":422,"error_code":701,"body":{"ErrorCode":701,"Message":"Message not found"},"request":{"method":"GET","url":"https://api.postmarkapp.com/server"}}}"#
        );
    }

    #[test]
    fn other_kinds_have_no_status_and_request_only_when_one_was_built() {
        assert_eq!(
            Error::config("bad").to_json(),
            json!({"error": {"kind": "config", "message": "bad"}})
        );
        assert_eq!(
            Error::new(ErrorKind::Timeout { seconds: 30 })
                .at(url())
                .to_json(),
            json!({"error": {
                "kind": "timeout",
                "message": "no response within 30 s",
                "request": {"method": "GET", "url": "https://api.postmarkapp.com/server"}
            }})
        );
    }

    #[test]
    fn known_error_codes_have_a_hint() {
        let hint_for = |status, code, token| {
            let error = Error::api(status, code, None, b"", token);
            error.to_json()["error"]["hint"].as_str().map(String::from)
        };
        let server = hint_for(401, Some(10), TokenKind::Server).unwrap();
        assert!(server.contains("POSTMARK_SERVER_TOKEN"), "{server}");
        assert!(server.contains("--server-token"), "{server}");
        let account = hint_for(401, Some(10), TokenKind::Account).unwrap();
        assert!(account.contains("POSTMARK_ACCOUNT_TOKEN"), "{account}");
        // A 401 without a code is the same advice.
        assert_eq!(hint_for(401, None, TokenKind::Server), Some(server));
        let cases = [
            (404, 12, "another server"),
            (422, 13, "exactly as Postmark sent it"),
            (422, 14, "Bulk API"),
            (422, 600, "--count 500"),
            (422, 800, "--count 500"),
            (422, 614, "plan"),
            (422, 1500, "one year"),
        ];
        for (status, code, needle) in cases {
            let hint = hint_for(status, Some(code), TokenKind::Server).unwrap();
            assert!(hint.contains(needle), "{code}: {hint}");
        }
        assert_eq!(hint_for(422, Some(300), TokenKind::Server), None);
        assert_eq!(hint_for(500, None, TokenKind::Server), None);
    }

    #[test]
    fn a_body_that_is_not_json_is_cut_on_a_char_boundary() {
        // 3-byte characters: 4096 is not a boundary, 4095 is.
        let body = "€".repeat(2000);
        let kept = body_value(body.as_bytes());
        assert_eq!(kept.as_str().unwrap().len(), 4095);
        assert_eq!(body_value(b"short"), json!("short"));
        assert_eq!(body_value(b"\xff"), json!("\u{fffd}"));
    }

    #[test]
    fn a_request_error_is_a_config_error() {
        let error: Error = RequestError::BadParam {
            name: "bounceid",
            value: "..".into(),
        }
        .into();
        assert_eq!(error.kind_name(), "config");
        assert!(error.message().starts_with("bounceid cannot be"));
    }

    #[test]
    fn describe_joins_the_sources_without_repeating_them() {
        #[derive(Debug)]
        struct Layer(&'static str, Option<Box<Layer>>);
        impl std::fmt::Display for Layer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.0)
            }
        }
        impl std::error::Error for Layer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.1.as_deref().map(|l| l as _)
            }
        }
        let error = Layer(
            "communication: connect failed",
            Some(Box::new(Layer(
                "connect failed",
                Some(Box::new(Layer("Connection refused", None))),
            ))),
        );
        assert_eq!(
            describe(&error),
            "communication: connect failed: Connection refused"
        );
    }
}
