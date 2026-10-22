//! `streams`: message streams (server token). Not paginated.

use crate::cli::args::api_name;
use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum StreamsCommand {
    /// List the server's message streams. Not paginated.
    List(ListArgs),
    /// Get one message stream.
    Get(GetArgs),
}

impl StreamsCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            StreamsCommand::List(args) => args,
            StreamsCommand::Get(args) => args,
        }
    }
}

/// `MessageStreamType` for the stream list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum StreamType {
    #[value(name = "All")]
    All,
    #[value(name = "Inbound")]
    Inbound,
    #[value(name = "Transactional")]
    Transactional,
    #[value(name = "Broadcasts")]
    Broadcasts,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    /// Only streams of this type (sends `MessageStreamType`). Postmark's
    /// default is `All`.
    #[arg(long, value_enum, ignore_case = true, value_name = "TYPE")]
    message_stream_type: Option<StreamType>,

    /// Include archived streams (sends `IncludeArchivedStreams=true`).
    #[arg(long)]
    include_archived_streams: bool,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/message-streams",
    token: TokenKind::Server,
    summary: "List the server's message streams. Not paginated.",
    pagination: Pagination::None,
    list_key: Some("MessageStreams"),
    example: "postmark-cli streams list --message-stream-type Broadcasts --include-archived-streams",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&LIST, &[])?
            .query_opt(
                "MessageStreamType",
                self.message_stream_type.as_ref().map(api_name),
            )
            .query_flag("IncludeArchivedStreams", self.include_archived_streams))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The stream ID, e.g. `outbound` or `broadcast`.
    stream_id: String,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/message-streams/{stream_ID}",
    token: TokenKind::Server,
    summary: "Get one message stream.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli streams get outbound",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.stream_id])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sends_the_type_and_the_archived_flag_only_when_set() {
        let none = ListArgs {
            message_stream_type: None,
            include_archived_streams: false,
        };
        assert_eq!(none.request().unwrap().path_and_query(), "/message-streams");
        let both = ListArgs {
            message_stream_type: Some(StreamType::Transactional),
            include_archived_streams: true,
        };
        assert_eq!(
            both.request().unwrap().path_and_query(),
            "/message-streams?MessageStreamType=Transactional&IncludeArchivedStreams=true"
        );
        assert_eq!(both.paging(), None);
    }

    #[test]
    fn get_puts_the_stream_id_in_the_path() {
        let get = GetArgs {
            stream_id: "outbound".into(),
        };
        assert_eq!(
            get.request().unwrap().path_and_query(),
            "/message-streams/outbound"
        );
    }
}
