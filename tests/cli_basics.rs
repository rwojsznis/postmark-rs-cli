//! The flags clap answers before any command runs.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_postmark-cli"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn version_prints_the_binary_name_and_the_cargo_version() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "postmark-cli 0.0.0\n"
    );
}

#[test]
fn help_states_the_read_only_guarantee() {
    let out = run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8(out.stdout).unwrap().contains("read-only"));
}

#[test]
fn no_arguments_is_a_usage_error() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
}
