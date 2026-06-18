#!/usr/bin/env python3
"""Fail if sow-client source files exceed the line-count budget."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT_SRC = ROOT / "sow-client" / "src"
MAX_LINES = 600

# Files over budget today — shrink this list as follow-up splits land.
ALLOWLIST = {
    CLIENT_SRC / "render" / "frame.rs",
    CLIENT_SRC / "net" / "update.rs",
    CLIENT_SRC / "hud" / "leaderboard.rs",
    CLIENT_SRC / "loader.rs",
    CLIENT_SRC / "render" / "interact" / "context_menu.rs",
    CLIENT_SRC / "render" / "world" / "buildings.rs",
    CLIENT_SRC / "render" / "world" / "nameplates.rs",
}


def main() -> int:
    violations: list[tuple[int, Path]] = []
    for path in sorted(CLIENT_SRC.rglob("*.rs")):
        if path in ALLOWLIST:
            continue
        count = sum(1 for _ in path.open("r", encoding="utf-8"))
        if count > MAX_LINES:
            violations.append((count, path))

    if violations:
        print(f"sow-client file size check failed (limit {MAX_LINES} lines):", file=sys.stderr)
        for count, path in violations:
            rel = path.relative_to(ROOT)
            print(f"  {count:4d}  {rel}", file=sys.stderr)
        print(f"allowlisted: {len(ALLOWLIST)} file(s)", file=sys.stderr)
        return 1

    print(f"OK: all sow-client .rs files <= {MAX_LINES} lines ({len(ALLOWLIST)} allowlisted)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
