#!/usr/bin/env bash
# Regenerate the Rust SBOM section in NOTICE from Cargo.lock metadata.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SBOM="$(mktemp)"
cargo metadata --format-version=1 | python3 -c "
import json,sys
m=json.load(sys.stdin)
licenses={}
for p in m['packages']:
    lic = (p.get('license') or 'UNKNOWN').replace('\n',' / ')
    licenses.setdefault(lic, set()).add(f\"{p['name']} v{p['version']}\")
lines = ['Rust Third-Party Crates', '========================', '']
for lic in sorted(licenses):
    lines.append(f'### {lic}')
    for c in sorted(licenses[lic]):
        lines.append(f'- {c}')
    lines.append('')
lines.extend([
    '### Vendored path dependencies (gitignored locally)',
    '',
    '- egui / egui_extras — MIT OR Apache-2.0 (path patch: egui/crates/egui)',
    '- winit — Apache-2.0 (path patch: winit/winit)',
    '- blade-graphics / blade-egui — MIT (path patch: blade/)',
    '',
    'Pin upstream commit hashes when publishing forks.',
    '',
])
print('\n'.join(lines))
" > "$SBOM"

python3 - "$SBOM" << 'PY'
import sys
from pathlib import Path
sbom = Path(sys.argv[1]).read_text()
notice = Path('NOTICE').read_text()
start = notice.find('Rust Third-Party Crates')
if start == -1:
    notice = notice.replace('<!-- SBOM_PLACEHOLDER -->', sbom)
else:
    notice = notice[:start] + sbom
Path('NOTICE').write_text(notice)
print('Updated NOTICE SBOM section')
PY

rm -f "$SBOM"
