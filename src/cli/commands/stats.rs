//! `stats`: outbound statistics (server token). Twelve endpoints that take the
//! same four filters; none is paginated and none has a list key, so jsonl
//! prints each body on one line.

use crate::cli::args::date_only;
use crate::cli::commands::Leaf;
use crate::error::Error;
use crate::request::GetRequest;
use crate::spec::{EndpointSpec, Pagination, TokenKind};
use clap::{Args, Subcommand};

/// The filters every stats endpoint takes.
#[derive(Debug, Default, Args)]
pub(crate) struct StatsFilters {
    /// Only messages with this tag (sends `tag`).
    #[arg(long)]
    tag: Option<String>,

    /// From this date (sends `fromdate`): YYYY-MM-DD, inclusive, EST. With no
    /// dates, the stats cover all time. Error 1500 means it is more than a
    /// year ago.
    #[arg(long, value_name = "DATE", value_parser = date_only)]
    from_date: Option<String>,

    /// Up to this date (sends `todate`): YYYY-MM-DD, inclusive, EST.
    #[arg(long, value_name = "DATE", value_parser = date_only)]
    to_date: Option<String>,

    /// Only this stream (sends `messagestream`). Left out, the stats cover
    /// all streams.
    #[arg(long, value_name = "ID")]
    message_stream: Option<String>,
}

impl StatsFilters {
    // `messagestream` in lowercase, as the docs spell it; postmark-rs sends
    // `MessageStream` (decision 8: the docs win).
    fn request(&self, spec: &EndpointSpec) -> Result<GetRequest, Error> {
        Ok(GetRequest::new(spec, &[])?
            .query_opt("tag", self.tag.clone())
            .query_opt("fromdate", self.from_date.clone())
            .query_opt("todate", self.to_date.clone())
            .query_opt("messagestream", self.message_stream.clone()))
    }
}

/// One leaf per endpoint: its args type, its `static` spec, and its `Leaf`
/// impl, so the registry keeps one spec per command (decision 11).
macro_rules! stats_leaves {
    ($($variant:ident, $args:ident, $spec:ident, $path:literal, $summary:literal, $example:literal;)*) => {
        #[derive(Debug, Subcommand)]
        pub(crate) enum StatsCommand {
            $(
                #[doc = $summary]
                $variant($args),
            )*
        }

        impl StatsCommand {
            pub(crate) fn leaf(&self) -> &dyn Leaf {
                match self {
                    $(StatsCommand::$variant(args) => args,)*
                }
            }
        }

        $(
            #[derive(Debug, Default, Args)]
            pub(crate) struct $args {
                #[command(flatten)]
                filters: StatsFilters,
            }

            pub(crate) static $spec: EndpointSpec = EndpointSpec {
                path: $path,
                token: TokenKind::Server,
                summary: $summary,
                pagination: Pagination::None,
                list_key: None,
                example: $example,
            };

            impl Leaf for $args {
                fn spec(&self) -> &'static EndpointSpec {
                    &$spec
                }

                fn request(&self) -> Result<GetRequest, Error> {
                    self.filters.request(&$spec)
                }
            }
        )*
    };
}

