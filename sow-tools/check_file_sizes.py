#!/usr/bin/env python3
"""Fail if first-party crate source files exceed the line-count budget."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_LINES = 600

CRATES: dict[str, set[Path]] = {
    "sow-client": set(),
    "sow-core": {
        ROOT / "sow-core" / "src" / "player" / "colors.rs",
        ROOT / "sow-core" / "src" / "intent" / "nation" / "tests.rs",
        ROOT / "sow-core" / "src" / "intent" / "mod.rs",
        ROOT / "sow-core" / "src" / "engine.rs",
        ROOT / "sow-core" / "src" / "tribes.rs",
        ROOT / "sow-core" / "src" / "execution" / "combat.rs",
        ROOT / "sow-core" / "src" / "pathfinding.rs",
    },
    "sow-ui": {
        ROOT / "sow-ui" / "src" / "ui" / "main_menu" / "mod.rs",
        ROOT / "sow-ui" / "src" / "ui" / "hud" / "tabs" / "controls.rs",
    },
    "sow-audio": set(),
    "sow-map": {
        ROOT / "sow-map" / "src" / "osm_coast.rs",
    },
    "sow-render": {
        ROOT / "sow-render" / "src" / "map_renderer.rs",
    },
    "sow-dist": {
        ROOT / "sow-dist" / "src" / "infra.rs",
    },
}


def check_crate(name: str, allowlist: set[Path]) -> list[tuple[int, Path]]:
    src = ROOT / name / "src"
    if not src.is_dir():
        return []
    violations: list[tuple[int, Path]] = []
    for path in sorted(src.rglob("*.rs")):
        if path in allowlist:
            continue
        count = sum(1 for _ in path.open("r", encoding="utf-8"))
        if count > MAX_LINES:
            violations.append((count, path))
    return violations


def main() -> int:
    failed = False
    total_allowlisted = 0
    for crate, allowlist in CRATES.items():
        total_allowlisted += len(allowlist)
        violations = check_crate(crate, allowlist)
        if violations:
            failed = True
            print(f"{crate} file size check failed (limit {MAX_LINES} lines):", file=sys.stderr)
            for count, path in violations:
                rel = path.relative_to(ROOT)
                print(f"  {count:4d}  {rel}", file=sys.stderr)

    if failed:
        print(f"allowlisted entries: {total_allowlisted}", file=sys.stderr)
        return 1

    print(
        f"OK: all checked crate .rs files <= {MAX_LINES} lines "
        f"({len(CRATES)} crates, {total_allowlisted} allowlisted)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
