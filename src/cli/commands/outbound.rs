//! `outbound`: sent messages (server token).

use crate::cli::args::{DateTimeRange, api_name, key_value};
use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum OutboundCommand {
    /// Search sent messages.
    // Boxed: the search flags make this variant many times larger than
    // the others.
    Search(Box<SearchArgs>),
    /// Get one sent message: its details, events and body.
    Get(GetArgs),
    /// Get the raw source of one sent message.
    Dump(DumpArgs),
}

impl OutboundCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            OutboundCommand::Search(args) => args.as_ref(),
            OutboundCommand::Get(args) => args,
            OutboundCommand::Dump(args) => args,
        }
    }
}

/// `status` for outbound search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutboundStatus {
    #[value(name = "queued")]
    Queued,
    /// The same results as `processed`.
    #[value(name = "sent")]
    Sent,
    /// The same results as `sent`.
    #[value(name = "processed")]
    Processed,
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

    /// Only messages with this status (sends `status`).
    #[arg(long, value_enum, ignore_case = true)]
    status: Option<OutboundStatus>,

    #[command(flatten)]
    dates: DateTimeRange,

    /// Only messages with this subject (sends `subject`).
    #[arg(long, value_name = "TEXT")]
    subject: Option<String>,

    // The docs say the default is `outbound`; the live API searches every
    // stream.
    /// Only messages in this stream (sends `messagestream`). Left out, the
    /// search covers all streams.
    #[arg(long, value_name = "ID")]
    message_stream: Option<String>,

    /// Only messages with this metadata value (sends `metadata_KEY=VALUE`).
    /// One key per search: an API limit.
    #[arg(long, value_name = "KEY=VALUE", value_parser = key_value)]
    metadata: Option<(String, String)>,
}

pub(crate) static SEARCH: EndpointSpec = EndpointSpec {
    path: "/messages/outbound",
    token: TokenKind::Server,
    summary: "Search sent messages.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: Some(10_000),
    },
    list_key: Some("Messages"),
    example: "postmark-cli outbound search --recipient john@example.com --from-date 2026-10-01 --count 10",
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
            .query_opt("status", self.status.as_ref().map(api_name));
        let request = self
            .dates
            .query(request)
            .query_opt("subject", self.subject.clone())
            .query_opt("messagestream", self.message_stream.clone());
        Ok(match &self.metadata {
            Some((key, value)) => request.query(&format!("metadata_{key}"), value.clone()),
            None => request,
        })
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
    path: "/messages/outbound/{messageid}/details",
    token: TokenKind::Server,
    summary: "Get one sent message: its details, events and body.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli outbound get 07311c54-0687-4ab9-b034-b54b5bad88ba",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.message_id])?)
    }
}

#[derive(Debug, Args)]
pub(crate) struct DumpArgs {
    /// The message ID (`MessageID` in search results).
    message_id: String,
}

pub(crate) static DUMP: EndpointSpec = EndpointSpec {
    path: "/messages/outbound/{messageid}/dump",
    token: TokenKind::Server,
    summary: "Get the raw source of one sent message. An empty Body means no dump is kept.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli outbound dump 07311c54-0687-4ab9-b034-b54b5bad88ba",
};

impl Leaf for DumpArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &DUMP
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&DUMP, &[&self.message_id])?)
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
            status: None,
            dates: DateTimeRange::default(),
            subject: None,
            message_stream: None,
            metadata: None,
        }
    }

    #[test]
    fn search_with_no_filters_sends_the_page_only() {
        assert_eq!(
            search().request().unwrap().path_and_query(),
            "/messages/outbound?count=50&offset=0"
        );
    }

    #[test]
    fn search_sends_every_filter_in_the_docs_order() {
        let args = SearchArgs {
            page: PageArgs {
                count: Some(10),
                offset: 20,
            },
            recipient: Some("a@example.com".into()),
            from_email: Some("b@example.com".into()),
            tag: Some("welcome".into()),
            status: Some(OutboundStatus::Processed),
            dates: DateTimeRange {
                from_date: Some("2026-01-01".into()),
                to_date: Some("2026-01-31T23:59:59".into()),
            },
            subject: Some("Hi there".into()),
            message_stream: Some("broadcast".into()),
            metadata: Some(("color".into(), "dark blue".into())),
            ..search()
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/messages/outbound?count=10&offset=20&recipient=a%40example.com\
             &fromemail=b%40example.com&tag=welcome&status=processed\
             &todate=2026-01-31T23%3A59%3A59&fromdate=2026-01-01&subject=Hi+there\
             &messagestream=broadcast&metadata_color=dark+blue"
        );
        assert!(args.paging().is_some());
    }

    #[test]
    fn get_and_dump_put_the_message_id_in_one_segment() {
        let id = "a/b c";
        let get = GetArgs {
            message_id: id.into(),
        };
        assert_eq!(
            get.request().unwrap().path_and_query(),
            "/messages/outbound/a%2Fb%20c/details"
        );
        let dump = DumpArgs {
            message_id: id.into(),
        };
        assert_eq!(
            dump.request().unwrap().path_and_query(),
            "/messages/outbound/a%2Fb%20c/dump"
        );
        assert_eq!(get.paging(), None);
        assert_eq!(dump.spec().token, TokenKind::Server);
    }
}
