//! The `--all` / `--limit` loop (decision 9).
//!
//! Pages are fetched one at a time, never in parallel, so the output is
//! deterministic and the client stays polite to the API. The loop knows
//! nothing about the network or stdout: it takes a fetch function and an
//! `emit` function, so the unit tests drive it with canned pages.

use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination};
use serde_json::Value;
use std::future::Future;

/// The user's paging flags, as the pager needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    /// `--count`, when given. Otherwise a page is the endpoint's maximum.
    pub count: Option<u32>,
    pub offset: u32,
    pub all: bool,
    pub limit: Option<u32>,
}

impl Paging {
    /// `--limit` pages too: "give me N items" may take several pages.
    #[must_use]
    pub fn active(&self) -> bool {
        self.all || self.limit.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Offset { window: Option<u32> },
    Cursor,
}

/// How to walk one endpoint's pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    mode: Mode,
    list_key: &'static str,
    page_size: u32,
    offset: u32,
    limit: Option<u32>,
}

/// Why the loop stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// No more items: `TotalCount` reached, a short or empty page, or no
    /// next `PaginationKey`.
    Done,
    /// `--limit` items were emitted.
    Limit,
    /// The next page would pass the endpoint's `count + offset` window.
    Ceiling,
    /// `emit` reported that stdout is closed.
    Closed,
    /// A page has no array under the list key; this is that page.
    NoList(Value),
}

/// The loop's result.
#[derive(Debug, Clone, PartialEq)]
pub struct Paged {
    /// The first page with a list, for the merged `--all` output.
    pub first: Option<Value>,
    pub stop: Stop,
}

impl Plan {
    /// `None` when the endpoint does not page or has no list key.
    #[must_use]
    pub fn new(spec: &EndpointSpec, paging: Paging) -> Option<Plan> {
        let (mode, max_count) = match spec.pagination {
            Pagination::None => return None,
            Pagination::OffsetCount {
                max_count,
                max_window,
            } => (Mode::Offset { window: max_window }, max_count),
            Pagination::Cursor { max_count } => (Mode::Cursor, max_count),
        };
        Some(Plan {
            mode,
            list_key: spec.list_key?,
            page_size: paging.count.unwrap_or(max_count),
            offset: paging.offset,
            limit: paging.limit,
        })
    }

