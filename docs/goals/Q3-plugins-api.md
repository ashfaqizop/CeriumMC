# Q3: Plugins API

**Goal:** a first-class, stable API for extending CeriumMC.

> 📝 The detailed requirements for this milestone haven't been written yet. They will be defined
> after Q1 and Q2 are scoped, and this document will be updated then.

## Constraints already known

- Plugins must not be able to silently break [Q1](Q1-vanilla-parity.md) parity guarantees.
  Vanilla behaviour stays the baseline.
- The API boundary is security-sensitive and is covered by the [Security Policy](../../SECURITY.md).
