# Phase 1 research: distribution, boot order and the status protocol

Studied: vanilla **26.3** (protocol 777). This note describes behaviour in our own words; it
contains no Mojang code.

## 1. How the server is distributed

- `server.jar` is a **bundler**: a small launcher jar that contains the real server jar and its
  libraries.
  - `META-INF/versions.list` lists the inner `server-26.3.jar` with its SHA-256.
  - `META-INF/libraries.list` lists 39 dependency jars with their SHA-256 hashes and Maven
    coordinates.
- On first start the bundler extracts both into `versions/` and `libraries/` in the working
  directory, then launches the inner jar.
- Since the 26.x line, the inner jar is **not obfuscated**, so no mapping files are needed for
  research.
- `version.json` inside the jar records: world version `5023`, protocol `777`, data pack format
  `121.0`, resource pack format `97.1`, Java `25`.

**CeriumMC decision:** these values live in one place, the `cerium-version` crate.
`ci/vanilla.env` mirrors them for shell scripts, and a unit test keeps the two identical.

## 2. Boot order and what "time to status" measures

The dedicated server starts up in this order:

1. Load `server.properties`, the EULA, ops, whitelist and bans.
2. **Bind the TCP listener** and log `Starting Minecraft server on *:<port>`.
3. Prepare the level: load or generate the spawn area. On a fresh world this is the slow step.
4. Log `Done (<time>)!`.
5. Build the status response, then enter the tick loop.

Incoming connections are processed **from the tick loop**. A client that connects during step 3
is therefore accepted by the operating system, but gets no answer until the first tick runs.

**Measurement decision:** `cerium-probe boot` measures from process start until the **first
complete status reply, including the ping/pong**. Each attempt waits up to 30 s on an open
connection, so the reply is timed to the moment the server can actually serve clients. This
matches what a player sees in the server list, and it applies equally to vanilla and CeriumMC.

Q1 requires fast boot on both fresh and existing worlds, so CI measures vanilla twice: once on a
fresh world and once on the world it just created.

## 3. Wire format used by the probe

| Element | Encoding |
|---|---|
| `VarInt` | 7 data bits per byte, least-significant group first, high bit = continuation, at most 5 bytes |
| String | `VarInt` byte length, then UTF-8 bytes |
| Frame (uncompressed) | `VarInt` length, then `VarInt` packet id, then payload; the length prefix is at most 21 bits (3 bytes) |

Handshake: packet `0x00`, made of a `VarInt` protocol, a string host (≤ 255 chars), an `u16`
port (big-endian) and a `VarInt` intent: `1` status, `2` login, `3` transfer.

Status state:

| Direction | Id | Packet | Body |
|---|---|---|---|
| → server | `0x00` | status request | empty |
| ← client | `0x00` | status response | JSON string, ≤ 32 767 chars |
| → server | `0x01` | ping request | `i64` (big-endian) |
| ← client | `0x01` | pong response | the same `i64` |

Packet ids are assigned in registration order within each protocol state. That is why the status
request and ping are `0x00` and `0x01`.

The probe uses a fixed ping payload (`0x0123456789ABCDEF`), so its bytes are identical on every
run.

## 4. Open questions for later phases

- Compression and encryption (Phase 3) are never enabled in the status state; the probe doesn't
  need them.
- The exact order of fields in the status JSON matters for byte-exact parity. It will be pinned
  in Phase 3 with golden tests.
