#!/usr/bin/env bash
# Pre-public-push safety checks. Run from repo root before making the repo public.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FAIL=0

red() { echo "FAIL: $*"; FAIL=1; }
green() { echo "OK: $*"; }

# 1. No keystore passwords in tracked Cargo.toml
if grep -E 'keystore_password|key_password' sow-client/Cargo.toml 2>/dev/null; then
  red "Keystore passwords found in sow-client/Cargo.toml"
else
  green "No keystore passwords in sow-client/Cargo.toml"
fi

# 2. .gitignore covers sensitive paths
for path in OpenFrontIO/ MapGenerator/ keystores/ sow-client/signing.local.toml; do
  if grep -qxF "$path" .gitignore 2>/dev/null; then
    green ".gitignore includes $path"
  else
    red ".gitignore missing $path"
  fi
done
if grep -qE '^\.env' .gitignore 2>/dev/null; then
  green ".gitignore includes .env*"
else
  red ".gitignore missing .env*"
fi

# 3. No proprietary OpenFront assets tracked
if git ls-files --error-unmatch OpenFrontIO/proprietary 2>/dev/null; then
  red "OpenFrontIO/proprietary/ is tracked by git"
elif [[ -d OpenFrontIO/proprietary ]] && git status --porcelain OpenFrontIO/proprietary 2>/dev/null | grep -q .; then
  red "OpenFrontIO/proprietary/ has untracked files — ensure it stays out of commits"
else
  green "No OpenFront proprietary/ in git index"
fi

# 4. LICENSE exists and is AGPL
if [[ -f LICENSE ]] && head -1 LICENSE | grep -qi 'AFFERO'; then
  green "LICENSE is AGPL"
else
  red "LICENSE missing or not AGPL"
fi

# 5. Workspace license is AGPL
if grep -q 'AGPL-3.0-or-later' Cargo.toml; then
  green "Cargo.toml workspace license is AGPL-3.0-or-later"
else
  red "Cargo.toml workspace license is not AGPL-3.0-or-later"
fi

# 6. Scan for hardcoded keystore passwords in tracked files (not env-var templates)
if git grep -l 'keystore_password\s*=\s*"[^$]' -- '*.toml' 2>/dev/null | grep -v signing.local.toml.example; then
  red "Hardcoded keystore password in tracked .toml (see above)"
else
  green "No hardcoded keystore_password in tracked .toml"
fi

# 7. OpenFrontIO/ and MapGenerator/ not in git index
for dir in OpenFrontIO MapGenerator; do
  if git ls-files "$dir" 2>/dev/null | grep -q .; then
    red "$dir/ has tracked files"
  else
    green "$dir/ not in git index"
  fi
done

# 8. Shipped maps only (northamerica + catalog + SOURCES)
BAD_MAPS=$(git ls-files 'assets/maps/' | grep -vE '^assets/maps/(northamerica/|catalog\.bin|SOURCES\.toml$)' || true)
if [[ -n "$BAD_MAPS" ]]; then
  red "Unexpected tracked paths under assets/maps/:"
  echo "$BAD_MAPS" | head -20
else
  green "assets/maps/ index is northamerica-only"
fi

# 9. credits maps_cc mentions development history
if grep -q 'development history\|historial de desarrollo' sow-lang/strings/en/credits.toml \
   sow-lang/strings/es/credits.toml 2>/dev/null; then
  green "credits maps_cc documents OpenFront map history"
else
  red "credits maps_cc missing history note (en/es)"
fi

# 10. OpenFront §7 footer string present
if grep -q 'openfront_footer' sow-lang/strings/en/credits.toml; then
  green "openfront_footer defined in credits strings"
else
  red "openfront_footer missing from credits.toml"
fi

echo ""
if [[ $FAIL -ne 0 ]]; then
  echo "Pre-push checks FAILED. Fix issues before making the repo public."
  exit 1
fi
echo "All pre-push checks passed."
