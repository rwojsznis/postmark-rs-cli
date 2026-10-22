//! What goes on stdout (decisions 3 and 4): Postmark's JSON, unchanged, in
//! one of two layouts. The bytes do not depend on whether stdout is a
//! terminal.

use crate::out;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Pretty JSON, 2-space indent.
    Json,
    /// One compact JSON value per line.
    Jsonl,
}

/// The text for one value, with its trailing newline.
#[must_use]
pub fn render(value: &Value, format: Format) -> String {
    match format {
        Format::Json => format!("{value:#}\n"),
        Format::Jsonl => format!("{value}\n"),
    }
}

/// Prints one value. Returns `false` when stdout is closed.
#[expect(
    clippy::must_use_candidate,
    reason = "a command that prints one value has nothing to do when stdout is closed; only the `--all` loop reads the result"
)]
pub fn print(value: &Value, format: Format) -> bool {
    out!("{}", render(value, format))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"b":1,"a":{"z":[1,2],"y":"ü"}}"#;

    #[test]
    fn json_is_pretty_with_two_spaces_and_keeps_the_key_order() {
        let value: Value = serde_json::from_str(BODY).unwrap();
        assert_eq!(
            render(&value, Format::Json),
            "{\n  \"b\": 1,\n  \"a\": {\n    \"z\": [\n      1,\n      2\n    ],\n    \"y\": \"ü\"\n  }\n}\n"
        );
    }

    #[test]
    fn jsonl_is_the_compact_body_on_one_line() {
        let value: Value = serde_json::from_str(BODY).unwrap();
        assert_eq!(render(&value, Format::Jsonl), format!("{BODY}\n"));
    }
}
