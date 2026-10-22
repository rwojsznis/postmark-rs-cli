//! `bulk`: Bulk API send requests (server token). Both endpoints need Bulk
//! API approval; without it Postmark answers 422 with error code 14.

use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, CursorArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum BulkCommand {
    /// List bulk send requests, newest first. Needs Bulk API approval (error
    /// 14 without it).
    List(ListArgs),
    /// Get the status of one bulk send request. Needs Bulk API approval
    /// (error 14); an ID from another server gives 404 (error 12).
    Get(GetArgs),
}

impl BulkCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            BulkCommand::List(args) => args,
            BulkCommand::Get(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    cursor: CursorArgs,
    #[command(flatten)]
    all: AllArgs,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/email/bulk",
    token: TokenKind::Server,
    summary: "List bulk send requests, newest first. Needs Bulk API approval.",
    // The only cursor endpoint. The docs give no maximum for `count`; 500 is
    // the limit of every other list.
    pagination: Pagination::Cursor { max_count: 500 },
    list_key: Some("Requests"),
    example: "postmark-cli bulk list --count 20",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    fn request(&self) -> Result<GetRequest, Error> {
        self.cursor.request(&LIST, &[])
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.cursor.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The bulk request ID (a UUID, `ID` in the bulk list).
    bulk_request_id: String,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/email/bulk/{bulk-request-id}",
    token: TokenKind::Server,
    summary: "Get the status of one bulk send request. Needs Bulk API approval.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli bulk get f24af63c-533d-4b7a-ad65-4a7b3202d3a7",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.bulk_request_id])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sends_count_and_the_key_byte_for_byte() {
        let key = "eyJT+dW/Jt aXQ=";
        let args = ListArgs {
            cursor: CursorArgs {
                count: Some(20),
                pagination_key: Some(key.into()),
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
        };
        let sent = args.request().unwrap().path_and_query();
        assert_eq!(
            sent,
            "/email/bulk?count=20&paginationKey=eyJT%2BdW%2FJt+aXQ%3D"
        );
        let query = sent.split_once('?').unwrap().1;
        let received: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(received[1], ("paginationKey".to_owned(), key.to_owned()));
        assert!(args.paging().is_some());
    }

    #[test]
    fn get_puts_the_id_in_the_path() {
        let args = GetArgs {
            bulk_request_id: "f24af63c".into(),
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/email/bulk/f24af63c"
        );
    }
}
