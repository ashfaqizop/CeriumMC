# Contributing to CeriumMC

Thanks for your interest! CeriumMC is an AI-coded research project (see the
[AI disclaimer](README.md#ai-disclaimer)), and human contributions, reviews and bug reports are
very welcome.

By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ground rules

1. **No Mojang code or assets, ever.**
   - Do not paste or commit decompiled, deobfuscated or copied Minecraft source code.
   - Do not commit textures, sounds, the server jar or data generated from it.
   - Describe vanilla *behaviour* in your own words and implement it in original Rust.
   - See [docs/LEGAL.md](docs/LEGAL.md).
2. **Parity first.** A change that alters vanilla-observable behaviour needs a differential test
   proving it matches the official 26.3 server. If speed conflicts with parity, parity wins.
3. **One phase at a time.** Work follows [ROADMAP.md](ROADMAP.md). Please open an issue before
   starting work outside the current phase.
4. **CI is the source of truth.** Builds, tests and benchmarks run in GitHub Actions. A PR can be
   merged only when CI is green.

## Workflow

1. Open or claim an issue, and link it to the right milestone (Q1–Q5).
2. Fork the repo and create a branch: `phase-N/short-description`.
3. Keep commits focused, using [Conventional Commits](https://www.conventionalcommits.org/)
   style, e.g. `feat(protocol): add login encryption`.
4. Open a PR using the template, and explain *how* you verified parity.

## Coding standards (from Phase 1)

- Idiomatic, safe Rust. `unsafe` needs a `// SAFETY:` comment and a reviewer's sign-off.
- Formatting with `rustfmt` and linting with `clippy -D warnings`, both enforced in CI.
- Keep lines under 100 characters.
- Public items get doc comments. Research notes go in `docs/research/`.
- Code must not depend on CPU architecture or thread scheduling for its results (determinism).

## Using AI tools

AI assistance is welcome, since this project is itself AI-coded. You remain responsible for
everything you submit: review it, test it, and make sure it contains no copied Mojang code.

## License

By contributing, you agree that your contributions are licensed under the
[GNU GPL v3.0](LICENSE).
