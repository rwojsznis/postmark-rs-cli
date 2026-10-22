//! The paging flags (decision 9). A command with offset pagination flattens
//! `PageArgs`, one with cursor pagination `CursorArgs`; every paginated
//! command flattens `AllArgs`.

use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::EndpointSpec;
use clap::Args;

/// The `count` of one page when `--count` is not given and `--all` is not on.
pub(crate) const DEFAULT_COUNT: u32 = 50;

#[derive(Debug, Args)]
pub(crate) struct PageArgs {
    /// Items per page (sends `count`). Default 50; with --all or --limit, the
    /// endpoint's maximum.
    #[arg(long, value_name = "N")]
    pub(crate) count: Option<u32>,

    /// Items to skip (sends `offset`).
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub(crate) offset: u32,
}

#[derive(Debug, Args)]
pub(crate) struct AllArgs {
    /// Fetch every page, one after another, and print them as one response
    /// (json) or as one line per item (jsonl). With --dry-run, prints only the
    /// first page's request.
    #[arg(long)]
    pub(crate) all: bool,

    /// Stop after N items in total, fetching pages as needed.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub(crate) limit: Option<u32>,
}

impl PageArgs {
    /// The first page's request: `count` and `offset` first, checked against
    /// the endpoint's limits before anything is sent.
    pub(crate) fn request(
        &self,
        spec: &EndpointSpec,
        path_params: &[&str],
    ) -> Result<GetRequest, Error> {
        self.validate(spec)?;
        Ok(GetRequest::new(spec, path_params)?
            .query("count", self.count.unwrap_or(DEFAULT_COUNT).to_string())
            .query("offset", self.offset.to_string()))
    }

    /// `1 <= count <= max_count`, and `count + offset <= max_window` where the
    /// docs state a window. A breach is a `config` error, exit 2.
    pub(crate) fn validate(&self, spec: &EndpointSpec) -> Result<(), Error> {
        let count = validate_count(self.count, spec)?;
        let reach = u64::from(count) + u64::from(self.offset);
        if let Some(window) = spec.pagination.max_window()
            && reach > u64::from(window)
        {
            return Err(Error::config(format!(
                "--count plus --offset is {reach}, over the {window} items Postmark lets this \
                 search reach; narrow the search (with --from-date and --to-date, where the \
                 command has them) to see further"
            )));
        }
        Ok(())
    }

    /// What the pager needs, when `--all` or `--limit` is on.
    pub(crate) fn paging(&self, all: &AllArgs) -> Paging {
        Paging {
            count: self.count,
            offset: self.offset,
            all: all.all,
            limit: all.limit,
        }
    }
}

/// `1 <= count <= max_count`. Returns the count to send.
fn validate_count(count: Option<u32>, spec: &EndpointSpec) -> Result<u32, Error> {
    let count = count.unwrap_or(DEFAULT_COUNT);
    if count == 0 {
        return Err(Error::config("--count must be at least 1"));
    }
    if let Some(max) = spec.pagination.max_count()
        && count > max
    {
        return Err(Error::config(format!(
            "--count {count} is over this endpoint's maximum of {max}"
        )));
    }
    Ok(count)
}

#[derive(Debug, Args)]
pub(crate) struct CursorArgs {
    /// Items per page (sends `count`). Default 50; with --all or --limit, the
    /// endpoint's maximum.
    #[arg(long, value_name = "N")]
    pub(crate) count: Option<u32>,

    /// Start at this page: the `PaginationKey` of the previous response,
    /// exactly as received (sends `paginationKey`).
    #[arg(long, value_name = "KEY")]
    pub(crate) pagination_key: Option<String>,
}

impl CursorArgs {
    /// The first page's request: `count`, then `paginationKey` when given.
    pub(crate) fn request(
        &self,
        spec: &EndpointSpec,
        path_params: &[&str],
    ) -> Result<GetRequest, Error> {
        let count = validate_count(self.count, spec)?;
        Ok(GetRequest::new(spec, path_params)?
            .query("count", count.to_string())
            .query_opt("paginationKey", self.pagination_key.clone()))
    }

