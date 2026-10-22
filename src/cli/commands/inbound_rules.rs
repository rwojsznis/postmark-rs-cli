//! `inbound-rules`: the server's inbound blocking rules (server token).

use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum InboundRulesCommand {
    /// List the rules that block inbound mail by address or domain.
    List(ListArgs),
}

impl InboundRulesCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            InboundRulesCommand::List(args) => args,
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
    path: "/triggers/inboundrules",
    token: TokenKind::Server,
    summary: "List the rules that block inbound mail by address or domain.",
    // No maximum in the parameter table; error 800 says "up to 500 per call".
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("InboundRules"),
    example: "postmark-cli inbound-rules list --all",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sends_the_page() {
        let args = ListArgs {
            page: PageArgs {
                count: Some(100),
                offset: 200,
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/triggers/inboundrules?count=100&offset=200"
        );
        assert!(args.paging().is_some());
    }
}
