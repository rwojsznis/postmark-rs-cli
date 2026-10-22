//! `data-removals`: the status of a data removal request (account token).

use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum DataRemovalsCommand {
    /// Get the status of one data removal request (Pending or Done). The
    /// feature is available only on request, and may need the account
    /// owner's token.
    Get(GetArgs),
}

impl DataRemovalsCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            DataRemovalsCommand::Get(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The data removal request ID.
    id: u64,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/data-removals/{id}",
    token: TokenKind::Account,
    summary: "Get the status of one data removal request. Available on request only.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli data-removals get 1234",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.id.to_string()])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_puts_the_id_in_the_path() {
        let get = GetArgs { id: 1234 };
        assert_eq!(
            get.request().unwrap().path_and_query(),
            "/data-removals/1234"
        );
        assert_eq!(get.spec().token, TokenKind::Account);
        assert_eq!(get.paging(), None);
    }
}
