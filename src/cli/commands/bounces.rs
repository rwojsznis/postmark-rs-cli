//! `bounces`: delivery failures and their raw dumps (server token).

use crate::cli::args::{DateTimeRange, api_name};
use crate::cli::commands::Leaf;
use crate::cli::page::{AllArgs, PageArgs};
use crate::error::Error;
use crate::paginate::Paging;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Subcommand)]
pub(crate) enum BouncesCommand {
    /// Count bounces by type, and the number of inactive addresses.
    DeliveryStats(DeliveryStatsArgs),
    /// List bounces. Use the date filters to get past the 10,000 window.
    // Boxed: the filters make this variant many times larger than the others.
    List(Box<ListArgs>),
    /// Get one bounce, with its SMTP response content.
    Get(GetArgs),
    /// Get the raw source of one bounce. Dumps are kept for 30 days; an
    /// empty `Body` means there is none.
    Dump(DumpArgs),
}

impl BouncesCommand {
    pub(crate) fn leaf(&self) -> &dyn Leaf {
        match self {
            BouncesCommand::DeliveryStats(args) => args,
            BouncesCommand::List(args) => args.as_ref(),
            BouncesCommand::Get(args) => args,
            BouncesCommand::Dump(args) => args,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct DeliveryStatsArgs {}

pub(crate) static DELIVERY_STATS: EndpointSpec = EndpointSpec {
    path: "/deliverystats",
    token: TokenKind::Server,
    summary: "Count bounces by type, and the number of inactive addresses.",
    pagination: Pagination::None,
    // Two arrays (`Bounces` and `InactiveMails`), so jsonl prints the body
    // whole.
    list_key: None,
    example: "postmark-cli bounces delivery-stats",
};

impl Leaf for DeliveryStatsArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &DELIVERY_STATS
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&DELIVERY_STATS, &[])?)
    }
}

/// `type` for the bounce list. Each name is the API's exact spelling: clap's
/// default kebab-case would turn `SMTPApiError` into `smtp-api-error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum BounceType {
    #[value(name = "HardBounce")]
    HardBounce,
    #[value(name = "Transient")]
    Transient,
    #[value(name = "Unsubscribe")]
    Unsubscribe,
    #[value(name = "Subscribe")]
    Subscribe,
    #[value(name = "AutoResponder")]
    AutoResponder,
    #[value(name = "AddressChange")]
    AddressChange,
    #[value(name = "DnsError")]
    DnsError,
    #[value(name = "SpamNotification")]
    SpamNotification,
    #[value(name = "OpenRelayTest")]
    OpenRelayTest,
    #[value(name = "Unknown")]
    Unknown,
    #[value(name = "SoftBounce")]
    SoftBounce,
    #[value(name = "VirusNotification")]
    VirusNotification,
    #[value(name = "ChallengeVerification")]
    ChallengeVerification,
    #[value(name = "BadEmailAddress")]
    BadEmailAddress,
    #[value(name = "SpamComplaint")]
    SpamComplaint,
    #[value(name = "ManuallyDeactivated")]
    ManuallyDeactivated,
    #[value(name = "Unconfirmed")]
    Unconfirmed,
    #[value(name = "Blocked")]
    Blocked,
    #[value(name = "SMTPApiError")]
    SmtpApiError,
    #[value(name = "InboundError")]
    InboundError,
    #[value(name = "DMARCPolicy")]
    DmarcPolicy,
    #[value(name = "TemplateRenderingFailed")]
    TemplateRenderingFailed,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[command(flatten)]
    page: PageArgs,
    #[command(flatten)]
    all: AllArgs,

    /// Only bounces of this type (sends `type`).
    #[arg(long = "type", value_enum, ignore_case = true, value_name = "TYPE")]
    bounce_type: Option<BounceType>,

    /// Only bounces that did, or did not, deactivate the address (sends
    /// `inactive`). Leave it out for both.
    #[arg(long, value_name = "true|false")]
    inactive: Option<bool>,

    /// Only addresses that contain this text (sends `emailFilter`).
    #[arg(long, value_name = "TEXT")]
    email_filter: Option<String>,

    /// Only bounces of messages with this tag (sends `tag`).
    #[arg(long)]
    tag: Option<String>,

    /// Only the bounces of this message (sends `messageID`).
    #[arg(long, value_name = "ID")]
    message_id: Option<String>,

    #[command(flatten)]
    dates: DateTimeRange,

    /// Only bounces in this stream (sends `messagestream`). Postmark's default
    /// is `outbound`, the transactional stream.
    #[arg(long, value_name = "ID")]
    message_stream: Option<String>,
}

pub(crate) static LIST: EndpointSpec = EndpointSpec {
    path: "/bounces",
    token: TokenKind::Server,
    summary: "List bounces. Use the date filters to get past the 10,000 window.",
    pagination: Pagination::OffsetCount {
        max_count: 500,
        max_window: Some(10_000),
    },
    list_key: Some("Bounces"),
    example: "postmark-cli bounces list --type HardBounce --inactive true --from-date 2026-10-01",
};

