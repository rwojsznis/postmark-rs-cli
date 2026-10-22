//! What one endpoint is: its path, the token it needs, and how it pages.
//!
//! There is no method field. Every endpoint is GET (decision 1), so a spec
//! cannot describe anything else.

/// Which Postmark token an endpoint needs. The client gets only that one, so
/// the other token is never sent (decision 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Server,
    Account,
}

impl TokenKind {
    /// The name in `postmark-cli commands`, `--dry-run` and `--verbose`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TokenKind::Server => "server",
            TokenKind::Account => "account",
        }
    }

    /// The header postmark-rs sends the token in.
    #[must_use]
    pub fn header(self) -> &'static str {
        match self {
            TokenKind::Server => "X-Postmark-Server-Token",
            TokenKind::Account => "X-Postmark-Account-Token",
        }
    }

    /// The environment variable that supplies the token. An error for a
    /// missing token names both this and [`TokenKind::flag`].
    #[must_use]
    pub fn env_var(self) -> &'static str {
        match self {
            TokenKind::Server => "POSTMARK_SERVER_TOKEN",
            TokenKind::Account => "POSTMARK_ACCOUNT_TOKEN",
        }
    }

    /// The command-line flag that supplies the token.
    #[must_use]
    pub fn flag(self) -> &'static str {
        match self {
            TokenKind::Server => "--server-token",
            TokenKind::Account => "--account-token",
        }
    }
}

/// How an endpoint pages through a list (decision 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pagination {
    None,
    /// `count` and `offset`. `max_window` is the documented ceiling on
    /// `count + offset`, where the docs state one.
    OffsetCount {
        max_count: u32,
        max_window: Option<u32>,
    },
    /// `count` and `paginationKey`. The response has no `PaginationKey` on
    /// the last page.
    Cursor {
        max_count: u32,
    },
}

impl Pagination {
    /// The largest `count` the endpoint takes, if it pages.
    #[must_use]
    pub fn max_count(self) -> Option<u32> {
        match self {
            Pagination::None => None,
            Pagination::OffsetCount { max_count, .. } | Pagination::Cursor { max_count } => {
                Some(max_count)
            }
        }
    }

    /// The ceiling on `count + offset`, if the docs state one.
    #[must_use]
    pub fn max_window(self) -> Option<u32> {
        match self {
            Pagination::OffsetCount { max_window, .. } => max_window,
            Pagination::None | Pagination::Cursor { .. } => None,
        }
    }
}

/// One endpoint, as a leaf command declares it (decision 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointSpec {
    /// The path template, with the docs' own `{placeholder}` names.
    pub path: &'static str,
    pub token: TokenKind,
    pub summary: &'static str,
    pub pagination: Pagination,
    /// The response key that holds the list, for `--format jsonl` and `--all`.
    pub list_key: Option<&'static str>,
    /// A full command line for `--help` and the catalog. Words split on
    /// whitespace, so no quoting; a test parses every one.
    pub example: &'static str,
}

impl EndpointSpec {
    /// The `{...}` names in `path`, in order.
    #[must_use]
    pub fn placeholders(&self) -> Vec<&'static str> {
        self.path.split('/').filter_map(placeholder).collect()
    }
}

/// The name inside a `{...}` path segment. One function, so that listing the
/// placeholders and filling them cannot disagree on what one is.
pub(crate) fn placeholder(segment: &str) -> Option<&str> {
    segment.strip_prefix('{')?.strip_suffix('}')
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_path_without_placeholders_has_none() {
        assert_eq!(spec("/server").placeholders(), [] as [&str; 0]);
    }

    #[test]
    fn one_placeholder_keeps_the_docs_name() {
        assert_eq!(
            spec("/messages/outbound/{messageid}/details").placeholders(),
            ["messageid"]
        );
    }

    #[test]
    fn two_placeholders_come_in_path_order() {
        assert_eq!(
            spec("/a/{second-not-first}/b/{stream_ID}").placeholders(),
            ["second-not-first", "stream_ID"]
        );
    }

    #[test]
    fn pagination_limits() {
        let window = Pagination::OffsetCount {
            max_count: 500,
            max_window: Some(10_000),
        };
        assert_eq!(window.max_count(), Some(500));
        assert_eq!(window.max_window(), Some(10_000));
        let cursor = Pagination::Cursor { max_count: 500 };
        assert_eq!(cursor.max_count(), Some(500));
        assert_eq!(cursor.max_window(), None);
        assert_eq!(Pagination::None.max_count(), None);
        assert_eq!(Pagination::None.max_window(), None);
    }

    #[test]
    fn the_token_kinds_name_their_header_env_var_and_flag() {
        assert_eq!(TokenKind::Server.as_str(), "server");
        assert_eq!(TokenKind::Server.header(), "X-Postmark-Server-Token");
        assert_eq!(TokenKind::Server.env_var(), "POSTMARK_SERVER_TOKEN");
        assert_eq!(TokenKind::Server.flag(), "--server-token");
        assert_eq!(TokenKind::Account.as_str(), "account");
        assert_eq!(TokenKind::Account.header(), "X-Postmark-Account-Token");
        assert_eq!(TokenKind::Account.env_var(), "POSTMARK_ACCOUNT_TOKEN");
        assert_eq!(TokenKind::Account.flag(), "--account-token");
    }
}
