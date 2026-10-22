//! Shared value parsers (decision 8). Each one checks the user's string and
//! returns it unchanged, so the API gets exactly what was typed: a date is
//! never reformatted or moved to another time zone.

use crate::request::GetRequest;
use clap::{Args, ValueEnum};
use time::macros::format_description;
use time::{Date, PrimitiveDateTime};

/// `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`, for the endpoints that take a time.
pub(crate) fn date_or_datetime(value: &str) -> Result<String, String> {
    let date_time = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
    if date_only(value).is_ok() || PrimitiveDateTime::parse(value, date_time).is_ok() {
        Ok(value.to_owned())
    } else {
        Err("expected a real date as YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS".to_owned())
    }
}

/// `YYYY-MM-DD`, for the endpoints that take a date only.
pub(crate) fn date_only(value: &str) -> Result<String, String> {
    match Date::parse(value, format_description!("[year]-[month]-[day]")) {
        Ok(_) => Ok(value.to_owned()),
        Err(_) => Err("expected a real date as YYYY-MM-DD".to_owned()),
    }
}

/// `KEY=VALUE`, split at the first `=`. The key must not be empty.
pub(crate) fn key_value(value: &str) -> Result<(String, String), String> {
    match value.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
        _ => Err("expected KEY=VALUE with a non-empty KEY".to_owned()),
    }
}

/// `--from-date` and `--to-date` for the endpoints that take a date or a date
/// and time, in Eastern Time (messages, bounces).
#[derive(Debug, Default, Args)]
pub(crate) struct DateTimeRange {
    /// Only on or after this date (sends `fromdate`): YYYY-MM-DD or
    /// YYYY-MM-DDTHH:MM:SS, inclusive, Eastern Time.
    #[arg(long, value_name = "DATE", value_parser = date_or_datetime)]
    pub(crate) from_date: Option<String>,

    /// Only up to this date (sends `todate`): YYYY-MM-DD or
    /// YYYY-MM-DDTHH:MM:SS, inclusive, Eastern Time.
    #[arg(long, value_name = "DATE", value_parser = date_or_datetime)]
    pub(crate) to_date: Option<String>,
}

impl DateTimeRange {
    /// Adds `todate` and `fromdate`, in the docs' order.
    pub(crate) fn query(&self, request: GetRequest) -> GetRequest {
        request
            .query_opt("todate", self.to_date.clone())
            .query_opt("fromdate", self.from_date.clone())
    }
}

/// The exact API spelling of an enum flag's value (decision 8).
pub(crate) fn api_name(value: &impl ValueEnum) -> String {
    // `None` only for a variant marked `#[value(skip)]`, which no flag has.
    value
        .to_possible_value()
        .map(|v| v.get_name().to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_or_a_date_time_is_kept_as_typed() {
        for ok in [
            "2026-01-01",
            "2026-02-28",
            "2024-02-29",
            "2026-01-01T12:00:00",
        ] {
            assert_eq!(date_or_datetime(ok).as_deref(), Ok(ok));
        }
        for bad in [
            "2026-02-30",
            "2026-13-01",
            "2026-1-1",
            "2026-01-01T25:00:00",
            "2026-01-01 12:00:00",
            "2026-01-01T12:00",
            "2026-01-01Z",
            "",
        ] {
            let error = date_or_datetime(bad).unwrap_err();
            assert!(error.contains("YYYY-MM-DD"), "{bad}: {error}");
        }
    }

    #[test]
    fn a_date_only_flag_takes_no_time() {
        assert_eq!(date_only("2026-01-01").as_deref(), Ok("2026-01-01"));
        for bad in ["2026-01-01T12:00:00", "2026-02-30", "01/02/2026"] {
            let error = date_only(bad).unwrap_err();
            assert!(error.contains("YYYY-MM-DD"), "{bad}: {error}");
        }
    }

    #[test]
    fn key_value_splits_at_the_first_equals_sign() {
        assert_eq!(key_value("a=b"), Ok(("a".into(), "b".into())));
        assert_eq!(key_value("a=b=c"), Ok(("a".into(), "b=c".into())));
        assert_eq!(
            key_value("color=dark blue"),
            Ok(("color".into(), "dark blue".into()))
        );
        assert_eq!(key_value("a="), Ok(("a".into(), String::new())));
        assert!(key_value("=b").is_err());
        assert!(key_value("a").is_err());
    }

    #[test]
    fn an_enum_value_keeps_its_api_spelling() {
        #[derive(Clone, ValueEnum)]
        enum Kind {
            #[value(name = "HardBounce")]
            HardBounce,
            #[value(name = "queued")]
            Queued,
        }
        assert_eq!(api_name(&Kind::HardBounce), "HardBounce");
        assert_eq!(api_name(&Kind::Queued), "queued");
    }
}
