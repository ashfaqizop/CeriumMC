# Q2: The `.HCR` chunk region format

**Goal:** a modern chunk region format, written in Rust, to replace Anvil's `.mca`. It aims to be
**50–70% smaller** and **4–5× faster**, and it is the foundation for fast data transfer between
machines in [Q5](Q5-pooled-server.md).

## The idea: borrowed from OBS Studio's Hybrid MP4

A normal MP4 writes its index only when recording ends, so a crash ruins the whole file. OBS
Studio's *Hybrid MP4* records as a crash-safe **fragmented** file. When recording stops, it does
a quick "soft-remux" so the result looks like a normal, compact MP4.

`.HCR` applies the same idea to chunk storage:

1. **While running:** chunk saves are appended to the region as **fragments**. Writes are
   sequential and fast, and a crash loses at most the unfinished tail fragment.
2. **Finalization:** when a region goes idle (no players nearby) and on clean shutdown, it is
   compacted into a dense, indexed layout.

## Design decisions

| Topic | Decision |
|---|---|
| Region size | 32×32 chunks per file, the same as MCA (1:1 mapping) |
| Encoding | Custom compact binary: palettes shared across chunks, bit-packed sections, deduplicated empty or uniform sections |
| Compression | zstd with a dictionary trained on chunk data |
| MCA bridge | Lossless both ways: MCA → HCR → MCA round-trips to identical chunk data, enforced in CI |
| Durability | Group commit: fragments are batched and fsynced every *N* ms (configurable) |
| Storage hardware | Tuned for everything, including SD cards and HDDs: sequential appends and low write amplification |

## Built-in features

- **Per-chunk checksums.** Corruption is detected, and bad chunks are repaired or skipped instead
  of crashing the server.
- **Incremental backups.** Fragments make cheap snapshots possible; only the changes are backed up.
- **Chunk history and rollback.** Older chunk versions can be kept, so griefing can be rolled back
  at the storage level.
- **Open specification and a standalone crate,** so map viewers, editors and other tools can read
  `.HCR`.

## Constraint

`.HCR` must never break [Q1](Q1-vanilla-parity.md) parity. The chunk content it stores must stay
semantically identical to vanilla's.
