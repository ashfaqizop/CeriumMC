//! The single source of truth for the vanilla release CeriumMC targets.
//!
//! Every crate reads the target version from here. CI shell scripts read the same values
//! from `ci/vanilla.env`, and a unit test keeps the two in sync.
//!
//! CeriumMC stays pinned to one release until it reaches full parity with it.

#![cfg_attr(not(test), no_std)]

/// Version of CeriumMC itself.
pub const CERIUM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Minecraft: Java Edition release CeriumMC targets.
pub const MINECRAFT_VERSION: &str = "26.3";

/// Network protocol version of [`MINECRAFT_VERSION`].
pub const PROTOCOL_VERSION: i32 = 777;

/// World data version ("`DataVersion`") written into chunks and `level.dat`.
pub const WORLD_VERSION: i32 = 5023;

/// Data pack format of [`MINECRAFT_VERSION`].
pub const DATA_PACK_FORMAT: PackFormat = PackFormat {
    major: 121,
    minor: 0,
};

/// Resource pack format of [`MINECRAFT_VERSION`].
pub const RESOURCE_PACK_FORMAT: PackFormat = PackFormat {
    major: 97,
    minor: 1,
};

/// Java major version the vanilla reference server runs on.
pub const REFERENCE_JAVA_VERSION: u32 = 25;

/// The official vanilla server jar used as the parity reference in CI.
///
/// It is downloaded from Mojang and verified against this hash at test time.
/// It is never committed or redistributed.
pub const VANILLA_SERVER_JAR: Download = Download {
    url: "https://piston-data.mojang.com/v1/objects/\
          33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar",
    sha1: "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c",
    size: 62_294_556,
};

/// A pack format version: `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackFormat {
    /// Major format number; a change means a breaking change.
    pub major: u32,
    /// Minor format number; a change means a backwards-compatible addition.
    pub minor: u32,
}

/// A file published by Mojang, identified by its URL and verified by its hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Download {
    /// Download URL.
    pub url: &'static str,
    /// Lowercase hexadecimal SHA-1 of the file.
    pub sha1: &'static str,
    /// File size in bytes.
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV_FILE: &str = include_str!("../../../ci/vanilla.env");

    fn env(key: &str) -> &'static str {
        let found = ENV_FILE
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .find(|(k, _)| k.trim() == key);
        let Some((_, value)) = found else {
            panic!("{key} is missing from ci/vanilla.env");
        };
        value.trim()
    }

    #[test]
    fn constants_match_ci_env() {
        assert_eq!(env("VANILLA_VERSION"), MINECRAFT_VERSION);
        assert_eq!(env("VANILLA_PROTOCOL"), PROTOCOL_VERSION.to_string());
        assert_eq!(env("VANILLA_WORLD_VERSION"), WORLD_VERSION.to_string());
        assert_eq!(env("VANILLA_JAVA"), REFERENCE_JAVA_VERSION.to_string());
        assert_eq!(env("VANILLA_SERVER_URL"), VANILLA_SERVER_JAR.url);
        assert_eq!(env("VANILLA_SERVER_SHA1"), VANILLA_SERVER_JAR.sha1);
        assert_eq!(
            env("VANILLA_SERVER_SIZE"),
            VANILLA_SERVER_JAR.size.to_string()
        );
    }

    #[test]
    fn jar_url_is_content_addressed_by_its_sha1() {
        let sha1 = VANILLA_SERVER_JAR.sha1;
        assert_eq!(sha1.len(), 40);
        assert!(sha1.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        assert!(VANILLA_SERVER_JAR.url.contains(sha1));
        assert!(
            VANILLA_SERVER_JAR
                .url
                .starts_with("https://piston-data.mojang.com/")
        );
    }
}
