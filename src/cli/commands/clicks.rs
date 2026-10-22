//! `clicks`: click tracking for sent messages (server token).

use crate::cli::commands::Leaf;
use crate::cli::commands::tracking::TrackingFilters;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum ClicksCommand {
    /// List link clicks in sent messages, with client, OS and location. No date filters. To find messages by date, use `outbound search
    /// --from-date` first.
    // Boxed: the filters make this variant many times larger than the other.
    List(Box<ListArgs>),
    /// List the link clicks in one sent message.
    ForMessage(ForMessageArgs),
}

impl ClicksCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            ClicksCommand::List(args) => args.as_ref(),
            ClicksCommand::ForMessage(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,
    #[command(flatten)]
    filters: TrackingFilters,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/messages/outbound/clicks",
    token: TokenKind::Server,
    summary: "List link clicks in sent messages, with client, OS and location. No date filters.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("Clicks"),
    example: "postmark-cli clicks list --recipient john@example.com --platform mobile",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(self.filters.query(self.page.request(&LIST, &[])?))
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct ForMessageArgs {
    /// The message ID (`MessageID` in search results).
    message_id: String,
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,
}

pub(crate) static FOR_MESSAGE: EndpointSpec = EndpointSpec {
    path: "/messages/outbound/clicks/{messageid}",
    token: TokenKind::Server,
    summary: "List the link clicks in one sent message.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("Clicks"),
    example: "postmark-cli clicks for-message 07311c54-0687-4ab9-b034-b54b5bad88ba",
};

impl Leaf for ForMessageArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &FOR_MESSAGE
    }

    fn request(&self) -> Result<GetRequest, Error> {
        self.page.request(&FOR_MESSAGE, &[&self.message_id])
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands::tracking::tests::{ALL_SET_QUERY, all_set};

    fn page() -> PageArgs {
        PageArgs {
            count: None,
            offset: 0,
        }
    }

    fn all() -> AllArgs {
        AllArgs {
            all: false,
            limit: None,
        }
    }

    #[test]
    fn list_with_no_filters_sends_the_page_only() {
        let args = ListArgs {
            page: page(),
            all: all(),
            filters: TrackingFilters::default(),
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/messages/outbound/clicks?count=50&offset=0"
        );
        assert!(args.paging().is_some());
    }

    #[test]
    fn list_sends_every_filter() {
        let args = ListArgs {
            page: page(),
            all: all(),
            filters: all_set(),
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            format!("/messages/outbound/clicks?count=50&offset=0&{ALL_SET_QUERY}")
        );
    }

    #[test]
    fn for_message_puts_the_id_in_the_path_and_pages() {
        let args = ForMessageArgs {
            message_id: "a/b".into(),
            page: PageArgs {
                count: Some(5),
                offset: 10,
            },
            all: all(),
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/messages/outbound/clicks/a%2Fb?count=5&offset=10"
        );
        assert!(args.paging().is_some());
    }
}
