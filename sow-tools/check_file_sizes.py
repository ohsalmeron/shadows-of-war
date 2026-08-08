#!/usr/bin/env python3
"""Fail if first-party crate source files exceed the line-count budget."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_LINES = 750

# Static data tables and deferred UI modules (see README "Source file size guard").
CRATES: dict[str, set[Path]] = {
    "sow-client": {
        ROOT / "sow-client" / "src" / "hud" / "leaderboard" / "panel.rs",
        ROOT / "sow-client" / "src" / "render" / "frame" / "ui.rs",
        ROOT / "sow-client" / "src" / "render" / "interact" / "context_menu" / "build_popover.rs",
        ROOT / "sow-client" / "src" / "render" / "world" / "nameplates" / "render.rs",
    },
    "sow-render": set(),
    "sow-core": {
        ROOT / "sow-core" / "src" / "intent" / "nation" / "tests.rs",
    },
    "sow-data": {
        ROOT / "sow-data" / "src" / "colors" / "premium_colors.rs",
        ROOT / "sow-data" / "src" / "tribes" / "names.rs",
        ROOT / "sow-data" / "src" / "emoji" / "manifest.rs",
    },
    "sow-ui": {
        ROOT / "sow-ui" / "src" / "ui" / "hud" / "tabs" / "controls.rs",
        ROOT / "sow-ui" / "src" / "ui" / "main_menu" / "queue_overlay.rs",
        ROOT / "sow-ui" / "src" / "ui" / "main_menu" / "mod.rs",
    },
    "sow-audio": set(),
    "sow-map": set(),
    "sow-dist": {
        # CLI grew past 750 with the multi-worker relay deploy rework
        # (worker catalog, relay subcommand, prod/relay modules).
        ROOT / "sow-dist" / "src" / "main.rs",
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
