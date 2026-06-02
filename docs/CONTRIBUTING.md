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

## Pull requests

- Keep changes focused — one logical change per PR when possible.
- Update `assets/SOURCES.toml` and `assets/maps/SOURCES.toml` if you add or replace assets.
- Document OSM-derived maps with `source = "osm"` and attribution in SOURCES.toml.
- Do not commit secrets, keystores, `.env` files, or `OpenFrontIO/proprietary/` assets.

## License

By contributing, you agree that your contributions will be licensed under the
[GNU Affero General Public License v3.0 or later](LICENSE), the same license
as the project.

Inbound contributions = outbound under AGPL-3.0-or-later. No separate CLA is required.

## Code of conduct

Be respectful and constructive. We want a welcoming community for everyone.

## Questions

Open an issue on GitHub for bugs, feature requests, or questions about contributing.
