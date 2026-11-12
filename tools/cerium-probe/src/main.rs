//! `cerium-probe` talks to Minecraft servers from the outside, the way a client would.
//!
//! - `ping` performs one Server List Ping and prints the status JSON.
//! - `boot` launches a server, measures how long it takes to answer a status ping and how
//!   much memory it uses, stops it gracefully and prints a JSON report.
//!
//! CI uses the probe to measure the vanilla reference server and CeriumMC in exactly the
//! same way. It depends only on the standard library.

mod boot;
mod json;
mod memory;
mod slp;
mod wire;

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

use cerium_version::PROTOCOL_VERSION;

use crate::boot::BootOptions;

const USAGE: &str = "\
Usage:
  cerium-probe ping [--host HOST] [--port PORT] [--protocol N] [--timeout-secs S]
  cerium-probe boot [OPTIONS] -- COMMAND [ARGS...]

Boot options:
  --label NAME             Name recorded in the report (default: server)
  --host HOST              Host to ping (default: 127.0.0.1)
  --port PORT              Port to ping (default: 25565)
  --protocol N             Handshake protocol version (default: CeriumMC's target)
  --timeout-secs S         Fail if there is no status reply after S seconds (default: 300)
  --settle-secs S          Sample memory for S seconds after the first reply (default: 5)
  --stop-command CMD       Console command that stops the server (default: kill it)
  --stop-timeout-secs S    Kill the server if it has not stopped after S seconds (default: 120)
  --expect-version NAME    Fail unless the server reports version NAME
  --expect-protocol N      Fail unless the server reports protocol N
  --cwd DIR                Working directory for the server
  --report FILE            Also write the JSON report to FILE
";

const PING_FLAGS: &[&str] = &["--host", "--port", "--protocol", "--timeout-secs"];

const BOOT_FLAGS: &[&str] = &[
    "--label",
    "--host",
    "--port",
    "--protocol",
    "--timeout-secs",
    "--settle-secs",
    "--stop-command",
    "--stop-timeout-secs",
    "--expect-version",
    "--expect-protocol",
    "--cwd",
    "--report",
];

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 25_565;

/// Why the probe failed.
#[derive(Debug)]
enum Failure {
    /// The command line was wrong.
    Usage(String),
    /// The server could not be reached or did not behave as expected.
    Check(String),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ping") => run_ping(&args[1..]),
        Some("boot") => run_boot(&args[1..]),
        Some("-h" | "--help") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => Err(Failure::Usage("expected a subcommand".to_owned())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Check(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run_ping(args: &[String]) -> Result<(), Failure> {
    let flags = Flags::parse(args, PING_FLAGS)?;
    if !flags.rest.is_empty() {
        return Err(Failure::Usage("ping does not take a command".to_owned()));
    }
    let host = flags.get("--host").unwrap_or(DEFAULT_HOST);
    let port = flags.value("--port", DEFAULT_PORT)?;
    let protocol = flags.value("--protocol", PROTOCOL_VERSION)?;
    let timeout = Duration::from_secs(flags.value("--timeout-secs", 10)?);

    match slp::query(host, port, protocol, timeout) {
        Ok(reply) => {
            println!("{}", reply.json);
            eprintln!("ping: {} us", reply.ping.as_micros());
            Ok(())
        }
        Err(error) => Err(Failure::Check(format!("{host}:{port}: {error}"))),
    }
}

fn run_boot(args: &[String]) -> Result<(), Failure> {
    let flags = Flags::parse(args, BOOT_FLAGS)?;
    if flags.rest.is_empty() {
        let message = "boot needs a server command after '--'".to_owned();
        return Err(Failure::Usage(message));
    }
    let options = BootOptions {
        label: flags.get("--label").unwrap_or("server").to_owned(),
        host: flags.get("--host").unwrap_or(DEFAULT_HOST).to_owned(),
        port: flags.value("--port", DEFAULT_PORT)?,
        protocol: flags.value("--protocol", PROTOCOL_VERSION)?,
        timeout: Duration::from_secs(flags.value("--timeout-secs", 300)?),
        settle: Duration::from_secs(flags.value("--settle-secs", 5)?),
        stop_command: flags.get("--stop-command").map(str::to_owned),
        stop_timeout: Duration::from_secs(flags.value("--stop-timeout-secs", 120)?),
        cwd: flags.get("--cwd").map(PathBuf::from),
        command: flags.rest.clone(),
    };

    let report = boot::run(&options).map_err(Failure::Check)?;
    let json = report.to_json();
    println!("{json}");
    if let Some(path) = flags.get("--report") {
        write_report(path, &json)?;
    }

    if let Some(expected) = flags.get("--expect-version") {
        let actual = report.version_name();
        if actual != Some(expected) {
            let message = format!("expected version {expected}, got {actual:?}");
            return Err(Failure::Check(message));
        }
    }
    let expected_protocol: Option<i32> = flags.optional_value("--expect-protocol")?;
    if let Some(expected) = expected_protocol {
        let actual = report.protocol();
        if actual != Some(expected) {
            let message = format!("expected protocol {expected}, got {actual:?}");
            return Err(Failure::Check(message));
        }
    }
    Ok(())
}

/// Writes the JSON report to `path`, followed by a newline.
fn write_report(path: &str, json: &str) -> Result<(), Failure> {
    match std::fs::write(path, format!("{json}\n")) {
        Ok(()) => Ok(()),
        Err(error) => Err(Failure::Check(format!("cannot write {path}: {error}"))),
    }
}

/// Parsed `--flag value` pairs, plus everything after a `--` separator.
#[derive(Debug)]
struct Flags {
    pairs: Vec<(String, String)>,
    rest: Vec<String>,
}

impl Flags {
    /// Parses `args`, accepting only the flags listed in `allowed`.
    fn parse(args: &[String], allowed: &[&str]) -> Result<Self, Failure> {
        let mut pairs = Vec::new();
        let mut iter = args.iter();
        while let Some(flag) = iter.next() {
            if flag == "--" {
                let rest = iter.cloned().collect();
                return Ok(Self { pairs, rest });
            }
            if !allowed.contains(&flag.as_str()) {
                return Err(Failure::Usage(format!("unknown option '{flag}'")));
            }
            let Some(value) = iter.next() else {
                return Err(Failure::Usage(format!("{flag} needs a value")));
            };
            pairs.push((flag.clone(), value.clone()));
        }
        Ok(Self {
            pairs,
            rest: Vec::new(),
        })
    }

    /// The last value given for `name`.
    fn get(&self, name: &str) -> Option<&str> {
        let pair = self.pairs.iter().rev().find(|(flag, _)| flag == name);
        pair.map(|(_, value)| value.as_str())
    }

    /// The parsed value of `name`, or `None` when it was not given.
    fn optional_value<T: FromStr>(&self, name: &str) -> Result<Option<T>, Failure> {
        let Some(raw) = self.get(name) else {
            return Ok(None);
        };
        let Ok(value) = raw.parse() else {
            let message = format!("invalid value '{raw}' for {name}");
            return Err(Failure::Usage(message));
        };
        Ok(Some(value))
    }

    /// The parsed value of `name`, or `default` when it was not given.
    fn value<T: FromStr>(&self, name: &str, default: T) -> Result<T, Failure> {
        Ok(self.optional_value(name)?.unwrap_or(default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().copied().map(String::from).collect()
    }

    #[test]
    fn flags_and_command_are_split_at_double_dash() {
        let args = strings(&["--port", "25566", "--", "java", "-jar", "server.jar"]);
        let flags = Flags::parse(&args, BOOT_FLAGS).unwrap();
        assert_eq!(flags.value("--port", DEFAULT_PORT).unwrap(), 25_566);
        assert_eq!(flags.rest, strings(&["java", "-jar", "server.jar"]));
    }

    #[test]
    fn defaults_apply_when_flags_are_absent() {
        let flags = Flags::parse(&[], PING_FLAGS).unwrap();
        assert_eq!(flags.value("--port", DEFAULT_PORT).unwrap(), DEFAULT_PORT);
        let protocol = flags.value("--protocol", PROTOCOL_VERSION).unwrap();
        assert_eq!(protocol, PROTOCOL_VERSION);
        assert_eq!(flags.rest, Vec::<String>::new());
    }

    #[test]
    fn unknown_flags_and_missing_values_are_usage_errors() {
        let unknown = Flags::parse(&strings(&["--nope", "1"]), PING_FLAGS);
        assert!(matches!(unknown, Err(Failure::Usage(_))));
        let missing = Flags::parse(&strings(&["--port"]), PING_FLAGS);
        assert!(matches!(missing, Err(Failure::Usage(_))));
    }

    #[test]
    fn invalid_values_are_usage_errors() {
        let args = strings(&["--port", "seventy"]);
        let flags = Flags::parse(&args, PING_FLAGS).unwrap();
        let port = flags.value("--port", DEFAULT_PORT);
        assert!(matches!(port, Err(Failure::Usage(_))));
    }

    #[test]
    fn last_value_wins() {
        let args = strings(&["--host", "a", "--host", "b"]);
        let flags = Flags::parse(&args, PING_FLAGS).unwrap();
        assert_eq!(flags.get("--host"), Some("b"));
    }
}
