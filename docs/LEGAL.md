# Legal notes

## Not affiliated with Mojang or Microsoft

CeriumMC is an independent project by StepByte Studios. It is **not an official Minecraft
product**, and it is not approved by or associated with Mojang or Microsoft. "Minecraft" and
related names are trademarks of Mojang Synergies AB. They are used here only to describe
compatibility.

## How vanilla behaviour is studied

To reach exact behavioural parity, maintainers study the official Minecraft: Java Edition
dedicated server (currently **26.3**) **locally, for research only**:

- The official server jar is downloaded from Mojang's own distribution servers on the
  maintainer's machine and in CI, and its published hash is verified.
- Any decompiled or extracted material stays in a local, git-ignored research cache. It is
  **never committed, published or distributed**, including as CI artifacts.
- CeriumMC's source code is original Rust written from an understanding of that behaviour.
  It is not a translation of Mojang's code.

## What this repository contains

- Original CeriumMC source code and documentation, licensed GPL-2.0-only.
- CeriumMC and StepByte Studios brand assets.

It does **not** contain Minecraft code, textures, sounds, models, language files or other game
assets.

## Licensing

CeriumMC is licensed under the **GNU GPL, version 2 only**.

To reach bit-exact parity, CeriumMC reproduces the math routines that vanilla's results depend
on. On x86_64, HotSpot implements `Math.sin`, `cos`, `tan`, `log`, `log10`, `exp` and `pow`
with Intel's LIBM algorithms. Those are published only in OpenJDK
(`src/hotspot/cpu/x86/stubGenerator_x86_64_*.cpp`, Copyright Intel Corporation and others),
under the GPL version 2 only.

Ports of that code keep their original copyright notices and stay under GPL-2.0-only.
GPL-2.0-only code cannot be combined with GPL-3.0 code, so the whole project moved from
GPL-3.0 to GPL-2.0-only on 2026-11-12. At that point every commit had a single author.

Dependencies must be GPL-2.0-compatible: Apache-2.0-only, GPL-3.0 and LGPL-3.0 crates are not
allowed. `cargo-deny` enforces this in CI (`deny.toml`).

## Players

CeriumMC is a server. Players connect with their own legitimately purchased Minecraft: Java
Edition clients. Online-mode authentication goes through Mojang's official services.

## Questions

For legal or licensing concerns, contact **ashfaqizop@gmail.com**.
