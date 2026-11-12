# Q1: Vanilla parity

**Goal:** a full, idiomatic Rust rewrite of the Minecraft: Java Edition 26.3 dedicated server.
It must behave **identically** to vanilla while being dramatically faster and leaner.

## What "identical" means

All four of these must match the official server:

| Area | Requirement |
|---|---|
| Network protocol | An unmodified 26.3 client (protocol 777) connects and plays with no visible difference. |
| World generation | The same seed produces bit-identical terrain, biomes, features and structures. |
| Tick mechanics | Redstone, mob AI, RNG, farms and tick order behave identically; technical contraptions work. |
| Data & config | Data packs, all vanilla commands, `server.properties`, ops, whitelist and bans work as-is. |

## Performance targets

| Target | Value |
|---|---|
| Boot | Near-instant: both until the server accepts connections and until a player can join, on existing **and** fresh worlds. |
| Memory | ≤ 1 GB RAM for 20–30 players. |
| Threads | Multithreaded wherever the work is provably order-independent (chunk generation, lighting, I/O, networking). |
| Platforms | x86_64 Linux, Windows, ARM boards (Raspberry Pi, ARM cloud), Android phones. |

## Rules

- **Parity wins.** If speed and exact vanilla behaviour conflict, choose parity. Tick results must
  be deterministic and must not depend on thread count or CPU architecture.
- **Hybrid code style.** The architecture is original idiomatic Rust (data-oriented where it
  helps). Vanilla's algorithm structure is followed closely only where bit-exactness demands it,
  such as world generation and RNG.
- **Anvil support.** Q1 reads and writes `.mca` worlds, so output can be compared directly with
  vanilla. `.HCR` comes in Q2.
- **Pinned version.** Stay on 26.3 until parity, then move to the latest release.

## Plan

Q1 is delivered in Phases 1–15. See [ROADMAP.md](../../ROADMAP.md).
