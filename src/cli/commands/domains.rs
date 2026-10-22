//! `domains`: the account's sending domains (account token).

use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum DomainsCommand {
    /// List the account's domains, with their verification state.
    List(ListArgs),
    /// Get one domain: full DKIM, Return-Path and SPF details.
    /// `SPFVerified` is deprecated.
    Get(GetArgs),
}

impl DomainsCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            DomainsCommand::List(args) => args,
            DomainsCommand::Get(args) => args,
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
    path: "/domains",
    token: TokenKind::Account,
    summary: "List the account's domains, with their verification state.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("Domains"),
    example: "postmark-cli domains list --all",
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
    /// The domain ID (`ID` in the domain list).
    domain_id: u64,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/domains/{domainid}",
    token: TokenKind::Account,
    summary: "Get one domain: full DKIM, Return-Path and SPF details. SPFVerified is deprecated.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli domains get 36735",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.domain_id.to_string()])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_pages_and_get_puts_the_id_in_the_path() {
        let list = ListArgs {
            page: PageArgs {
                count: Some(10),
                offset: 0,
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
        };
        assert_eq!(
            list.request().unwrap().path_and_query(),
            "/domains?count=10&offset=0"
        );
        assert!(list.paging().is_some());
        let get = GetArgs { domain_id: 36735 };
        assert_eq!(get.request().unwrap().path_and_query(), "/domains/36735");
        assert_eq!(get.spec().token, TokenKind::Account);
    }
}
