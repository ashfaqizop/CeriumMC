//! CeriumMC server entry point.
//!
//! Phase 1 ships the shell of the binary: it reports its version and target and exits.
//! The server itself is built phase by phase; see `ROADMAP.md`.

use std::process::ExitCode;

use cerium_version::{CERIUM_VERSION, MINECRAFT_VERSION, PROTOCOL_VERSION};

const HELP: &str = "\
Usage: ceriummc [OPTIONS]

Options:
  -V, --version  Print version information and exit
  -h, --help     Print this help and exit
";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None => {
            println!("{}", version_line());
            println!("The server is not implemented yet; see ROADMAP.md for progress.");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("{}", version_line());
            ExitCode::SUCCESS
        }
        Some("-h" | "--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown argument '{other}'\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

/// One-line description of this build and the vanilla release it targets.
fn version_line() -> String {
    format!(
        "CeriumMC {CERIUM_VERSION} (Minecraft {MINECRAFT_VERSION}, protocol {PROTOCOL_VERSION})"
    )
}
