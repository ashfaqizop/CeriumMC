//! Boot benchmark: start a server, wait until it answers a status ping, sample its
//! memory, then stop it gracefully and report what was measured.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::json::{ObjectWriter, version_field};
use crate::memory::{self, MemorySample};
use crate::slp::{self, StatusReply};

/// Pause between connection attempts while the server is starting.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How long one status attempt may wait. A server that accepts the connection before it
/// can answer replies as soon as it is ready, so this sets the measurement's precision
/// only for servers that refuse connections while starting.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// Pause between memory samples while the server settles.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

/// What to launch and how to measure it.
#[derive(Debug, Clone)]
pub(crate) struct BootOptions {
    /// Name recorded in the report.
    pub(crate) label: String,
    /// Host to ping.
    pub(crate) host: String,
    /// Port to ping.
    pub(crate) port: u16,
    /// Protocol version sent in the handshake.
    pub(crate) protocol: i32,
    /// Give up if there is no status reply within this time.
    pub(crate) timeout: Duration,
    /// How long to keep sampling memory after the first reply.
    pub(crate) settle: Duration,
    /// Console command that stops the server gracefully; `None` kills it instead.
    pub(crate) stop_command: Option<String>,
    /// Kill the server if it has not exited this long after the stop command.
    pub(crate) stop_timeout: Duration,
    /// Working directory for the server.
    pub(crate) cwd: Option<PathBuf>,
    /// The server command line: program first, then arguments.
    pub(crate) command: Vec<String>,
}

/// Everything measured during one boot.
#[derive(Debug, Clone)]
pub(crate) struct BootReport {
    /// Name from [`BootOptions::label`].
    pub(crate) label: String,
    /// From process start until the first complete status reply.
    pub(crate) time_to_status: Duration,
    /// Round-trip time of the first ping/pong.
    pub(crate) ping: Duration,
    /// The first status JSON the server sent.
    pub(crate) status_json: String,
    /// Memory after settling, if the platform can measure it.
    pub(crate) memory: Option<MemorySample>,
    /// How the server shut down.
    pub(crate) shutdown: Shutdown,
}

/// How the server process ended.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shutdown {
    /// Time from the stop command until the process exited; `None` if it was killed.
    pub(crate) duration: Option<Duration>,
    /// The process exit code, if it exited normally.
    pub(crate) exit_code: Option<i32>,
}

impl BootReport {
    /// The version name the server reported.
    pub(crate) fn version_name(&self) -> Option<&str> {
        version_field(&self.status_json, "name")
    }

    /// The protocol version the server reported.
    pub(crate) fn protocol(&self) -> Option<i32> {
        version_field(&self.status_json, "protocol")?.parse().ok()
    }

    /// The report as a single-line JSON object.
    pub(crate) fn to_json(&self) -> String {
        let memory = self.memory;
        let shutdown_ms = self.shutdown.duration.as_ref().map(Duration::as_millis);
        let mut object = ObjectWriter::default();
        object
            .string("label", &self.label)
            .number("time_to_status_ms", self.time_to_status.as_millis())
            .number("ping_us", self.ping.as_micros())
            .opt_string("version_name", self.version_name())
            .opt_number("protocol", self.protocol())
            .opt_number("rss_kib", memory.map(|m| m.rss_kib))
            .opt_number("peak_rss_kib", memory.map(|m| m.peak_rss_kib))
            .opt_number("shutdown_ms", shutdown_ms)
            .opt_number("exit_code", self.shutdown.exit_code);
        object.finish()
    }
}

/// Launches the server, measures it and stops it.
pub(crate) fn run(options: &BootOptions) -> Result<BootReport, String> {
    let Some((program, args)) = options.command.split_first() else {
        return Err("no server command was given".to_owned());
    };
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::piped());
    if let Some(dir) = &options.cwd {
        command.current_dir(dir);
    }

    let started = Instant::now();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Err(format!("cannot start {program}: {error}")),
    };
    let reply = match wait_for_status(&mut child, options, started) {
        Ok(reply) => reply,
        Err(error) => {
            kill(&mut child);
            return Err(error);
        }
    };
    let time_to_status = started.elapsed();
    let memory = settle(child.id(), options.settle);
    let shutdown = stop(&mut child, options)?;

    Ok(BootReport {
        label: options.label.clone(),
        time_to_status,
        ping: reply.ping,
        status_json: reply.json,
        memory,
        shutdown,
    })
}

