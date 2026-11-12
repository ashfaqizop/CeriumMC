# CeriumMC Roadmap

CeriumMC is built one phase at a time. A phase is **done** only when its "Done when" criteria pass
in **GitHub CI**. The next phase starts only after that.

- Milestones **Q1–Q5** are tracked as GitHub milestones.
- **Q1 (vanilla parity)** is split into Phases 1–15 below.
- Phase 0 sets up the repository.

Status legend: ⬜ Not started · 🟨 In progress · ✅ Done (date)

## How each phase works

1. **Study.** Read the relevant vanilla 26.3 packages, which are decompiled locally and never
   committed, and write a research note in `docs/research/`.
2. **Design.** Plan the idiomatic Rust structure: crates, data layout, ownership and threading.
3. **Implement.** Write original Rust, using the vanilla algorithm structure only where bit-exact
   parity requires it.
4. **Prove.** Write differential tests against the real vanilla server in CI: packets, world
   output and tick traces.
5. **Mark done.** Once CI is green, mark the phase ✅ in a separate commit.

---

## Phase 0: Public repository ✅ (2026-11-12)

GitHub-ready project setup.

- README with the AI disclaimer, LICENSE (GPL-2.0-only since 2026-11-12, originally GPL-3.0), Code of Conduct, Security policy,
  Contributing guide and legal notes.
- Issue and PR templates, plus brand assets (transparent logos with light and dark variants).
- Goal documents for Q1–Q5, this roadmap, and GitHub milestones Q1–Q5.

**Done when:** the repository is public and every document renders correctly on GitHub.

## Phase 1: Foundation & CI ✅ (2026-11-12)

- Cargo workspace layout and coding standards; `rust-toolchain`, rustfmt and clippy configuration.
- CI matrix: Linux x86_64, Windows, AArch64/ARMv7 cross-builds and Android (Termux) targets.
- Vanilla harness: CI downloads the official 26.3 jar, verifies its hash and runs it headless as
  the reference server (it is never committed).
- Benchmark and memory-tracking harness, so boot time and RSS are tracked from day one.

**Done when:** an empty server binary builds on every target, and the vanilla reference server
boots in CI.

## Phase 2: Core primitives 🟨

Delivered in three CI-verified steps:

- **2.1 Math & RNG** 🟨: all of vanilla's `Mth` (lookup-table trigonometry, rounding, angle
  wrapping, interpolation, positional seeds) and **every RNG**: the three LCG flavours,
  Xoroshiro128++, `WorldgenRandom` seeding, positional factories and `RandomSupport`. Also MD5
  and Java's `String.hashCode`. Golden data is recorded from the real 26.3 server in CI and
  replayed on x86_64, ARM64, ARMv7, Windows and macOS.
- **2.2 NBT** ⬜: binary (file and network forms), Modified UTF-8, size and depth limits, and
  SNBT parsing and printing.
- **2.3 Wire & core types** ⬜: VarInt/VarLong, strings, identifiers, UUIDs, block, section
  and chunk positions, directions and vectors.

Text components move to Phase 3: their click and hover events embed items and entities, which
need registries.

**Done when:** golden tests replay vanilla's own outputs with zero differences on every
platform.

## Phase 3: Networking & protocol ⬜

- An async connection pipeline with framing, compression and AES/CFB8 encryption.
- Text components (JSON and NBT forms) and the codec framework they are serialized with.
- The handshake, status, login (offline and online Mojang auth) and configuration states.
- Definitions for every protocol-777 play packet, generated from the vanilla packet reports.

**Done when:** a vanilla client gets the server-list ping and completes login into the
configuration state; packet encodings match vanilla.

## Phase 4: Registries & data-driven content ⬜

- Built-in registries: blocks and **all block states**, items, entity types, and so on.
- Data packs: JSON codecs, tags, dynamic registries (biomes, dimension types, damage types…),
  and registry sync to the client.
- Definitions for recipes, loot tables, predicates and item components.

**Done when:** the registry dumps match the vanilla data generator reports exactly, and the
vanilla datapack loads with zero errors.

## Phase 5: World storage (Anvil) ⬜

