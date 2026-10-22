//! What goes on stdout (decisions 3 and 4): Postmark's JSON, unchanged, in
//! one of two layouts. The bytes do not depend on whether stdout is a
//! terminal.

use crate::{errln, out};
use serde_json::{Value, json};

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

/// Prints one response. For `jsonl` and a list endpoint, that is one line
/// per item of `list_key`; a body without the key is printed whole, with a
/// `list_key_missing` warning.
pub fn print_response(value: &Value, list_key: Option<&str>, format: Format) {
    let (Format::Jsonl, Some(key)) = (format, list_key) else {
        print(value, format);
        return;
    };
    if let Some(items) = value.get(key).and_then(Value::as_array) {
        // `all` stops at the first write that finds stdout closed.
        let _ = items.iter().all(|item| print(item, Format::Jsonl));
    } else {
        warn_list_key_missing(key);
        print(value, Format::Jsonl);
    }
}

/// One warning line on stderr. Warnings do not change the exit code
/// (decision 5).
pub fn warn(kind: &str, message: &str) {
    errln!("{}", json!({"warning": {"kind": kind, "message": message}}));
}

pub fn warn_list_key_missing(key: &str) {
    warn(
        "list_key_missing",
        &format!("the response has no {key:?} array, so it is printed whole"),
    );
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