    #[must_use]
    pub fn list_key(&self) -> &'static str {
        self.list_key
    }

    /// The `count + offset` ceiling, for the `pagination_ceiling` warning.
    #[must_use]
    pub fn window(&self) -> Option<u32> {
        match self.mode {
            Mode::Offset { window } => window,
            Mode::Cursor => None,
        }
    }

    /// The first page's request, as `run` sends it. `--dry-run --all` prints
    /// this one.
    #[must_use]
    pub fn first_request(&self, base: &GetRequest) -> GetRequest {
        // The window cannot be reached before the first page: `PageArgs`
        // rejects a start past it.
        let size = self.next_size(self.offset, 0).unwrap_or(self.page_size);
        self.page_request(base, size, self.offset, None)
    }

    /// The `count` for the next page: the page size, cut to what `--limit`
    /// leaves and to what the window leaves. `None` at the window.
    fn next_size(&self, offset: u32, taken: u32) -> Option<u32> {
        let mut size = self.page_size;
        if let Some(limit) = self.limit {
            size = size.min(limit.saturating_sub(taken));
        }
        if let Mode::Offset {
            window: Some(window),
        } = self.mode
        {
            let room = window.saturating_sub(offset);
            if room == 0 {
                return None;
            }
            size = size.min(room);
        }
        Some(size)
    }

    fn page_request(
        &self,
        base: &GetRequest,
        size: u32,
        offset: u32,
        cursor: Option<&str>,
    ) -> GetRequest {
        let request = base.clone().set_query("count", size.to_string());
        match (self.mode, cursor) {
            (Mode::Offset { .. }, _) => request.set_query("offset", offset.to_string()),
            (Mode::Cursor, Some(key)) => request.set_query("paginationKey", key),
            (Mode::Cursor, None) => request,
        }
    }

    /// Fetches page after page and passes each item to `emit`, until a
    /// [`Stop`] condition.
    ///
    /// # Errors
    ///
    /// The first error `fetch` returns. Items emitted before it stay emitted.
    pub async fn run<F, Fut, E>(
        &self,
        base: &GetRequest,
        mut fetch: F,
        mut emit: E,
    ) -> Result<Paged, Error>
    where
        F: FnMut(GetRequest) -> Fut,
        Fut: Future<Output = Result<Value, Error>>,
        E: FnMut(&Value) -> bool,
    {
        let mut first: Option<Value> = None;
        let mut taken: u32 = 0;
        let mut offset = self.offset;
        let mut cursor: Option<String> = None;
        let stop = loop {
            let Some(size) = self.next_size(offset, taken) else {
                break Stop::Ceiling;
            };
            let body = fetch(self.page_request(base, size, offset, cursor.as_deref())).await?;
            let Some(items) = body.get(self.list_key).and_then(Value::as_array) else {
                break Stop::NoList(body);
            };
            let received = u32::try_from(items.len()).unwrap_or(u32::MAX);
            for item in items {
                if self.limit == Some(taken) {
                    break;
                }
                if !emit(item) {
                    return Ok(Paged {
                        first: first.or(Some(body)),
                        stop: Stop::Closed,
                    });
                }
                taken += 1;
            }
            let more = match self.mode {
                Mode::Offset { .. } => {
                    offset = offset.saturating_add(received);
                    let total = body.get("TotalCount").and_then(Value::as_u64);
                    received >= size && total.is_none_or(|total| u64::from(offset) < total)
                }
                Mode::Cursor => {
                    let next = body.get("PaginationKey").and_then(Value::as_str);
                    // A key that comes back unchanged would loop forever.
                    let more = received > 0 && next.is_some() && next != cursor.as_deref();
                    cursor = next.map(str::to_owned);
                    more
                }
            };
            // Before any stop: the merged json is built on page 1.
            first.get_or_insert(body);
            if self.limit == Some(taken) {
                break Stop::Limit;
            }
            if !more {
                break Stop::Done;
            }
        };
        Ok(Paged { first, stop })
    }
}

