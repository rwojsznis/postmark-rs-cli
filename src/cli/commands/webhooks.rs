//! `webhooks`: the server's webhooks and their delivery statistics (server
//! token).

use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum WebhooksCommand {
    /// List the server's webhooks. Not paginated.
    List(ListArgs),
    /// Get one webhook: its URL, triggers and auth settings.
    Get(GetArgs),
    /// Get one webhook's delivery statistics for the last 24 hours.
    Stats(StatsArgs),
}

impl WebhooksCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            WebhooksCommand::List(args) => args,
            WebhooksCommand::Get(args) => args,
            WebhooksCommand::Stats(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    /// Only webhooks of this stream (sends `MessageStream`). Leave it out for
    /// every webhook of the server. A stream that does not exist is an error,
    /// not an empty list.
    #[arg(long, value_name = "ID")]
    message_stream: Option<String>,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/webhooks",
    token: TokenKind::Server,
    summary: "List the server's webhooks. Not paginated.",
    pagination: Pagination::None,
    list_key: Some("Webhooks"),
    example: "postmark-cli webhooks list --message-stream outbound",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    // `MessageStream` in PascalCase, unlike `messagestream` on the message
    // and bounce endpoints: the docs' spelling (decision 8).
    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&LIST, &[])?.query_opt("MessageStream", self.message_stream.clone()))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The webhook ID (`ID` in the webhook list).
    webhook_id: u64,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/webhooks/{Id}",
    token: TokenKind::Server,
    summary: "Get one webhook: its URL, triggers and auth settings.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli webhooks get 1234567",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.webhook_id.to_string()])?)
    }
}

#[derive(Debug, Args)]
pub(crate) struct StatsArgs {
    /// The webhook ID (`ID` in the webhook list). The statistics cover the
    /// last 24 hours and cannot be filtered.
    webhook_id: u64,
}

pub(crate) static STATS: EndpointSpec = EndpointSpec {
    path: "/webhooks/{Id}/statistics",
    token: TokenKind::Server,
    summary: "Get one webhook's delivery statistics for the last 24 hours. No filters.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli webhooks stats 1234567",
};

impl Leaf for StatsArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &STATS
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&STATS, &[&self.webhook_id.to_string()])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sends_the_stream_in_pascal_case() {
        let none = ListArgs {
            message_stream: None,
        };
        assert_eq!(none.request().unwrap().path_and_query(), "/webhooks");
        let one = ListArgs {
            message_stream: Some("outbound".into()),
        };
        assert_eq!(
            one.request().unwrap().path_and_query(),
            "/webhooks?MessageStream=outbound"
        );
        assert_eq!(one.paging(), None);
    }

    #[test]
    fn get_and_stats_put_the_id_in_the_path() {
        assert_eq!(
            GetArgs { webhook_id: 7 }
                .request()
                .unwrap()
                .path_and_query(),
            "/webhooks/7"
        );
        assert_eq!(
            StatsArgs { webhook_id: 7 }
                .request()
                .unwrap()
                .path_and_query(),
            "/webhooks/7/statistics"
        );
    }
}
