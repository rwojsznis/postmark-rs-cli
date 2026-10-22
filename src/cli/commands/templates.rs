//! `templates`: email templates and layouts (server token).

use crate::cli::args::api_name;
use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum TemplatesCommand {
    /// List templates and layouts.
    List(ListArgs),
    /// Get one template, with its content, by ID or alias.
    Get(GetArgs),
}

impl TemplatesCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            TemplatesCommand::List(args) => args,
            TemplatesCommand::Get(args) => args,
        }
    }
}

/// `TemplateType` for the template list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum TemplateType {
    #[value(name = "All")]
    All,
    #[value(name = "Standard")]
    Standard,
    #[value(name = "Layout")]
    Layout,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,

    /// Only standard templates or only layouts (sends `TemplateType`).
    /// Postmark's default is `All`.
    #[arg(long, value_enum, ignore_case = true)]
    template_type: Option<TemplateType>,

    /// Only templates that use the layout with this alias (sends
    /// `LayoutTemplate`).
    #[arg(long, value_name = "ALIAS")]
    layout_template: Option<String>,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/templates",
    token: TokenKind::Server,
    summary: "List templates and layouts.",
    // The docs state no maximum; 500 is the limit of every other list.
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: None,
    },
    list_key: Some("Templates"),
    example: "postmark-cli templates list --template-type Layout",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    // The docs' table says `Count` and `Offset`; their curl example, and
    // every other list, send them in lowercase (decision 8).
    fn request(&self) -> Result<GetRequest, Error> {
        Ok(self
            .page
            .request(&LIST, &[])?
            .query_opt("TemplateType", self.template_type.as_ref().map(api_name))
            .query_opt("LayoutTemplate", self.layout_template.clone()))
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The template's numeric ID or its alias.
    id_or_alias: String,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/templates/{templateIdOrAlias}",
    token: TokenKind::Server,
    summary: "Get one template, with its content, by ID or alias.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli templates get welcome-email",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.id_or_alias])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> ListArgs {
        ListArgs {
            page: PageArgs {
                count: None,
                offset: 0,
            },
            all: AllArgs {
                all: false,
                limit: None,
            },
            template_type: None,
            layout_template: None,
        }
    }

    #[test]
    fn list_sends_lowercase_paging_and_its_filters() {
        assert_eq!(
            list().request().unwrap().path_and_query(),
            "/templates?count=50&offset=0"
        );
        let args = ListArgs {
            template_type: Some(TemplateType::Layout),
            layout_template: Some("base layout".into()),
            ..list()
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/templates?count=50&offset=0&TemplateType=Layout&LayoutTemplate=base+layout"
        );
        assert!(args.paging().is_some());
    }

    #[test]
    fn get_takes_an_id_or_an_alias_as_one_segment() {
        let get = |id: &str| {
            GetArgs {
                id_or_alias: id.into(),
            }
            .request()
            .unwrap()
            .path_and_query()
        };
        assert_eq!(get("12345"), "/templates/12345");
        assert_eq!(
            get("folder/name with space"),
            "/templates/folder%2Fname%20with%20space"
        );
    }
}