/// The `--all` output in json: the first page, with its list replaced by every
/// item and without `PaginationKey`. The keys stay in page 1's order.
#[must_use]
pub fn merged(mut first: Value, list_key: &str, items: Vec<Value>) -> Value {
    if let Value::Object(map) = &mut first {
        map.insert(list_key.to_owned(), Value::Array(items));
        map.shift_remove("PaginationKey");
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::TokenKind;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    static OFFSET: EndpointSpec = EndpointSpec {
        path: "/servers",
        token: TokenKind::Account,
        summary: "",
        pagination: Pagination::OffsetCount {
            max_count: 500,
            max_window: None,
        },
        list_key: Some("Servers"),
        example: "",
    };

    static WINDOW: EndpointSpec = EndpointSpec {
        pagination: Pagination::OffsetCount {
            max_count: 4,
            max_window: Some(10),
        },
        ..OFFSET
    };

    static CURSOR: EndpointSpec = EndpointSpec {
        path: "/email/bulk",
        pagination: Pagination::Cursor { max_count: 500 },
        list_key: Some("Requests"),
        ..OFFSET
    };

    fn paging(count: Option<u32>, offset: u32, limit: Option<u32>) -> Paging {
        Paging {
            count,
            offset,
            all: true,
            limit,
        }
    }

    fn base(spec: &EndpointSpec, count: u32) -> GetRequest {
        let request = GetRequest::new(spec, &[])
            .unwrap()
            .query("count", count.to_string());
        match spec.pagination {
            Pagination::Cursor { .. } => request,
            _ => request.query("offset", "0"),
        }
        .query("name", "x")
    }

    /// A page of `n` items numbered from `from`.
    fn page(n: u32, from: u32, total: u32) -> Value {
        let items: Vec<_> = (from..from + n).map(|id| json!({"ID": id})).collect();
        json!({"TotalCount": total, "Servers": items})
    }

    struct Outcome {
        paged: Paged,
        items: Vec<Value>,
        requests: Vec<String>,
    }

    /// Runs the plan against canned pages, with `emit` closing after
    /// `open_for` items.
    fn drive(plan: &Plan, base: &GetRequest, pages: Vec<Value>, open_for: usize) -> Outcome {
        let canned = RefCell::new(VecDeque::from(pages));
        let requests = RefCell::new(Vec::new());
        let mut items = Vec::new();
        let fetch = |request: GetRequest| {
            requests.borrow_mut().push(request.path_and_query());
            let page = canned
                .borrow_mut()
                .pop_front()
                .expect("no request past the canned pages");
            std::future::ready(Ok(page))
        };
        let emit = |item: &Value| {
            if items.len() == open_for {
                return false;
            }
            items.push(item.clone());
            true
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(plan.run(base, fetch, emit)).unwrap();
        Outcome {
            paged: result,
            items,
            requests: requests.into_inner(),
        }
    }

    fn ids(items: &[Value]) -> Vec<u64> {
        items.iter().map(|i| i["ID"].as_u64().unwrap()).collect()
    }

    #[test]
    fn three_full_pages_and_a_short_one() {
        let plan = Plan::new(&OFFSET, paging(Some(2), 0, None)).unwrap();
        let pages = vec![
            page(2, 0, 99),
            page(2, 2, 99),
            page(2, 4, 99),
            page(1, 6, 99),
        ];

        let out = drive(&plan, &base(&OFFSET, 2), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(ids(&out.items), [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            out.requests,
            [
                "/servers?count=2&offset=0&name=x",
                "/servers?count=2&offset=2&name=x",
                "/servers?count=2&offset=4&name=x",
                "/servers?count=2&offset=6&name=x",
            ]
        );
        assert_eq!(out.paged.first, Some(page(2, 0, 99)));
    }

    #[test]
    fn total_count_reached_exactly_needs_no_extra_request() {
        let plan = Plan::new(&OFFSET, paging(Some(2), 0, None)).unwrap();

        let out = drive(
            &plan,
            &base(&OFFSET, 2),
            vec![page(2, 0, 4), page(2, 2, 4)],
            usize::MAX,
        );

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(out.items.len(), 4);
        assert_eq!(out.requests.len(), 2);
    }

    #[test]
    fn a_start_offset_counts_toward_total_count() {
        let plan = Plan::new(&OFFSET, paging(Some(2), 2, None)).unwrap();

        let out = drive(&plan, &base(&OFFSET, 2), vec![page(2, 2, 4)], usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(out.requests, ["/servers?count=2&offset=2&name=x"]);
    }

    #[test]
    fn an_empty_first_page_is_done() {
        let plan = Plan::new(&OFFSET, paging(None, 0, None)).unwrap();

        let out = drive(&plan, &base(&OFFSET, 50), vec![page(0, 0, 0)], usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(out.items.len(), 0);
        assert_eq!(out.paged.first, Some(page(0, 0, 0)));
        // --all without --count asks for the endpoint's maximum.
        assert_eq!(out.requests, ["/servers?count=500&offset=0&name=x"]);
    }

    #[test]
    fn limit_cuts_a_page_to_exactly_n_items() {
        let plan = Plan::new(&OFFSET, paging(Some(5), 0, Some(7))).unwrap();
        // The fake ignores `count` and always sends 5 items.
        let pages = vec![page(5, 0, 99), page(5, 5, 99)];

        let out = drive(&plan, &base(&OFFSET, 5), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Limit);
        assert_eq!(ids(&out.items), [0, 1, 2, 3, 4, 5, 6]);
        // The last page asks only for what is left.
        assert_eq!(
            out.requests,
            [
                "/servers?count=5&offset=0&name=x",
                "/servers?count=2&offset=5&name=x"
            ]
        );
    }

    #[test]
    fn a_limit_reached_on_the_first_page_keeps_that_page() {
        let plan = Plan::new(&OFFSET, paging(None, 0, Some(2))).unwrap();

        let out = drive(&plan, &base(&OFFSET, 50), vec![page(5, 0, 99)], usize::MAX);

        assert_eq!(out.paged.stop, Stop::Limit);
        assert_eq!(ids(&out.items), [0, 1]);
        // The merged json is built on page 1; without it, `--limit` prints null.
        assert_eq!(out.paged.first, Some(page(5, 0, 99)));
    }

    #[test]
    fn a_limit_under_the_page_size_shrinks_the_first_page() {
        let plan = Plan::new(&OFFSET, paging(None, 0, Some(7))).unwrap();
        assert_eq!(
            plan.first_request(&base(&OFFSET, 50)).path_and_query(),
            "/servers?count=7&offset=0&name=x"
        );
    }

    #[test]
    fn the_window_ceiling_stops_before_a_request_past_it() {
        let plan = Plan::new(&WINDOW, paging(None, 0, None)).unwrap();
        let pages = vec![page(4, 0, 50), page(4, 4, 50), page(2, 8, 50)];

        let out = drive(&plan, &base(&WINDOW, 4), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Ceiling);
        assert_eq!(out.items.len(), 10);
        // The last page shrinks to fit the window; nothing asks past 10.
        assert_eq!(
            out.requests,
            [
                "/servers?count=4&offset=0&name=x",
                "/servers?count=4&offset=4&name=x",
                "/servers?count=2&offset=8&name=x",
            ]
        );
    }

    #[test]
    fn the_window_is_no_ceiling_when_total_count_ends_first() {
        let plan = Plan::new(&WINDOW, paging(None, 0, None)).unwrap();
        let pages = vec![page(4, 0, 10), page(4, 4, 10), page(2, 8, 10)];

        let out = drive(&plan, &base(&WINDOW, 4), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
    }

    #[test]
    fn cursor_mode_passes_each_key_back_unchanged() {
        let plan = Plan::new(&CURSOR, paging(None, 0, None)).unwrap();
        let key = "a+b/c==";
        let pages = vec![
            json!({"Requests": [{"ID": 1}], "PaginationKey": "k1"}),
            json!({"Requests": [{"ID": 2}], "PaginationKey": key}),
            json!({"Requests": [{"ID": 3}]}),
        ];

        let out = drive(&plan, &base(&CURSOR, 500), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(ids(&out.items), [1, 2, 3]);
        assert_eq!(
            out.requests,
            [
                "/email/bulk?count=500&name=x",
                "/email/bulk?count=500&name=x&paginationKey=k1",
                "/email/bulk?count=500&name=x&paginationKey=a%2Bb%2Fc%3D%3D",
            ]
        );
        // Byte for byte, once the server decodes it.
        let sent = out.requests[2].split("paginationKey=").nth(1).unwrap();
        let decoded: String = url::form_urlencoded::parse(format!("k={sent}").as_bytes())
            .next()
            .unwrap()
            .1
            .into_owned();
        assert_eq!(decoded, key);
    }

    #[test]
    fn a_cursor_that_repeats_ends_the_loop() {
        let plan = Plan::new(&CURSOR, paging(None, 0, None)).unwrap();
        let pages = vec![
            json!({"Requests": [{"ID": 1}], "PaginationKey": "same"}),
            json!({"Requests": [{"ID": 2}], "PaginationKey": "same"}),
        ];

        let out = drive(&plan, &base(&CURSOR, 500), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
        assert_eq!(out.requests.len(), 2);
    }

    #[test]
    fn an_empty_cursor_page_is_done_even_with_a_key() {
        let plan = Plan::new(&CURSOR, paging(None, 0, None)).unwrap();
        let pages = vec![json!({"Requests": [], "PaginationKey": "k"})];

        let out = drive(&plan, &base(&CURSOR, 500), pages, usize::MAX);

        assert_eq!(out.paged.stop, Stop::Done);
    }

    #[test]
    fn a_closed_stdout_stops_before_the_next_request() {
        let plan = Plan::new(&OFFSET, paging(Some(2), 0, None)).unwrap();

        let out = drive(
            &plan,
            &base(&OFFSET, 2),
            vec![page(2, 0, 99), page(2, 2, 99)],
            1,
        );

        assert_eq!(out.paged.stop, Stop::Closed);
        assert_eq!(out.requests.len(), 1);
    }

    #[test]
    fn a_page_without_the_list_key_stops_with_that_page() {
        let plan = Plan::new(&OFFSET, paging(Some(2), 0, None)).unwrap();
        let odd = json!({"TotalCount": 9, "Other": []});

        let out = drive(
            &plan,
            &base(&OFFSET, 2),
            vec![page(2, 0, 9), odd.clone()],
            usize::MAX,
        );

        assert_eq!(out.paged.stop, Stop::NoList(odd));
        assert_eq!(out.items.len(), 2);
        assert_eq!(out.paged.first, Some(page(2, 0, 9)));
    }

    #[test]
    fn a_fetch_error_ends_the_loop() {
        let plan = Plan::new(&OFFSET, paging(None, 0, None)).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let result = runtime.block_on(plan.run(
            &base(&OFFSET, 50),
            |_| std::future::ready(Err(Error::transport("down"))),
            |_| true,
        ));
        assert_eq!(result.unwrap_err().message(), "down");
    }

    #[test]
    fn only_a_paged_endpoint_with_a_list_key_has_a_plan() {
        static PLAIN: EndpointSpec = EndpointSpec {
            pagination: Pagination::None,
            ..OFFSET
        };
        static NO_KEY: EndpointSpec = EndpointSpec {
            list_key: None,
            ..OFFSET
        };
        assert!(Plan::new(&PLAIN, paging(None, 0, None)).is_none());
        assert!(Plan::new(&NO_KEY, paging(None, 0, None)).is_none());
        assert_eq!(
            Plan::new(&OFFSET, paging(None, 0, None))
                .unwrap()
                .list_key(),
            "Servers"
        );
    }

    #[test]
    fn the_merged_output_keeps_page_one_and_its_key_order() {
        let first = json!({"Requests": [{"ID": 1}], "PaginationKey": "k", "Extra": true});
        assert_eq!(
            merged(first, "Requests", vec![json!({"ID": 1}), json!({"ID": 2})]).to_string(),
            r#"{"Requests":[{"ID":1},{"ID":2}],"Extra":true}"#
        );
        let first = json!({"TotalCount": 3, "Servers": [{"ID": 1}]});
        assert_eq!(
            merged(first, "Servers", vec![json!(1), json!(2), json!(3)]).to_string(),
            r#"{"TotalCount":3,"Servers":[1,2,3]}"#
        );
    }

    #[test]
    fn the_window_is_known_for_offset_plans_only() {
        assert_eq!(
            Plan::new(&WINDOW, paging(None, 0, None)).unwrap().window(),
            Some(10)
        );
        assert_eq!(
            Plan::new(&CURSOR, paging(None, 0, None)).unwrap().window(),
            None
        );
    }

    #[test]
    fn paging_is_active_with_all_or_limit() {
        let off = Paging {
            count: None,
            offset: 0,
            all: false,
            limit: None,
        };
        assert!(!off.active());
        assert!(Paging { all: true, ..off }.active());
        assert!(
            Paging {
                limit: Some(1),
                ..off
            }
            .active()
        );
    }
}
