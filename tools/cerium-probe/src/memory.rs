//! Process memory sampling.
//!
//! Linux and Android expose per-process memory in `/proc/<pid>/status`. Other platforms
//! report no data for now.

/// Resident memory of a process, in KiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemorySample {
    /// Resident set size when the sample was taken (`VmRSS`).
    pub(crate) rss_kib: u64,
    /// Peak resident set size since the process started (`VmHWM`).
    pub(crate) peak_rss_kib: u64,
}

impl MemorySample {
    /// Combines an older sample with a newer one: the latest RSS and the highest peak.
    pub(crate) fn then(self, newer: Self) -> Self {
        Self {
            rss_kib: newer.rss_kib,
            peak_rss_kib: self.peak_rss_kib.max(newer.peak_rss_kib),
        }
    }
}

/// Samples the memory of process `pid`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn sample(pid: u32) -> Option<MemorySample> {
    let path = format!("/proc/{pid}/status");
    let status = std::fs::read_to_string(path).ok()?;
    parse_proc_status(&status)
}

/// Samples the memory of process `pid`. Not supported on this platform yet.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(crate) fn sample(_pid: u32) -> Option<MemorySample> {
    None
}

/// Extracts `VmRSS` and `VmHWM` from the contents of `/proc/<pid>/status`.
#[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
fn parse_proc_status(status: &str) -> Option<MemorySample> {
    Some(MemorySample {
        rss_kib: kib_field(status, "VmRSS:")?,
        peak_rss_kib: kib_field(status, "VmHWM:")?,
    })
}

/// Reads a `Name:   1234 kB` line.
#[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
fn kib_field(status: &str, name: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        let value = line.strip_prefix(name)?.trim();
        value.strip_suffix("kB")?.trim().parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "Name:\tjava\nVmPeak:\t 4000000 kB\nVmHWM:\t  812345 kB\n\
                          VmRSS:\t  790001 kB\nThreads:\t42\n";

    #[test]
    fn parses_rss_and_peak() {
        let sample = parse_proc_status(STATUS).unwrap();
        assert_eq!(sample.rss_kib, 790_001);
        assert_eq!(sample.peak_rss_kib, 812_345);
    }

    #[test]
    fn missing_fields_yield_none() {
        assert_eq!(parse_proc_status("Name:\tjava\n"), None);
    }

    #[test]
    fn then_keeps_latest_rss_and_highest_peak() {
        let older = MemorySample {
            rss_kib: 500,
            peak_rss_kib: 900,
        };
        let newer = MemorySample {
            rss_kib: 600,
            peak_rss_kib: 700,
        };
        let merged = older.then(newer);
        assert_eq!((merged.rss_kib, merged.peak_rss_kib), (600, 900));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn samples_this_process() {
        let sample = sample(std::process::id()).unwrap();
        assert!(sample.rss_kib > 0);
        assert!(sample.peak_rss_kib >= sample.rss_kib);
    }
}