stats_leaves! {
    Overview, OverviewArgs, OVERVIEW, "/stats/outbound",
        "Sent, bounced, spam, open and click totals.",
        "postmark-cli stats overview --from-date 2026-10-01";
    Sends, SendsArgs, SENDS, "/stats/outbound/sends",
        "Sent counts by day.",
        "postmark-cli stats sends --tag welcome";
    Bounces, BouncesArgs, BOUNCES, "/stats/outbound/bounces",
        "Bounce counts by day and type.",
        "postmark-cli stats bounces --message-stream outbound";
    Spam, SpamArgs, SPAM, "/stats/outbound/spam",
        "Spam complaint counts by day.",
        "postmark-cli stats spam";
    Tracked, TrackedArgs, TRACKED, "/stats/outbound/tracked",
        "Counts of tracked sent messages by day.",
        "postmark-cli stats tracked";
    Opens, OpensArgs, OPENS, "/stats/outbound/opens",
        "Open counts by day, total and unique.",
        "postmark-cli stats opens --from-date 2026-10-01 --to-date 2026-10-07";
    OpensPlatforms, OpensPlatformsArgs, OPENS_PLATFORMS, "/stats/outbound/opens/platforms",
        "Opens by platform (desktop, mobile, webmail) by day.",
        "postmark-cli stats opens-platforms";
    OpensEmailClients, OpensEmailClientsArgs, OPENS_EMAIL_CLIENTS, "/stats/outbound/opens/emailclients",
        "Opens by email client by day. One key per client name, so the keys vary.",
        "postmark-cli stats opens-email-clients";
    Clicks, ClicksArgs, CLICKS, "/stats/outbound/clicks",
        "Click counts by day, total and unique.",
        "postmark-cli stats clicks";
    ClicksBrowserFamilies, ClicksBrowserFamiliesArgs, CLICKS_BROWSER_FAMILIES, "/stats/outbound/clicks/browserfamilies",
        "Clicks by browser family by day. One key per browser, so the keys vary.",
        "postmark-cli stats clicks-browser-families";
    ClicksPlatforms, ClicksPlatformsArgs, CLICKS_PLATFORMS, "/stats/outbound/clicks/platforms",
        "Clicks by platform (desktop, mobile) by day.",
        "postmark-cli stats clicks-platforms";
    ClicksLocation, ClicksLocationArgs, CLICKS_LOCATION, "/stats/outbound/clicks/location",
        "Clicks by link location in the message (HTML or text) by day.",
        "postmark-cli stats clicks-location";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_set() -> StatsFilters {
        StatsFilters {
            tag: Some("welcome mail".into()),
            from_date: Some("2026-01-01".into()),
            to_date: Some("2026-01-31".into()),
            message_stream: Some("outbound".into()),
        }
    }

    #[test]
    fn every_stats_leaf_sends_its_path_and_the_shared_filters() {
        let leaves: Vec<(Box<dyn Leaf>, &str)> = vec![
            (Box::new(OverviewArgs::default()), "/stats/outbound"),
            (Box::new(SendsArgs::default()), "/stats/outbound/sends"),
            (Box::new(BouncesArgs::default()), "/stats/outbound/bounces"),
            (Box::new(SpamArgs::default()), "/stats/outbound/spam"),
            (Box::new(TrackedArgs::default()), "/stats/outbound/tracked"),
            (Box::new(OpensArgs::default()), "/stats/outbound/opens"),
            (
                Box::new(OpensPlatformsArgs::default()),
                "/stats/outbound/opens/platforms",
            ),
            (
                Box::new(OpensEmailClientsArgs::default()),
                "/stats/outbound/opens/emailclients",
            ),
            (Box::new(ClicksArgs::default()), "/stats/outbound/clicks"),
            (
                Box::new(ClicksBrowserFamiliesArgs::default()),
                "/stats/outbound/clicks/browserfamilies",
            ),
            (
                Box::new(ClicksPlatformsArgs::default()),
                "/stats/outbound/clicks/platforms",
            ),
            (
                Box::new(ClicksLocationArgs::default()),
                "/stats/outbound/clicks/location",
            ),
        ];
        for (leaf, path) in leaves {
            assert_eq!(leaf.request().unwrap().path_and_query(), path);
            assert_eq!(leaf.spec().path, path);
            assert_eq!(leaf.paging(), None);
            assert_eq!(
                all_set().request(leaf.spec()).unwrap().path_and_query(),
                format!(
                    "{path}?tag=welcome+mail&fromdate=2026-01-01&todate=2026-01-31\
                     &messagestream=outbound"
                )
            );
        }
    }
}
