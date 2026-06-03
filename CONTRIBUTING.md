# Contributing to Shadows of War

Thank you for your interest in contributing!

## Getting started

1. Fork the repository and clone your fork.
2. Build the workspace: `cargo build --workspace`
3. Run tests: `cargo test --workspace`
4. Format and lint before submitting:
   ```bash
   cargo fmt --all
   cargo clippy --workspace -- -D warnings
   ```

## Marketing site (`sow-web`)

Static HTML lives in `sow-web/site/` (landing, privacy, terms). The WASM game shell lives in `sow-web/shell/` — see `sow-web/README.md`. Do not embed game code in the marketing pages.

```bash
cd sow-web/site && python3 -m http.server 8787   # edit HTML/CSS and refresh the browser
```

## Pull requests

- Keep changes focused — one logical change per PR when possible.
- Update `assets/SOURCES.toml` and `assets/static/maps/SOURCES.toml` if you add or replace assets.
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

## Questions

Open an issue on GitHub for bugs, feature requests, or questions about contributing.
