#!/usr/bin/env bash
# Regenerate NOTICE.deps from Cargo.lock (third-party Rust crates only).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/NOTICE.deps"

python3 - "$ROOT" "$OUT" <<'PY'
import json
import subprocess
import sys
from collections import defaultdict

root, out_path = sys.argv[1], sys.argv[2]
data = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--format-version=1", "--locked"],
        cwd=root,
        text=True,
    )
)
by_license: dict[str, set[str]] = defaultdict(set)
for pkg in data["packages"]:
    name = pkg["name"]
    if name.startswith("sow-"):
        continue
    lic = (pkg.get("license") or "UNKNOWN").strip()
    by_license[lic].add(f"{name} v{pkg['version']}")

lines = [
    "Rust Third-Party Crates (auto-generated)",
    "========================================",
    "",
    "Do not edit by hand. Regenerate:",
    "  ./scripts/generate_notice_deps.sh",
    "",
]
for lic in sorted(by_license.keys(), key=str.lower):
    lines.append(f"### {lic}")
    for crate in sorted(by_license[lic], key=str.lower):
        lines.append(f"- {crate}")
    lines.append("")

with open(out_path, "w", encoding="utf-8") as f:
    f.write("\n".join(lines))
print(f"Wrote {out_path} ({sum(len(v) for v in by_license.values())} crates)")
PY

chmod +x "$ROOT/scripts/generate_notice_deps.sh"