    /// What the pager needs, when `--all` or `--limit` is on.
    pub(crate) fn paging(&self, all: &AllArgs) -> Paging {
        Paging {
            count: self.count,
            offset: 0,
            all: all.all,
            limit: all.limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Pagination, TokenKind};

    static WINDOW: EndpointSpec = EndpointSpec {
        path: "/bounces",
        token: TokenKind::Server,
        summary: "",
        pagination: Pagination::OffsetCount {
            max_count: 500,
            max_window: Some(10_000),
        },
        list_key: Some("Bounces"),
        example: "",
    };

    fn page(count: Option<u32>, offset: u32) -> PageArgs {
        PageArgs { count, offset }
    }

    fn message(count: Option<u32>, offset: u32) -> Option<String> {
        page(count, offset)
            .validate(&WINDOW)
            .err()
            .map(|e| e.message())
    }

    #[test]
    fn count_is_between_one_and_the_endpoint_maximum() {
        assert_eq!(
            message(Some(0), 0).as_deref(),
            Some("--count must be at least 1")
        );
        assert_eq!(message(Some(1), 0), None);
        assert_eq!(message(Some(500), 0), None);
        assert_eq!(
            message(Some(501), 0).as_deref(),
            Some("--count 501 is over this endpoint's maximum of 500")
        );
    }

    #[test]
    fn count_plus_offset_stays_inside_the_window() {
        assert_eq!(message(Some(500), 9_500), None);
        let breach = message(Some(500), 9_501).unwrap();
        assert!(
            breach.starts_with("--count plus --offset is 10001, over the 10000"),
            "{breach}"
        );
        assert!(breach.contains("--from-date"), "{breach}");
        // The default count counts too.
        assert_eq!(message(None, 9_950), None);
        assert!(message(None, 9_951).is_some());
        // No overflow at the top of u32.
        assert!(message(Some(500), u32::MAX).is_some());
    }

    #[test]
    fn without_a_window_only_the_count_is_checked() {
        static NO_WINDOW: EndpointSpec = EndpointSpec {
            pagination: Pagination::OffsetCount {
                max_count: 500,
                max_window: None,
            },
            ..WINDOW
        };
        assert!(page(Some(500), 1_000_000).validate(&NO_WINDOW).is_ok());
    }

    #[test]
    fn a_cursor_request_sends_count_and_the_key_unchanged() {
        static CURSOR: EndpointSpec = EndpointSpec {
            pagination: Pagination::Cursor { max_count: 500 },
            ..WINDOW
        };
        let cursor = |count, key: Option<&str>| CursorArgs {
            count,
            pagination_key: key.map(String::from),
        };
        assert_eq!(
            cursor(None, None)
                .request(&CURSOR, &[])
                .unwrap()
                .path_and_query(),
            "/bounces?count=50"
        );
        assert_eq!(
            cursor(Some(20), Some("a+b/c= d"))
                .request(&CURSOR, &[])
                .unwrap()
                .path_and_query(),
            "/bounces?count=20&paginationKey=a%2Bb%2Fc%3D+d"
        );
        assert!(cursor(Some(501), None).request(&CURSOR, &[]).is_err());
        let all = AllArgs {
            all: true,
            limit: None,
        };
        assert_eq!(cursor(Some(5), None).paging(&all).count, Some(5));
    }

    #[test]
    fn the_request_starts_with_count_and_offset() {
        let request = page(None, 0).request(&WINDOW, &[]).unwrap();
        assert_eq!(request.path_and_query(), "/bounces?count=50&offset=0");
        let request = page(Some(7), 3).request(&WINDOW, &[]).unwrap();
        assert_eq!(request.path_and_query(), "/bounces?count=7&offset=3");
        assert!(page(Some(501), 0).request(&WINDOW, &[]).is_err());
    }
}
