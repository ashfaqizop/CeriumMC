//! Smoke tests for the `ceriummc` command-line interface.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ceriummc"))
        .args(args)
        .output()
        .expect("failed to run the ceriummc binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is not UTF-8")
}

#[test]
fn version_flag_reports_the_vanilla_target() {
    let expected = format!(
        "CeriumMC {} (Minecraft 26.3, protocol 777)",
        env!("CARGO_PKG_VERSION")
    );
    for flag in ["--version", "-V"] {
        let output = run(&[flag]);
        assert!(output.status.success(), "{flag} failed: {output:?}");
        assert_eq!(stdout(&output).trim_end(), expected);
    }
}

#[test]
fn help_flag_prints_usage() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    assert!(stdout(&output).starts_with("Usage: ceriummc"));
}

#[test]
fn unknown_argument_fails_with_usage_error() {
    let output = run(&["--definitely-not-a-flag"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown argument '--definitely-not-a-flag'"));
}
