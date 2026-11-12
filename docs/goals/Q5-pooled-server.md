# Q5: Pooled multi-machine server

**Goal:** *"Our PC, our server."* One Minecraft world runs on hardware pooled from friends' home
PCs, so the server keeps running even when the PC that started it goes offline.

This is the hardest milestone. If it works, it is a real breakthrough for self-hosted servers.

## The scenario

1. Player A starts a CeriumMC server on their PC.
2. Player B doesn't want the world to die when A's PC turns off, so B **pools** their hardware.
3. B's machine is now **active**, **standby**, **warm** or **cold**, depending on what the world needs.
4. More friends join the pool, and the same world scales to **50–60 players**.

This is **not** a Velocity-style proxy network of separate servers. It is **one world** sharing
resources on the fly. [`.HCR`](Q2-hcr-format.md) makes this practical, because compact,
fragment-based data moves quickly between PCs.

## Architecture

| Topic | Decision |
|---|---|
| Authority | **Staged:** (1) one active node with hot and warm standbys; (2) the active node offloads parallel-safe work (chunk generation, lighting, pathfinding, saving) to pool nodes; (3) spatial sharding |
| Entry point | **Any node is a door.** Every node accepts vanilla clients and forwards to the active node; a dynamic-DNS name follows the best entry |
| Failover | **Seamless:** a hot standby takes over within about 1–2 s, and players see only a lag spike |
| Replication | The deterministic tick journal is streamed, plus periodic `.HCR` fragment snapshots; standbys replay it |
| Connectivity | Direct P2P mesh: IPv6, UPnP and UDP hole punching first, then routing through a reachable peer. **Never a central relay.** Not every node has to join the full mesh, which protects home bandwidth |
| Node selection | Scored on network quality, hardware power and reliability history |
| Live allocation | Owners set live caps for RAM, CPU and bandwidth, and the pool rebalances automatically within them |
| Split-brain | A majority quorum; two-node pools break ties with a lease in cloud storage |
| Scale | 50–60 players on 3–6 home PCs |

## Membership & trust

- Pools are **private and invite-only** (friends). All traffic is signed and encrypted anyway.
- Members join with an **invite code or QR**, optionally linked to their Minecraft account for
  display and permissions.
- **Android phones** can be full nodes, including active, if they score best.

## Cloud storage fallback

Used for **cold world backups**, **pool rendezvous** (peers find each other through a shared
folder, so no central server is needed) and as the **cold-start source** when every node was off.

Supported providers: **Google Drive**, **S3-compatible** storage (AWS S3, Cloudflare R2,
Backblaze B2, MinIO) and **WebDAV / local NAS**.

## Management

A **desktop GUI app** and a **CLI**.

## First Q5 milestone

**Seamless failover.** PC A runs the world, and PC B is a hot standby fed by the journal stream.
Switch A off, and B takes over without kicking anyone.

## Dependency on Q1

Q5 relies on Q1's deterministic tick engine. Q1 and Q2 internals (the tick journal, determinism
and `.HCR` fragments) are designed from day one so that this replication is possible.
