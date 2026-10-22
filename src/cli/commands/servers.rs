//! `servers`: the servers of the account (account token).

use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum ServersCommand {
    /// List the servers of the account.
    List(ListArgs),
    /// Get one server of the account.
    Get(GetArgs),
}

impl ServersCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            ServersCommand::List(args) => args,
            ServersCommand::Get(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,
    /// Only servers whose name contains this text (sends `name`).
    #[arg(long, value_name = "SUBSTRING")]
    name: Option<String>,
}

static LIST: EndpointSpec = EndpointSpec {
    path: "/servers",
    token: TokenKind::Account,
    summary: "List the servers of the account.",
    // The docs state no maximum; error 600 says "up to 500 servers per call".
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("Servers"),
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(self
            .page
            .request(&LIST, &[])?
            .query_opt("name", self.name.clone()))
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The server ID.
    server_id: u64,
}

static GET: EndpointSpec = EndpointSpec {
    path: "/servers/{serverid}",
    token: TokenKind::Account,
    summary: "Get one server of the account.",
    pagination: Pagination::None,
    list_key: None,
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.server_id.to_string()])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(count: Option<u32>, name: Option<&str>) -> ListArgs {
        ListArgs {
            page: PageArgs { count, offset: 0 },
            all: AllArgs {
                all: false,
                limit: None,
            },
            name: name.map(String::from),
        }
    }

    #[test]
    fn list_sends_count_offset_and_name() {
        assert_eq!(
            list(None, None).request().unwrap().path_and_query(),
            "/servers?count=50&offset=0"
        );
        assert_eq!(
            list(Some(10), Some("prod eu"))
                .request()
                .unwrap()
                .path_and_query(),
            "/servers?count=10&offset=0&name=prod+eu"
        );
        assert!(list(Some(501), None).request().is_err());
        assert_eq!(list(None, None).paging().unwrap().count, None);
    }

    #[test]
    fn get_puts_the_id_in_the_path() {
        let args = GetArgs { server_id: 123 };
        assert_eq!(args.request().unwrap().path_and_query(), "/servers/123");
        assert_eq!(args.spec().token, TokenKind::Account);
        assert_eq!(args.paging(), None);
    }
}