impl Leaf for ListArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &LIST
    }

    // The docs' order: `fromdate` before `todate` here, unlike messages.
    fn request(&self) -> Result<GetRequest, Error> {
        Ok(self
            .page
            .request(&LIST, &[])?
            .query_opt("type", self.bounce_type.as_ref().map(api_name))
            .query_opt("inactive", self.inactive.map(|v| v.to_string()))
            .query_opt("emailFilter", self.email_filter.clone())
            .query_opt("tag", self.tag.clone())
            .query_opt("messageID", self.message_id.clone())
            .query_opt("fromdate", self.dates.from_date.clone())
            .query_opt("todate", self.dates.to_date.clone())
            .query_opt("messagestream", self.message_stream.clone()))
    }

    fn paging(&self) -> Option<Paging> {
        Some(self.page.paging(&self.all))
    }
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    /// The bounce ID (`ID` in the bounce list).
    bounce_id: u64,
}

pub(crate) static GET: EndpointSpec = EndpointSpec {
    path: "/bounces/{bounceid}",
    token: TokenKind::Server,
    summary: "Get one bounce, with its SMTP response content.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli bounces get 692560173",
};

impl Leaf for GetArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &GET
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&GET, &[&self.bounce_id.to_string()])?)
    }
}

#[derive(Debug, Args)]
pub(crate) struct DumpArgs {
    /// The bounce ID (`ID` in the bounce list).
    bounce_id: u64,
}

pub(crate) static DUMP: EndpointSpec = EndpointSpec {
    path: "/bounces/{bounceid}/dump",
    token: TokenKind::Server,
    summary: "Get the raw source of one bounce. Kept for 30 days; an empty Body means none.",
    pagination: Pagination::None,
    list_key: None,
    example: "postmark-cli bounces dump 692560173",
};

impl Leaf for DumpArgs {
    fn spec(&self) -> &'static EndpointSpec {
        &DUMP
    }

    fn request(&self) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(&DUMP, &[&self.bounce_id.to_string()])?)
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
            bounce_type: None,
            inactive: None,
            email_filter: None,
            tag: None,
            message_id: None,
            dates: DateTimeRange::default(),
            message_stream: None,
        }
    }

    #[test]
    fn every_type_is_sent_with_the_api_spelling() {
        // Copied from the docs (API-REFERENCE.md, "The bounce `type` enum").
        let documented = [
            "HardBounce",
            "Transient",
            "Unsubscribe",
            "Subscribe",
            "AutoResponder",
            "AddressChange",
            "DnsError",
            "SpamNotification",
            "OpenRelayTest",
            "Unknown",
            "SoftBounce",
            "VirusNotification",
            "ChallengeVerification",
            "BadEmailAddress",
            "SpamComplaint",
            "ManuallyDeactivated",
            "Unconfirmed",
            "Blocked",
            "SMTPApiError",
            "InboundError",
            "DMARCPolicy",
            "TemplateRenderingFailed",
        ];
        let ours: Vec<String> = BounceType::value_variants().iter().map(api_name).collect();
        assert_eq!(ours, documented);
    }

    #[test]
    fn list_with_no_filters_sends_the_page_only() {
        assert_eq!(
            list().request().unwrap().path_and_query(),
            "/bounces?count=50&offset=0"
        );
        assert!(list().paging().is_some());
    }

    #[test]
    fn list_sends_every_filter_in_the_docs_order() {
        let args = ListArgs {
            bounce_type: Some(BounceType::SmtpApiError),
            inactive: Some(false),
            email_filter: Some("example.com".into()),
            tag: Some("t".into()),
            message_id: Some("m1".into()),
            dates: DateTimeRange {
                from_date: Some("2026-01-01".into()),
                to_date: Some("2026-01-02T00:00:00".into()),
            },
            message_stream: Some("outbound".into()),
            ..list()
        };
        assert_eq!(
            args.request().unwrap().path_and_query(),
            "/bounces?count=50&offset=0&type=SMTPApiError&inactive=false\
             &emailFilter=example.com&tag=t&messageID=m1&fromdate=2026-01-01\
             &todate=2026-01-02T00%3A00%3A00&messagestream=outbound"
        );
    }

    #[test]
    fn the_single_bounce_commands_put_the_id_in_the_path() {
        assert_eq!(
            DeliveryStatsArgs {}.request().unwrap().path_and_query(),
            "/deliverystats"
        );
        assert_eq!(
            GetArgs { bounce_id: 42 }
                .request()
                .unwrap()
                .path_and_query(),
            "/bounces/42"
        );
        assert_eq!(
            DumpArgs { bounce_id: 42 }
                .request()
                .unwrap()
                .path_and_query(),
            "/bounces/42/dump"
        );
        assert_eq!(DumpArgs { bounce_id: 42 }.spec().list_key, None);
        assert_eq!(GetArgs { bounce_id: 1 }.spec().token, TokenKind::Server);
        assert_eq!(DeliveryStatsArgs {}.spec().list_key, None);
    }
}
