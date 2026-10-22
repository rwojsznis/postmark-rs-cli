//! `senders`: the account's sender signatures (account token).

use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum SendersCommand {
    /// List the account's sender signatures.
    List(ListArgs),
    /// Get one sender signature, with its DKIM and Return-Path details.
    Get(GetArgs),
}

impl SendersCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            SendersCommand::List(args) => args,
            SendersCommand::Get(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/senders",
    token: TokenKind::Account,
    summary: "List the account's sender signatures.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("SenderSignatures"),
    example: "postmark-cli senders list --count 100",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    fn request(&self) -> Result<GetRequest, Error> {
        self.page.request(&LIST, &[])
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The sender signature ID (`ID` in the sender list). `SPFVerified` in
    /// the response is deprecated.
    signature_id: u64,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/senders/{signatureid}",
    token: TokenKind::Account,
    summary: "Get one sender signature, with its DKIM and Return-Path details.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli senders get 1",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.signature_id.to_string()])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_pages_and_get_puts_the_id_in_the_path() {
        let list = ListArgs {
            page: PageArgs {
                count: None,
                offset: 50,
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
        };
        assert_eq!(
            list.request().unwrap().path_and_query(),
            "/senders?count=50&offset=50"
        );
        assert!(list.paging().is_some());
        let get = GetArgs { signature_id: 1 };
        assert_eq!(get.request().unwrap().path_and_query(), "/senders/1");
        assert_eq!(get.spec().token, TokenKind::Account);
    }
}
