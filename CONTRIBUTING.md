# Contributing to Shadows of War

Thank you for your interest in contributing!

## Getting started

1. Fork the repository and clone your fork.
2. Ensure `assets/gameplay/` is complete (fonts, emoji, HUD assets, and avatars are required by the gameplay client).
3. Build the workspace: `cargo build --workspace`
4. Run tests: `cargo test --workspace`
5. Format and lint before submitting:
   ```bash
   cargo fmt --all
   cargo clippy --workspace -- -D warnings
   ```

## Marketing site (`sow-web`)

Static HTML lives in `sow-web/site/` (landing, privacy, terms, cookies, support). The WASM game shell lives in `sow-web/shell/` — see `sow-web/README.md`. Do not embed game code in the marketing pages.

The current entrypoint does not expose a local WASM preview command; use the
production packaging path for a deployable web build and `./sow native` for a
local desktop run.

## Pull requests

- Keep changes focused — one logical change per PR when possible.
- Update `assets/SOURCES.toml` and `assets/maps/SOURCES.toml` if you add or replace assets.
- New shipped art (portraits, avatars): default to **CC BY-SA 4.0** per `docs/legal/LICENSE-ASSETS`; verify AI tool ToS before setting `license = "CC-BY-SA-4.0"`.
- Document OSM-derived maps with `source = "osm"` and attribution in SOURCES.toml.
- Do not commit secrets, `sow-dist/deploy/keystores/`, `.env` files, or `OpenFrontIO/proprietary/` assets.

## License

By contributing, you agree that your contributions will be licensed under the
[GNU Affero General Public License v3.0 or later](LICENSE), the same license
as the project.

Inbound contributions = outbound under AGPL-3.0-or-later. No separate CLA is required.

## Attribution

- **Player-facing UI:** brand (`© Shadows of War`), OpenFront attribution on the main menu, full notices in Credits — see README “Attribution policy”.
- **Marketing HTML:** same footer pattern as `sow-web/site/`; do not add the copyright holder’s personal name.
- **Legal files:** update only under `docs/legal/` (COPYRIGHT, NOTICE, LICENSE-ASSETS). When `Cargo.lock` changes, regenerate `docs/legal/NOTICE.deps` with `cargo metadata --format-version=1 --locked` and the Python snippet in that file’s header comment (not part of deploy).
- **Upstream OpenFront:** legal entity is OpenFront Inc. and Contributors (see their LICENSING.md); player UI uses “OpenFront and Contributors”.

## Code of conduct

Be respectful and constructive. We want a welcoming community for everyone.

## Faster debug builds

For UI-only work, you can speed up clean debug compiles by lowering dependency optimization in a local `.cargo/config.toml` (not committed):

```toml
[profile.dev.package."*"]
opt-level = 1
```

The workspace default uses `opt-level = 3` for dependencies so debug runs stay smooth; lowering it trades runtime speed for compile time.

Native dev tools (FPS overlay, dev sidebar, map shader sliders) require `--features dev`:

```bash
cargo run -p sow-client --features dev
```
