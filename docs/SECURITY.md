# Security Policy

## Supported Versions

We only support the latest deployed version of the code on production servers (`shadowsofwar.io` and partner platforms).

| Component | Path / Crate | Target Stack | Status |
| --------- | ------------ | ------------ | ------ |
| sow-server / sow-relay | `sow-relay` | Rust / Tokio / `tokio-tungstenite` | :white_check_mark: |
| sow-database | `sow-database` | Rust / Axum / Valkey (Redis-RS) / JWT | :white_check_mark: |
| sow-client / sow-render | `sow-client` | Rust / WASM / WebGL (`wgpu`) | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability, please do not open a public issue. Instead, report it privately to:

- **Email:** info@shadowsofwar.io
- **Encryption:** (Optional) If you wish to encrypt your report, please email us to coordinate a secure keyshare.
- **Response window:** We will acknowledge receipt of your report within 48 hours and provide an estimated timeline for the fix.

## Scope & Priorities

### What We Care About Most
* **Authentication Bypass / Impersonation**: Forgery or exploitation of JWT tokens, or hijacking player profile linkage endpoints (`/profile/link`).
* **Database / State Manipulation**: Unauthorized command injection or arbitrary key modification in our Valkey/Redis instance.
* **Server Crash Vectors**: Resource exhaustion or panic triggers on the `sow-relay` or `sow-database` endpoints.
* **Remote Code Execution (RCE)**: Deserialization flaws or memory safety vulnerabilities during custom map loading (`sow-map`) or asset manifest resolution.

### What Is Out of Scope / Low Severity
* **Client-Side Cheat / Memory Tampering**: Since Shadows of War uses a lockstep multiplayer networking design, locally modified game clients will simply desync from the game loop and will not affect the integrity of matches on other players' clients.
* **Rate Limiting**: Minor rate-limiting issues on chat or match-finding (though reports regarding major orchestrator DDoS vectors are welcome).
* **XSS / CSRF**: The frontend marketing pages (`sow-web`) are static, and authentication tokens (`x-platform-auth`, JWT) are not stored in standard session cookies.
