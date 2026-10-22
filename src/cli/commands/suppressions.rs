//! `suppressions`: the addresses a stream will not send to (server token).
//! Not paginated.

use crate::cli::args::{api_name, date_only};
use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum SuppressionsCommand {
    /// List a stream's suppressed addresses. Not paginated.
    Dump(DumpArgs),
}

impl SuppressionsCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            SuppressionsCommand::Dump(args) => args,
        }
    }
}

/// `SuppressionReason` for the dump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Reason {
    #[value(name = "HardBounce")]
    HardBounce,
    #[value(name = "SpamComplaint")]
    SpamComplaint,
    #[value(name = "ManualSuppression")]
    ManualSuppression,
}

/// `Origin` for the dump: who added the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Origin {
    #[value(name = "Recipient")]
    Recipient,
    #[value(name = "Customer")]
    Customer,
    #[value(name = "Admin")]
    Admin,
}

#[derive(Debug, Args)]
pub(crate) struct DumpArgs {
    /// The stream ID, e.g. `outbound`.
    stream_id: String,

    /// Only addresses suppressed for this reason (sends `SuppressionReason`).
    #[arg(long, value_enum, ignore_case = true, value_name = "REASON")]
    suppression_reason: Option<Reason>,

    /// Only addresses added this way (sends `Origin`).
    #[arg(long, value_enum, ignore_case = true)]
    origin: Option<Origin>,

    /// Only suppressions on or after this date (sends `fromdate`): YYYY-MM-DD,
    /// inclusive. The docs do not state the time zone.
    #[arg(long, value_name = "DATE", value_parser = date_only)]
    from_date: Option<String>,

    /// Only suppressions up to this date (sends `todate`): YYYY-MM-DD,
    /// inclusive. The docs do not state the time zone.
    #[arg(long, value_name = "DATE", value_parser = date_only)]
    to_date: Option<String>,

    /// Only this address (sends `EmailAddress`).
    #[arg(long, value_name = "EMAIL")]
    email_address: Option<String>,
}

pub(crate) static DUMP: EndpointSpec = EndpointSpec {
    path: "/message-streams/{stream_id}/suppressions/dump",
    token: TokenKind::Server,
    summary: "List a stream's suppressed addresses. Not paginated.",
    pagination: Pagination::None,
    list_key: Some("Suppressions"),
    example: "postmark-cli suppressions dump outbound --suppression-reason HardBounce",
};

impl Leaf for DumpArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &DUMP
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&DUMP, &[&self.stream_id])?
            .query_opt(
                "SuppressionReason",
                self.suppression_reason.as_ref().map(api_name),
            )
            .query_opt("Origin", self.origin.as_ref().map(api_name))
            .query_opt("todate", self.to_date.clone())
            .query_opt("fromdate", self.from_date.clone())
            .query_opt("EmailAddress", self.email_address.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_sends_its_filters_in_the_docs_order() {
        let bare = DumpArgs {
            stream_id: "outbound".into(),
            suppression_reason: None,
            origin: None,
            from_date: None,
            to_date: None,
            email_address: None,
        };
        assert_eq!(
            bare.request().unwrap().path_and_query(),
            "/message-streams/outbound/suppressions/dump"
        );
        let all = DumpArgs {
            suppression_reason: Some(Reason::ManualSuppression),
            origin: Some(Origin::Recipient),
            from_date: Some("2026-01-01".into()),
            to_date: Some("2026-02-01".into()),
            email_address: Some("a@example.com".into()),
            ..bare
        };
        assert_eq!(
            all.request().unwrap().path_and_query(),
            "/message-streams/outbound/suppressions/dump?SuppressionReason=ManualSuppression\
             &Origin=Recipient&todate=2026-02-01&fromdate=2026-01-01&EmailAddress=a%40example.com"
        );
        assert_eq!(all.paging(), None);
    }
}
