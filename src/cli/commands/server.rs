//! `server`: the server that owns the server token.

use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum ServerCommand {
    /// Get the server that owns the server token.
    Get(GetArgs),
}

impl ServerCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            ServerCommand::Get(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/server",
    token: TokenKind::Server,
    summary: "Get the server that owns the server token.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli server get",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_calls_the_server_endpoint_with_no_query() {
        let request = GetArgs {}.request().unwrap();
        assert_eq!(request.path_and_query(), "/server");
        assert_eq!(GetArgs {}.spec().token, TokenKind::Server);
    }
}
