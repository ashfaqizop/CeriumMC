# Security Policy

## Supported versions

CeriumMC is pre-release research software (see the [AI disclaimer](README.md#ai-disclaimer)).
Security fixes go to the `main` branch only.

| Version | Supported |
|---|---|
| `main` | ✅ |
| Anything else | ❌ |

**Do not expose CeriumMC to untrusted players or the public internet** until a release says it
is production-ready.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Report it privately through GitHub:
**[Security → Report a vulnerability](https://github.com/ashfaqizop/CeriumMC/security/advisories/new)**.
If you can't use GitHub, email **ashfaqizop@gmail.com** with the subject `[CeriumMC security]`.

Please include:

- the affected commit or version;
- a description of the issue and its impact (crash, remote code execution, data loss, auth
  bypass, denial of service…);
- steps to reproduce, or a proof of concept (packets, world files, configuration);
- whether the issue also affects the vanilla server.

## What to expect

| Step | Target |
|---|---|
| Acknowledgement | within 72 hours |
| Initial assessment | within 7 days |
| Fix or mitigation plan | depends on severity; critical issues first |

We coordinate disclosure with you, and we credit reporters in the advisory unless you ask us not
to.

## Scope

**In scope**
- the network protocol implementation (malformed or malicious packets, decompression bombs,
  auth and encryption);
- world and region file parsing (`.mca`, and later `.HCR`);
- configuration and data-pack loading;
- the plugin API boundary, once it exists (Q3);
- pool networking, once it exists (Q5).

**Out of scope**
- vulnerabilities in Minecraft itself, the vanilla client, or Mojang services;
- issues that need an already-compromised host;
- social engineering.