/// Polls until the server answers a complete status exchange.
fn wait_for_status(
    child: &mut Child,
    options: &BootOptions,
    started: Instant,
) -> Result<StatusReply, String> {
    let (host, port, protocol) = (options.host.as_str(), options.port, options.protocol);
    loop {
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                let message = format!("server exited before answering a ping: {status}");
                return Err(message);
            }
            Err(error) => return Err(format!("cannot poll the server process: {error}")),
        }
        if let Ok(reply) = slp::query(host, port, protocol, ATTEMPT_TIMEOUT) {
            return Ok(reply);
        }
        if started.elapsed() >= options.timeout {
            let secs = options.timeout.as_secs();
            return Err(format!("no status reply within {secs} s"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Samples memory for `duration`, returning the latest RSS and the highest peak seen.
fn settle(pid: u32, duration: Duration) -> Option<MemorySample> {
    let deadline = Instant::now() + duration;
    let mut merged: Option<MemorySample> = None;
    loop {
        if let Some(sample) = memory::sample(pid) {
            merged = Some(match merged {
                Some(older) => older.then(sample),
                None => sample,
            });
        }
        if Instant::now() >= deadline {
            return merged;
        }
        thread::sleep(SAMPLE_INTERVAL);
    }
}

/// Stops the server with its console command, or kills it when there is none.
fn stop(child: &mut Child, options: &BootOptions) -> Result<Shutdown, String> {
    let Some(stop_command) = &options.stop_command else {
        kill(child);
        return Ok(Shutdown {
            duration: None,
            exit_code: None,
        });
    };

    let began = Instant::now();
    let sent = match child.stdin.as_mut() {
        Some(stdin) => writeln!(stdin, "{stop_command}"),
        None => Ok(()),
    };
    if let Err(error) = sent {
        kill(child);
        return Err(format!("cannot send the stop command: {error}"));
    }
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(Shutdown {
                    duration: Some(began.elapsed()),
                    exit_code: status.code(),
                });
            }
            Ok(None) => {}
            Err(error) => return Err(format!("cannot poll the server process: {error}")),
        }
        if began.elapsed() >= options.stop_timeout {
            kill(child);
            let secs = options.stop_timeout.as_secs();
            return Err(format!("server did not stop within {secs} s"));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Kills the server and reaps it. Errors are ignored: the process may already be gone.
fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(status_json: &str) -> BootReport {
        BootReport {
            label: "vanilla".to_owned(),
            time_to_status: Duration::from_millis(15_250),
            ping: Duration::from_micros(830),
            status_json: status_json.to_owned(),
            memory: Some(MemorySample {
                rss_kib: 1_000,
                peak_rss_kib: 1_200,
            }),
            shutdown: Shutdown {
                duration: Some(Duration::from_millis(2_500)),
                exit_code: Some(0),
            },
        }
    }

    #[test]
    fn report_json_has_every_field() {
        let report = report(r#"{"version":{"name":"26.3","protocol":777}}"#);
        assert_eq!(report.version_name(), Some("26.3"));
        assert_eq!(report.protocol(), Some(777));
        assert_eq!(
            report.to_json(),
            concat!(
                r#"{"label":"vanilla","time_to_status_ms":15250,"ping_us":830,"#,
                r#""version_name":"26.3","protocol":777,"rss_kib":1000,"#,
                r#""peak_rss_kib":1200,"shutdown_ms":2500,"exit_code":0}"#
            )
        );
    }

    #[test]
    fn unparseable_status_gives_null_fields() {
        let json = report("{}").to_json();
        assert!(json.contains(r#""version_name":null,"protocol":null"#));
    }

    #[test]
    fn empty_command_is_rejected() {
        let options = BootOptions {
            label: String::new(),
            host: "127.0.0.1".to_owned(),
            port: 1,
            protocol: 777,
            timeout: Duration::from_secs(1),
            settle: Duration::ZERO,
            stop_command: None,
            stop_timeout: Duration::from_secs(1),
            cwd: None,
            command: Vec::new(),
        };
        assert!(run(&options).is_err());
    }
}
