//! `inbound`: received messages (server token).

use crate::cli::args::{DateTimeRange, api_name};
use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum InboundCommand {
    /// Search received messages. Postmark returns only `processed` ones
    /// unless --status says otherwise.
    // Boxed: the search flags make this variant many times larger than
    // the others.
    Search(Box<SearchArgs>),
    /// Get one received message: its details and body.
    Get(GetArgs),
}

impl InboundCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            InboundCommand::Search(args) => args.as_ref(),
            InboundCommand::Get(args) => args,
        }
    }
}

/// `status` for inbound search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum InboundStatus {
    #[value(name = "blocked")]
    Blocked,
    #[value(name = "processed")]
    Processed,
    #[value(name = "queued")]
    Queued,
    #[value(name = "failed")]
    Failed,
    #[value(name = "scheduled")]
    Scheduled,
}

#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,

    /// Only messages to this recipient (sends `recipient`).
    #[arg(long, value_name = "EMAIL")]
    recipient: Option<String>,

    /// Only messages from this sender address (sends `fromemail`).
    #[arg(long, value_name = "EMAIL")]
    from_email: Option<String>,

    /// Only messages with this tag (sends `tag`).
    #[arg(long)]
    tag: Option<String>,

    /// Only messages with this subject (sends `subject`).
    #[arg(long, value_name = "TEXT")]
    subject: Option<String>,

    /// Only messages with this mailbox hash (sends `mailboxhash`).
    #[arg(long, value_name = "HASH")]
    mailbox_hash: Option<String>,

    /// Only messages with this status (sends `status`). Postmark's default is
    /// `processed`, so blocked and failed messages are hidden unless you set
    /// this.
    #[arg(long, value_enum, ignore_case = true)]
    status: Option<InboundStatus>,

    #[command(flatten)]
    dates: DateTimeRange,
}

pub(crate) static SEARCH: EndpointSpec = EndpointSpec {
    path: "/messages/inbound",
    token: TokenKind::Server,
    summary: "Search received messages. Only processed ones unless --status says otherwise.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: Some(10_000),
    },
    // The docs' response table says `Messages`; their example, and the API,
    // use `InboundMessages`.
    list_key: Some("InboundMessages"),
    example: "postmark-cli inbound search --status blocked --from-date 2026-10-01",
};

impl Leaf for SearchArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &SEARCH
    }

    fn request(&self) -> Result<GetRequest, Error> {
        let request = self
            .page
            .request(&SEARCH, &[])?
            .query_opt("recipient", self.recipient.clone())
            .query_opt("fromemail", self.from_email.clone())
            .query_opt("tag", self.tag.clone())
            .query_opt("subject", self.subject.clone())
            .query_opt("mailboxhash", self.mailbox_hash.clone())
            .query_opt("status", self.status.as_ref().map(api_name));
        Ok(self.dates.query(request))
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The message ID (`MessageID` in search results).
    message_id: String,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/messages/inbound/{messageid}/details",
    token: TokenKind::Server,
    summary: "Get one received message: its details and body.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli inbound get 041e3d29-737d-491e-9a13-a94d3ff6d0a4",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.message_id])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search() -> SearchArgs {
        SearchArgs {
            page: PageArgs {
                count: None,
                offset: 0,
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
            recipient: None,
            from_email: None,
            tag: None,
            subject: None,
            mailbox_hash: None,
            status: None,
            dates: DateTimeRange::default(),
        }
    }

    #[test]
    fn search_with_no_filters_sends_the_page_only() {
        assert_eq!(
            search().request().unwrap().path_and_query(),
            "/messages/inbound?count=50&offset=0"
        );
    }

    #[test]
    fn search_sends_every_filter_in_the_docs_order() {
        let args = SearchArgs {
            recipient: Some("a@example.com".into()),
            from_email: Some("b@example.com".into()),
            tag: Some("t".into()),
            subject: Some("s".into()),
            mailbox_hash: Some("h".into()),
            status: Some(InboundStatus::Blocked),
            dates: DateTimeRange {
                from_date: Some("2026-01-01".into()),
                to_date: Some("2026-01-02".into()),
            },
            ..search()
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/messages/inbound?count=50&offset=0&recipient=a%40example.com\
             &fromemail=b%40example.com&tag=t&subject=s&mailboxhash=h&status=blocked\
             &todate=2026-01-02&fromdate=2026-01-01"
        );
        assert!(args.paging().is_some());
    }

    #[test]
    fn get_puts_the_message_id_in_the_path() {
        let get = GetArgs {
            message_id: "abc".into(),
        };
        assert_eq!(
            get.request().unwrap().path_and_query(),
            "/messages/inbound/abc/details"
        );
        assert_eq!(get.paging(), None);
    }
}