- Read and write `.mca` regions, chunk NBT, `level.dat`, player data and POI.
- Paletted containers and heightmaps in their exact storage format.
- A data-fixer strategy for older worlds.

**Done when:** a vanilla-created world loads and re-saves with semantically identical chunk data.

## Phase 6: Server core: join & walk ⬜

- Boot sequence; `server.properties`, ops, whitelist and bans; the console.
- Tick loop and scheduler; levels and dimensions; the chunk ticket and loading system.
- Player join, chunk sending, movement validation, entity tracking basics and chat.

**Done when:** a vanilla client joins an existing world, walks around, chats and sees other
players. The first playable build.

## Phase 7: Block updates, lighting & fluids ⬜

- Neighbour and shape updates, scheduled block and fluid ticks, random ticks.
- The block and sky light engine; water and lava flow; falling blocks.
- Block placing and breaking with drops.

**Done when:** tick traces of the update order and light values match vanilla on scripted
scenarios.

## Phase 8: World generation I: terrain ⬜

- Noise router and density functions; biome source and climate sampling.
- Surface rules, aquifers, ore veins, carvers; Overworld, Nether and End terrain.

**Done when:** for a fixed set of seeds, generated terrain matches vanilla block-for-block.

## Phase 9: World generation II: features & structures ⬜

- Placed and configured features: trees, ores, vegetation, lakes and so on.
- Structures, the jigsaw system, structure templates and placement.
- Spawn-point selection and initial spawn-area generation.

**Done when:** fully decorated worlds for the seed set match vanilla chunk-for-chunk.

## Phase 10: Blocks, redstone & items ⬜

- All block behaviours and block entities (chests, furnaces, hoppers, pistons…).
- **Redstone** with exact update order: comparators, observers, pistons and quasi-connectivity.
- Items and item components, inventories and menus, crafting, smelting, enchanting, anvils
  and brewing.

**Done when:** a redstone and technical-contraption suite behaves identically to vanilla, tick
by tick.

## Phase 11: Entities I: physics & players ⬜

- The entity base: movement, collision, physics and status effects.
- Player survival mechanics: health, hunger, XP, combat, damage and death/respawn.
- Item entities, XP orbs, projectiles, vehicles, armour stands and similar entities.

**Done when:** entity physics and combat traces match vanilla, and a survival session plays
correctly.

## Phase 12: Entities II: mobs & AI ⬜

- Goal-based and brain-based AI, pathfinding and navigation.
- Natural spawning and mob caps; every mob's behaviour.
- Villagers and trading, raids, and the Wither, Ender Dragon and Warden.

**Done when:** mob spawning, AI decisions and farm outputs match vanilla on deterministic
scenarios.

## Phase 13: Gameplay systems ⬜

- All vanilla **commands** with exact parsing and suggestions.
- Advancements, statistics, recipe book, scoreboards and teams, boss bars.
- Game rules, time and weather, portals and dimension travel, the world border.

**Done when:** command and advancement test suites match vanilla output and side effects.

## Phase 14: Performance & multithreading ⬜

- Parallel, order-independent work: chunk generation, lighting, I/O, compression and networking.
- A determinism verifier, so that parallel runs equal single-threaded runs bit-for-bit.
- Memory diet (≤1 GB for 20–30 players) and boot-time optimisation, for existing and fresh worlds.

**Done when:** the benchmarks meet the targets in CI, and the determinism verifier passes on
every architecture.

## Phase 15: Parity certification & Q1 release ⬜

- Full differential test campaign against vanilla: long-running world, protocol and gameplay
  soak tests.
- Cross-platform validation on x86_64 Linux, Windows, ARM and Android.
- User documentation, configuration reference and migration guide.

**Done when:** every parity suite is green, and **CeriumMC 1.0 (Q1)** is tagged and released.

---

## After Q1

| Milestone | Summary |
|---|---|
| Q2 | `.HCR` region format: fragment-and-finalize, zstd with a dictionary, lossless MCA bridge |
| Q3 | Plugins API |
| Q4 | CeriumMC plugin library |
| Q5 | Pooled multi-machine server with seamless failover |

Each milestone gets its own phase plan when Q1 is complete.
