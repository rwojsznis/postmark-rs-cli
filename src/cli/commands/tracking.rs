//! The filters `opens list` and `clicks list` share. Neither endpoint takes a
//! date, so a search by date starts with `outbound search --from-date`.

use crate::request::GetRequest;
use clap::Args;

#[derive(Debug, Default, Args)]
pub(crate) struct TrackingFilters {
    /// Only events for this recipient, in To, Cc or Bcc (sends `recipient`).
    #[arg(long, value_name = "EMAIL")]
    recipient: Option<String>,

    /// Only events for messages with this tag (sends `tag`).
    #[arg(long)]
    tag: Option<String>,

    /// Only events in this stream (sends `messagestream`). Postmark's default
    /// is `outbound`, the transactional stream.
    #[arg(long, value_name = "ID")]
    message_stream: Option<String>,

    /// Only this email client, e.g. Outlook, Gmail (sends `client_name`).
    #[arg(long, value_name = "NAME")]
    client_name: Option<String>,

    /// Only clients from this company, e.g. Microsoft, Google (sends
    /// `client_company`).
    #[arg(long, value_name = "COMPANY")]
    client_company: Option<String>,

    /// Only this client family, e.g. Chrome (sends `client_family`).
    #[arg(long, value_name = "FAMILY")]
    client_family: Option<String>,

    /// Only this OS and version, e.g. "Windows 7" (sends `os_name`).
    #[arg(long, value_name = "NAME")]
    os_name: Option<String>,

    /// Only this OS, any version, e.g. Windows (sends `os_family`).
    #[arg(long, value_name = "FAMILY")]
    os_family: Option<String>,

    /// Only OSes from this company, e.g. "Microsoft Corporation" (sends
    /// `os_company`).
    #[arg(long, value_name = "COMPANY")]
    os_company: Option<String>,

    /// Only this platform, e.g. webmail, desktop, mobile (sends `platform`).
    #[arg(long)]
    platform: Option<String>,

    /// Only events in this country, e.g. Denmark (sends `country`).
    #[arg(long)]
    country: Option<String>,

    /// Only events in this region, by full name, e.g. "New York" (sends
    /// `region`).
    #[arg(long)]
    region: Option<String>,

    /// Only events in this city, by full name, e.g. Philadelphia (sends
    /// `city`).
    #[arg(long)]
    city: Option<String>,
}

impl TrackingFilters {
    /// Adds the filters that are set, in the docs' order for opens.
    pub(crate) fn query(&self, request: GetRequest) -> GetRequest {
        [
            ("recipient", &self.recipient),
            ("tag", &self.tag),
            ("messagestream", &self.message_stream),
            ("client_name", &self.client_name),
            ("client_company", &self.client_company),
            ("client_family", &self.client_family),
            ("os_name", &self.os_name),
            ("os_family", &self.os_family),
            ("os_company", &self.os_company),
            ("platform", &self.platform),
            ("country", &self.country),
            ("region", &self.region),
            ("city", &self.city),
        ]
        .into_iter()
        .fold(request, |request, (key, value)| {
            request.query_opt(key, value.clone())
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every filter set to `<key>-value`.
    pub(crate) fn all_set() -> TrackingFilters {
        let v = |key: &str| Some(format!("{key} value"));
        TrackingFilters {
            recipient: v("recipient"),
            tag: v("tag"),
            message_stream: v("messagestream"),
            client_name: v("client_name"),
            client_company: v("client_company"),
            client_family: v("client_family"),
            os_name: v("os_name"),
            os_family: v("os_family"),
            os_company: v("os_company"),
            platform: v("platform"),
            country: v("country"),
            region: v("region"),
            city: v("city"),
        }
    }

    pub(crate) const ALL_SET_QUERY: &str = "recipient=recipient+value&tag=tag+value\
        &messagestream=messagestream+value&client_name=client_name+value\
        &client_company=client_company+value&client_family=client_family+value\
        &os_name=os_name+value&os_family=os_family+value&os_company=os_company+value\
        &platform=platform+value&country=country+value&region=region+value&city=city+value";
}
