# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| latest on `main` | yes |
| older releases | best effort |

## Reporting a vulnerability

**Please do not open public GitHub issues for security vulnerabilities.**

Report security issues privately to the maintainer:

- **Email:** ohsalmeron@users.noreply.github.com (or open a GitHub Security Advisory on the repository)

Include:

- Description of the vulnerability
- Steps to reproduce
- Impact assessment (if known)
- Suggested fix (optional)

We aim to acknowledge reports within 7 days and provide a fix or mitigation timeline
when possible.

## Scope

In scope:

- `sow-server`, `sow-relay` — network-facing services
- `sow-client`, `sow-core` — game client and simulation
- Authentication, WebSocket protocol, and map/asset serving paths

Out of scope:

- Third-party dependencies (report upstream)
- OpenFrontIO reference tree (not part of this repository's release)
- Social engineering or physical attacks

## Safe harbor

We appreciate responsible disclosure and will credit reporters who wish to be named,
unless they prefer to remain anonymous.
