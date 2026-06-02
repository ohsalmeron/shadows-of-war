#!/usr/bin/env bash
# Shadows of War — build & deploy (single entrypoint)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
export CARGO_TARGET_DIR="${ROOT}/target"

usage() {
  cat <<EOF
Usage: ./scripts/sow.sh <command> [args]

  local|l            Debug native server + 2 clients (fast iteration)
  ptr|p              Deploy staging web to ptr.shadowsofwar.io
  cloud|c            Deploy production web to shadowsofwar.io
  package|pkg [portal]  Build dist/play/ + portal zip (default: crazygames)
  site               Run sow-site SSR locally (landing + legal pages)
  android|a [native|n|webview|w]

Legacy wrappers: ./scripts/cloud.sh, ptr.sh, local.sh, android.sh
EOF
}

sow_bump_version() {
  local vf="${ROOT}/.version"
  [[ -f "${vf}" ]] || echo "0.1.0" > "${vf}"
  local cur patch
  cur=$(cat "${vf}")
  if [[ "${cur}" == *.* ]]; then patch=$(echo "${cur}" | rev | cut -d. -f1 | rev); else patch="${cur}"; fi
  CLEAN_VERSION="0.1.$((patch + 1))"
  echo "${CLEAN_VERSION}" > "${vf}"
  echo "Version bumped to ${CLEAN_VERSION}"
}

print_agpl_release_steps() {
  local tag="v${CLEAN_VERSION}"
  echo ""
  echo "AGPL corresponding source:"
  echo "  1. Make GitHub repo PUBLIC: github.com/ohsalmeron/shadows-of-war"
  echo "  2. git tag -a ${tag} -m "Release ${tag}" && git push origin ${tag}"
  echo "  3. https://github.com/ohsalmeron/shadows-of-war/tree/${tag}"
  if git rev-parse "${tag}" >/dev/null 2>&1; then echo "  (tag ${tag} exists)"
  elif git diff --quiet && git diff --cached --quiet 2>/dev/null; then
    git tag -a "${tag}" -m "Release ${tag}"
    echo "  Created local tag ${tag} — push when ready"
  else echo "  (tag after commit/push)"
  fi
}

sow_compile_wasm_release() {
  echo "==> Compiling WASM client (wasm-release)..."
  RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --profile wasm-release -p sow-client --target wasm32-unknown-unknown
  WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/wasm-release/sow_client.wasm"
}

stage_core_assets() {
  local play="$1"
  echo "==> Staging minimal core assets (fonts, northamerica, UI boot)"
  mkdir -p "${play}/assets/fonts" "${play}/assets/maps/northamerica"
  rsync -a "${ROOT}/assets/fonts/" "${play}/assets/fonts/"
  cp -a "${ROOT}/assets/maps/northamerica/map.bin.br" \
    "${ROOT}/assets/maps/northamerica/thumbnail.webp" \
    "${play}/assets/maps/northamerica/"
  cargo run -q -p sow-tools --bin write-play-catalog -- \
    --maps-root "${ROOT}/assets/maps" \
    --output "${play}/assets/maps/catalog.bin" \
    northamerica
  copy_web_loader_assets "${play}/assets/ui"
}

stage_published_assets() {
  local dest="$1"
  echo "==> Staging nginx-published assets tree (${dest})"
  mkdir -p "${dest}/fonts"
  rsync -a "${ROOT}/assets/fonts/" "${dest}/fonts/"
  copy_web_loader_assets "${dest}/ui"
}

prune_querystring_artifacts() {
  local root_dir="$1"
  python3 - "${root_dir}" <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])
removed = 0
for path in root.rglob("*"):
    if not path.is_file():
        continue
    if "?v=" in path.name:
        path.unlink(missing_ok=True)
        removed += 1
if removed:
    print(f"Removed {removed} querystring artifact file(s) under {root}")
PY
}

sow_assemble_play_dist() {
  local portal="${1:-}"
  local play="${ROOT}/dist/play"
  echo "==> Packaging game shell (dist/play/)..."
  mkdir -p "${play}" && rm -rf "${play:?}"/*
  BUILD_TS=$(date +%s)
  JS_FILE="sow_client_${BUILD_TS}.js"
  WASM_FILE="sow_client_${BUILD_TS}_bg.wasm"
  local wasm_bindgen_bin
  wasm_bindgen_bin=$(find_wasm_bindgen)
  "${wasm_bindgen_bin}" --out-dir "${play}" --target web --out-name "sow_client_${BUILD_TS}" --no-typescript "${WASM_IN}"
  stage_core_assets "${play}"
  cp -a web/favicon_io/* "${play}/" 2>/dev/null || true
  cp web/sow.svg "${play}/sow.svg"
  mkdir -p "${play}/sdk" && cp -a web/sdk/. "${play}/sdk/"
  build_index_html "${ROOT}/web/index.html.template" "${play}/index.html" "${CLEAN_VERSION}" "${JS_FILE}" "${WASM_FILE}" "${BUILD_TS}"
  [[ "${portal}" == "crazygames" ]] && inject_crazygames_portal "${play}/index.html"
  minify_js_shim "${play}/${JS_FILE}"
  optimize_wasm_bundle "${play}/${WASM_FILE}"
  if command -v brotli >/dev/null 2>&1; then
    brotli -f -Z "${play}/${WASM_FILE}" & p1=$!
    brotli -f -Z "${play}/${JS_FILE}" & p2=$!
    wait "${p1}" "${p2}"
    echo "Brotli compression finished."
  fi
  sed -e "s/__VERSION__/${CLEAN_VERSION}/g" -e "s/__JS_FILE__/${JS_FILE}/g" \
    -e "s/__WASM_FILE__/${WASM_FILE}/g" -e "s/__BUILD_TS__/${BUILD_TS}/g" \
    "${ROOT}/web/sw.js.template" > "${play}/sw.js"
  prune_querystring_artifacts "${play}"
  mkdir -p "${ROOT}/dist"
  cp web/sow.svg "${ROOT}/dist/sow.svg"
  write_game_manifest "${play}" "${JS_FILE}" "${WASM_FILE}" "${BUILD_TS}"
  sow_assemble_site_assets "${BUILD_TS}"
  echo "Bundle sizes: ${WASM_FILE}=$(( $(stat -c%s "${play}/${WASM_FILE}") / 1024 )) KB, ${JS_FILE}=$(( $(stat -c%s "${play}/${JS_FILE}") / 1024 )) KB"
}

write_game_manifest() {
  local play="$1" js="$2" wasm="$3" ts="$4"
  cat > "${play}/game-manifest.json" <<EOF
{"js":"${js}","wasm":"${wasm}","build_ts":"${ts}","version":"${CLEAN_VERSION}"}
EOF
  cp "${play}/game-manifest.json" "${ROOT}/sow-site/game-manifest.json"
  cp "${play}/game-manifest.json" "${ROOT}/dist/game-manifest.json"
}

sow_assemble_site_assets() {
  local build_ts="$1"
  local play="${ROOT}/dist/play"
  cp "${ROOT}/web/boot.js" "${ROOT}/dist/boot.js"
  minify_js_shim "${ROOT}/dist/boot.js"
  cp "${ROOT}/web/loader.js" "${play}/loader.js"
  minify_js_shim "${play}/loader.js"
  sed -e "s/__BUILD_TS__/${build_ts}/g" \
    "${ROOT}/web/sw-site.js.template" > "${ROOT}/dist/sw.js"
}

sow_build_site() {
  echo "==> Compiling sow-site (SSR)..."
  export SOW_GAME_MANIFEST_PATH="${ROOT}/sow-site/game-manifest.json"
  if cargo build --release -p sow-site --target x86_64-unknown-linux-musl 2>/dev/null; then
    SITE_BIN="${CARGO_TARGET_DIR}/x86_64-unknown-linux-musl/release/sow-site"
  else
    cargo build --release -p sow-site
    SITE_BIN="${CARGO_TARGET_DIR}/release/sow-site"
  fi
}

deploy_sow_site_systemd() {
  local u="$1" h="$2" unit="$3" listen="$4" workdir="$5" bin_remote="$6"
  local unit_file="${unit%.service}.service"
  local manifest="${ROOT}/sow-site/game-manifest.json"
  ssh "${u}@${h}" "mkdir -p '${workdir}'"
  rsync -avz "${SITE_BIN}" "${u}@${h}:${bin_remote}"
  if [[ -f "${manifest}" ]]; then
    rsync -avz "${manifest}" "${u}@${h}:${workdir}/game-manifest.json"
  fi
  ssh "${u}@${h}" "UNIT='${unit_file}' LISTEN='${listen}' WORKDIR='${workdir}' BIN='${bin_remote}' bash -s" <<'REMOTE'
set -euo pipefail
cat << SYSTEMD | sudo tee "/etc/systemd/system/${UNIT}" > /dev/null
[Unit]
Description=Shadows of War SSR site
After=network.target

[Service]
Type=simple
User=bizkit
WorkingDirectory=${WORKDIR}
Environment="SOW_SITE_LISTEN=${LISTEN}"
Environment="SOW_GAME_MANIFEST_PATH=${WORKDIR}/game-manifest.json"
ExecStart=${BIN}
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
SYSTEMD
sudo rm -f "/etc/systemd/system/${UNIT%.service}"
sudo systemctl daemon-reload
sudo systemctl enable --now "${UNIT}"
sudo systemctl restart "${UNIT}"
systemctl is-active --quiet "${UNIT}"
echo "✅ ${UNIT} running on ${LISTEN}"
REMOTE
}


# Deploy environment checks (sourced by cloud.sh / ptr.sh).

find_cwebp() {
    if command -v cwebp >/dev/null 2>&1; then
        command -v cwebp
        return 0
    fi
    local p
    for p in /usr/bin/cwebp /usr/local/bin/cwebp; do
        if [[ -x "${p}" ]]; then
            echo "${p}"
            return 0
        fi
    done
    return 1
}

find_terser() {
    if command -v terser >/dev/null 2>&1; then
        echo "terser"
        return 0
    fi
    if command -v npx >/dev/null 2>&1; then
        echo "npx --yes terser"
        return 0
    fi
    return 1
}

find_wasm_opt() {
    if command -v wasm-opt >/dev/null 2>&1; then
        command -v wasm-opt
        return 0
    fi
    local p
    for p in /usr/bin/wasm-opt /usr/local/bin/wasm-opt; do
        if [[ -x "${p}" ]]; then
            echo "${p}"
            return 0
        fi
    done
    return 1
}

# Shrink wasm-bindgen output before brotli (optional; install binaryen for best size).
optimize_wasm_bundle() {
    local wasm_path="$1"
    local wasm_opt_bin
    if wasm_opt_bin=$(find_wasm_opt); then
        echo "==> wasm-opt -Oz (binaryen)..."
        "${wasm_opt_bin}" -Oz --strip-debug --vacuum \
            --enable-bulk-memory --enable-nontrapping-float-to-int \
            "${wasm_path}" -o "${wasm_path}"
        echo "✅ wasm-opt finished."
    else
        echo "⚠️  wasm-opt not found — install binaryen for smaller WASM (Arch: binaryen, Debian: binaryen)"
    fi
}

wasm_bindgen_version_from_lock() {
  awk '
    /^name = "wasm-bindgen"$/ {
      getline
      if ($1 == "version") {
        gsub(/"/, "", $3)
        print $3
        exit
      }
    }
  ' "${ROOT}/Cargo.lock"
}

ensure_wasm_bindgen_cli() {
  local want="$1"
  local bindgen_bin="${HOME}/.cargo/bin/wasm-bindgen"
  local have=""

  if [[ -x "${bindgen_bin}" ]]; then
    have=$("${bindgen_bin}" --version 2>/dev/null | awk '{print $2}' || true)
    if [[ "${have}" == "${want}" ]]; then
      echo "${bindgen_bin}"
      return 0
    fi
  fi

  echo "==> Installing wasm-bindgen-cli ${want} (CLI was ${have:-missing})..."
  cargo install -f wasm-bindgen-cli --version "${want}"
  echo "${bindgen_bin}"
}

find_wasm_bindgen() {
  local want
  want=$(wasm_bindgen_version_from_lock)
  if [[ -z "${want}" ]]; then
    echo "❌ Could not read wasm-bindgen version from Cargo.lock" >&2
    return 1
  fi
  ensure_wasm_bindgen_cli "${want}"
}

install_cwebp_if_missing() {
    find_cwebp >/dev/null && return 0
    [[ "${SOW_SKIP_TOOL_INSTALL:-}" == "1" ]] && return 1

    if command -v pacman >/dev/null 2>&1; then
        # libwebp = libs only; the cwebp binary is in libwebp-utils.
        sudo pacman -S --needed --noconfirm libwebp-utils
    elif command -v apt-get >/dev/null 2>&1; then
        sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq webp
    else
        return 1
    fi
    find_cwebp >/dev/null
}

check_local_build_tools() {
    local missing=()
    local cwebp_bin terser_cmd wasm_bindgen_bin

    if ! cwebp_bin=$(find_cwebp); then
        install_cwebp_if_missing || missing+=("cwebp (Arch: libwebp-utils, Debian: webp)")
        cwebp_bin=$(find_cwebp) || true
    fi

    command -v brotli >/dev/null 2>&1 || missing+=("brotli")
    if ! wasm_bindgen_bin=$(find_wasm_bindgen); then
        missing+=("wasm-bindgen-cli (auto-install failed)")
    fi
    find_terser >/dev/null || missing+=("terser or npx")

    if ((${#missing[@]})); then
        echo "❌ Missing local build tools:"
        printf '   - %s\n' "${missing[@]}"
        exit 1
    fi

    terser_cmd=$(find_terser)
    wasm_opt_hint=""
    find_wasm_opt >/dev/null || wasm_opt_hint=" (wasm-opt/binaryen optional, recommended)"
    echo "✅ Build tools: cwebp=${cwebp_bin} brotli=$(command -v brotli) wasm-bindgen=${wasm_bindgen_bin} terser=${terser_cmd}${wasm_opt_hint}"
}

check_vps_ready() {
    local vps_user="$1" vps_ip="$2" nginx_site="$3"
    local systemd_service="${4:-sow-server}"
    ssh "${vps_user}@${vps_ip}" "NGINX_SITE='${nginx_site}' SYSTEMD_SERVICE='${systemd_service}' bash -s" <<'REMOTE'
set -euo pipefail
export PATH="/usr/sbin:/usr/bin:/sbin:/bin:${PATH}"
fail=0
/usr/sbin/nginx -v >/dev/null 2>&1 || { echo "❌ nginx not installed"; fail=1; }
systemctl is-active --quiet nginx || { echo "❌ nginx not running"; fail=1; }
dpkg -s libnginx-mod-http-brotli-static >/dev/null 2>&1 \
    || { echo "❌ libnginx-mod-http-brotli-static not installed"; fail=1; }
[[ -f "${NGINX_SITE}" ]] || { echo "❌ nginx site missing: ${NGINX_SITE}"; fail=1; }
if systemctl is-active --quiet "${SYSTEMD_SERVICE}" 2>/dev/null; then
    echo "✅ VPS: nginx + brotli-static + ${SYSTEMD_SERVICE}"
else
    echo "✅ VPS: nginx + brotli-static (${SYSTEMD_SERVICE} will restart at end)"
fi
exit "${fail}"
REMOTE
}

sync_vps_nginx() {
    local vps_user="$1" vps_ip="$2" nginx_site="$3" local_conf="$4"
    local local_hash
    local_hash=$(md5sum "${local_conf}" | awk '{print $1}')

    scp "${local_conf}" "${vps_user}@${vps_ip}:/tmp/sow-nginx.conf"
    ssh "${vps_user}@${vps_ip}" "NGINX_SITE='${nginx_site}' LOCAL_HASH='${local_hash}' bash -s" <<'REMOTE'
set -euo pipefail
export PATH="/usr/sbin:/usr/bin:/sbin:/bin:${PATH}"
changed=0

if ! dpkg -s libnginx-mod-http-brotli-static >/dev/null 2>&1; then
    echo "==> Installing libnginx-mod-http-brotli-static on VPS..."
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq libnginx-mod-http-brotli-static
    changed=1
fi

remote_hash=""
if [[ -f "${NGINX_SITE}" ]]; then
    remote_hash=$(md5sum "${NGINX_SITE}" | awk '{print $1}')
fi

site_name="$(basename "${NGINX_SITE}")"
enabled_site="/etc/nginx/sites-enabled/${site_name}"

if [[ "${remote_hash}" != "${LOCAL_HASH}" ]]; then
    echo "==> Updating nginx site config..."
    sudo cp /tmp/sow-nginx.conf "${NGINX_SITE}"
    if [[ -f "${enabled_site}" && ! -L "${enabled_site}" ]]; then
        sudo cp /tmp/sow-nginx.conf "${enabled_site}"
    else
        sudo ln -sf "${NGINX_SITE}" "${enabled_site}"
    fi
    changed=1
else
    echo "✅ Nginx config unchanged."
fi

if [[ "${changed}" -eq 1 ]]; then
    sudo nginx -t
    sudo systemctl reload nginx
    echo "✅ Nginx reloaded."
fi
REMOTE
}

verify_prod_headers() {
  local base_url="$1"
  local play_url="${base_url%/}/play/"
  local wasm js
  wasm=$(curl -fsS "${play_url}game-manifest.json" | grep -oE 'sow_client_[0-9]+_bg\.wasm' | head -1) \
    || wasm=$(curl -fsS "${play_url}" 2>/dev/null | grep -oE 'sow_client_[0-9]+_bg\.wasm' | head -1) \
    || return 1
  js=$(curl -fsS "${play_url}game-manifest.json" | grep -oE 'sow_client_[0-9]+\.js' | head -1) \
    || js=$(curl -fsS "${play_url}" 2>/dev/null | grep -oE 'sow_client_[0-9]+\.js' | head -1) \
    || return 1

  echo "==> Verifying live headers..."
  curl -fsSI -H 'Accept-Encoding: br' "${play_url}${wasm}" | grep -qi 'content-encoding: br' \
    || { echo "❌ WASM not served with brotli"; return 1; }
  curl -fsSI "${base_url%/}/boot.js" | grep -qi 'cache-control' \
    || { echo "❌ /boot.js missing cache header"; return 1; }
  curl -fsSI "${play_url}assets/ui/loader_empty.webp" | grep -qi 'cache-control:.*max-age' \
    || { echo "❌ loader webp missing cache header"; return 1; }
  curl -fsS "${base_url%/}/health" | grep -qi '^ok$' \
    || { echo "❌ sow-site /health failed"; return 1; }
  echo "✅ Live: brotli WASM, cache headers, sow-site OK (${wasm}, ${js})"
}

ptr_dns_resolves() {
    python3 -c "import json,urllib.request; d=json.load(urllib.request.urlopen('https://dns.google/resolve?name=ptr.shadowsofwar.io&type=A',timeout=10)); raise SystemExit(0 if any(a.get('data')=='35.239.160.167' for a in d.get('Answer',[])) else 1)" 2>/dev/null
}

ensure_ptr_dns() {
    if ptr_dns_resolves; then
        echo "✅ DNS: ptr.shadowsofwar.io -> 35.239.160.167"
        return 0
    fi
    echo "⚠️  DNS: ptr.shadowsofwar.io no resuelve — añade registro A en Neubox (Registros DNS): ptr -> 35.239.160.167"
    return 1
}


PROD_ASSETS_USER="bizkit"
PROD_ASSETS_HOST="35.239.160.167"
PROD_ASSETS_PATH="/var/www/shadowsofwar.io/html/assets"

check_leader_portraits_complete() {
    local leaders_src="${ROOT}/assets/ui/leaders"
    python3 - "${leaders_src}" <<'PY'
import sys
from pathlib import Path

src = Path(sys.argv[1])
# Keep in sync with sow_core::player::Leader::ALL display names.
leaders = [
    "Caesar", "Cleopatra", "Ragnar", "Sun Tzu", "Alexander", "Genghis Khan",
    "Richard the Lionheart", "Vercingetorix", "Boudica", "Lady Six Sky",
    "Leonidas", "Napoleon",
]
missing = []
for name in leaders:
    slug = name.lower().replace(" ", "_")
    for form in ("desktop", "mobile"):
        path = src / f"{slug}_{form}.webp"
        if not path.is_file():
            missing.append(path)
if missing:
    print("❌ Missing leader portrait file(s):", file=sys.stderr)
    for path in missing:
        print(f"   {path}", file=sys.stderr)
    raise SystemExit(1)
PY
}

copy_leader_portraits() {
    local dest="${1:-dist/assets/ui/leaders}"
    local leaders_src="${ROOT}/assets/ui/leaders"
    if [[ ! -d "${leaders_src}" ]]; then
        echo "❌ Missing leader portraits: ${leaders_src}"
        exit 1
    fi
    check_leader_portraits_complete
    mkdir -p "${dest}"
    cp -a "${leaders_src}/." "${dest}/"
}

deploy_prod_published_assets() {
    local u="${1:-${PROD_ASSETS_USER}}" h="${2:-${PROD_ASSETS_HOST}}"
    echo "==> Deploying prod published assets (${u}@${h}:${PROD_ASSETS_PATH})"
    ssh "${u}@${h}" "mkdir -p ${PROD_ASSETS_PATH}"
    rsync -avz --delete "${ROOT}/dist/assets-publish/" "${u}@${h}:${PROD_ASSETS_PATH}/"
}

verify_prod_published_assets() {
    local base="https://shadowsofwar.io/assets"
    local path code
    echo "==> Verifying prod published assets..."
    for path in \
        "ui/leaders/caesar_desktop.webp" \
        "ui/leaders/richard_the_lionheart_desktop.webp" \
        "fonts/JockeyOne-Regular.ttf"; do
        code=$(curl -sS -o /dev/null -w "%{http_code}" "${base}/${path}")
        if [[ "${code}" != "200" ]]; then
            echo "❌ prod published asset failed: ${base}/${path} -> HTTP ${code}"
            return 1
        fi
    done
    echo "✅ prod published assets OK"
}

copy_web_loader_assets() {
    local dest="${1:-dist/assets/ui}"
    local ui_src="${ROOT}/assets/ui"
    local cwebp_bin
    mkdir -p "${dest}"

    if ! cwebp_bin=$(find_cwebp); then
        echo "❌ cwebp not found — run check_local_build_tools first"
        exit 1
    fi

    "${cwebp_bin}" -q 82 -resize 1032 256 "${ui_src}/loader_empty.webp" -o "${dest}/loader_empty.webp"
    "${cwebp_bin}" -q 82 -resize 1032 256 "${ui_src}/loader_full.webp" -o "${dest}/loader_full.webp"
    "${cwebp_bin}" -q 82 -resize 720 1280 "${ui_src}/sow-splash-mobile.webp" -o "${dest}/sow-splash-mobile.webp"
    cp "${ui_src}/sow-splash-desktop.webp" "${dest}/sow-splash-desktop.webp"
    copy_leader_portraits "${dest}/leaders"
}

# Inline web/loader.js into the loader marker of a built dist/index.html.
inline_loader_into_index() {
    local html_path="$1"
    python3 - "${ROOT}/web/loader.js" "${html_path}" <<'PY'
import sys
from pathlib import Path

loader = Path(sys.argv[1])
html_path = Path(sys.argv[2])
js = loader.read_text(encoding="utf-8").replace("</script>", "<\\/script>")
html = html_path.read_text(encoding="utf-8")
marker = "/* __INLINE_LOADER_JS__ */"
if marker in html:
    html = html.replace(marker, js, 1)
elif '<script src="./loader.js"></script>' in html:
    html = html.replace(
        '<script src="./loader.js"></script>',
        "<script>\n" + js + "\n</script>",
        1,
    )
else:
    raise SystemExit("index.html: no loader injection point")
html_path.write_text(html, encoding="utf-8")
PY
}

# Render web/index.html.template -> dist/index.html with build tokens, then inline loader.js.
# Portal SDK / boot vars are NOT templated here (one HTML for website + PTR + CrazyGames base).
# CrazyGames-only injection is handled separately by inject_crazygames_portal.
build_index_html() {
    local template="$1" out="$2" version="$3" js_file="$4" wasm_file="$5" build_ts="$6"
    sed -e "s/__VERSION__/${version}/g" \
        -e "s/__JS_FILE__/${js_file}/g" \
        -e "s/__WASM_FILE__/${wasm_file}/g" \
        -e "s/__BUILD_TS__/${build_ts}/g" \
        "${template}" > "${out}"
    inline_loader_into_index "${out}"
}

# CrazyGames package build only: replace the PORTAL_SDK_SLOT / PORTAL_BOOT_SLOT marker
# lines in dist/index.html with the real SDK <script> tag and portal boot vars.
inject_crazygames_portal() {
    local html_path="$1"
    local sdk_tag='    <script src="https://sdk.crazygames.com/crazygames-sdk-v3.js" async></script>'
    local boot_js='        window.SOW_PORTAL = "crazygames"; window.SOW_WS_URL = "wss://shadowsofwar.io/ws/"; window.SOW_MAPS_URL = "https://shadowsofwar.io/maps"; window.SOW_ASSETS_URL = "https://shadowsofwar.io/assets";'
    python3 - "${html_path}" "${sdk_tag}" "${boot_js}" <<'PY'
import sys
from pathlib import Path

html_path = Path(sys.argv[1])
sdk_tag, boot_js = sys.argv[2], sys.argv[3]
lines = html_path.read_text(encoding="utf-8").splitlines()
out = []
replaced_sdk = replaced_boot = False
for line in lines:
    if "PORTAL_SDK_SLOT" in line:
        out.append(sdk_tag)
        replaced_sdk = True
    elif "PORTAL_BOOT_SLOT" in line:
        out.append(boot_js)
        replaced_boot = True
    else:
        out.append(line)
if not (replaced_sdk and replaced_boot):
    raise SystemExit(
        f"inject_crazygames_portal: missing slot(s) sdk={replaced_sdk} boot={replaced_boot}"
    )
html_path.write_text("\n".join(out) + "\n", encoding="utf-8")
PY
}

minify_js_shim() {
    local js_file="$1"
    local terser_cmd
    if [[ ! -f "${js_file}" ]]; then
        return 0
    fi
    if ! terser_cmd=$(find_terser); then
        echo "❌ terser not found — run check_local_build_tools first"
        exit 1
    fi
    # shellcheck disable=SC2086
    ${terser_cmd} "${js_file}" -c -m --module -o "${js_file}.min" \
        && mv "${js_file}.min" "${js_file}"
}

cmd_package() {
  local portal="${1:-crazygames}"
  echo "Packaging for portal: ${portal}"
  check_local_build_tools
  sow_bump_version
  sow_compile_wasm_release
  sow_assemble_play_dist "${portal}"
  local z="shadows-of-war-${portal}.zip"
  rm -f "${ROOT}/${z}"
  (cd "${ROOT}/dist/play" && zip -r -q "../../${z}" .)
  echo "Portal package: ${ROOT}/${z} ($(du -sh "${ROOT}/${z}" | cut -f1))"
  if [[ -f "${ROOT}/dist/play/"*_bg.wasm.br ]]; then
    echo "WASM .br: $(du -sh "${ROOT}"/dist/play/*_bg.wasm.br | cut -f1)"
  fi
  print_agpl_release_steps
}

cmd_cloud() {
  check_local_build_tools
  check_vps_ready "bizkit" "35.239.160.167" "/etc/nginx/sites-available/shadowsofwar.io"
  sow_bump_version
  sow_compile_wasm_release
  local sb rb
  if cargo build --release -p sow-server --target x86_64-unknown-linux-musl && cargo build --release -p sow-relay --target x86_64-unknown-linux-musl; then
    sb=target/x86_64-unknown-linux-musl/release/sow-server; rb=target/x86_64-unknown-linux-musl/release/sow-relay
  else
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    cargo build --release -p sow-relay --target x86_64-unknown-linux-gnu
    sb=target/x86_64-unknown-linux-gnu/release/sow-server; rb=target/x86_64-unknown-linux-gnu/release/sow-relay
  fi
  sow_assemble_play_dist ""
  stage_published_assets "${ROOT}/dist/assets-publish"
  sow_build_site
  local u=bizkit h=35.239.160.167
  ssh "${u}@${h}" "mkdir -p /var/www/shadowsofwar.io/html/play"
  rsync -avz --delete --exclude='*.bin' "${ROOT}/dist/play/" "${u}@${h}:/var/www/shadowsofwar.io/html/play/" & w1=$!
  deploy_prod_published_assets "${u}" "${h}" & w6=$!
  rsync -avz "${ROOT}/dist/sow.svg" "${u}@${h}:/var/www/shadowsofwar.io/html/sow.svg" & w5=$!
  rsync -avz "${ROOT}/dist/boot.js" "${u}@${h}:/var/www/shadowsofwar.io/html/boot.js" & w7=$!
  rsync -avz "${ROOT}/dist/sw.js" "${u}@${h}:/var/www/shadowsofwar.io/html/sw.js" & w8=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar"
  rsync -avz "${sb}" "${u}@${h}:/home/bizkit/shadowsofwar/sow-server" & w2=$!
  rsync -avz "${rb}" "${u}@${h}:/home/bizkit/shadowsofwar/sow-relay" & w3=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar/assets/maps"
  rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' assets/maps/ "${u}@${h}:/home/bizkit/shadowsofwar/assets/maps/" & w4=$!
  wait "${w1}" "${w2}" "${w3}" "${w4}" "${w5}" "${w6}" "${w7}" "${w8}"
  verify_prod_published_assets
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/shadowsofwar.io" "${ROOT}/nginx_config.conf"
  deploy_sow_site_systemd "${u}" "${h}" "sow-site" "127.0.0.1:8787" \
    "/home/bizkit/shadowsofwar" "/home/bizkit/shadowsofwar/sow-site"
  ssh -t "${u}@${h}" "sudo systemctl enable --now sow-redis 2>/dev/null; sudo systemctl restart sow-server"
  verify_prod_headers "https://shadowsofwar.io" || true
  echo "Deployed v${CLEAN_VERSION} -> https://shadowsofwar.io"
  print_agpl_release_steps
}

cmd_ptr() {
  check_local_build_tools
  ensure_ptr_dns || true
  check_vps_ready "bizkit" "shadowsofwar.io" "/etc/nginx/sites-available/ptr.shadowsofwar.io" "sow-server-ptr"
  sow_bump_version
  sow_compile_wasm_release
  local sb rb
  if cargo build --release -p sow-server --target x86_64-unknown-linux-musl && cargo build --release -p sow-relay --target x86_64-unknown-linux-musl; then
    sb=target/x86_64-unknown-linux-musl/release/sow-server; rb=target/x86_64-unknown-linux-musl/release/sow-relay
  else
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    cargo build --release -p sow-relay --target x86_64-unknown-linux-gnu
    sb=target/x86_64-unknown-linux-gnu/release/sow-server; rb=target/x86_64-unknown-linux-gnu/release/sow-relay
  fi
  sow_assemble_play_dist ""
  stage_published_assets "${ROOT}/dist/assets-publish"
  sow_build_site
  local u=bizkit h=shadowsofwar.io
  ssh "${u}@${h}" "mkdir -p /var/www/ptr.shadowsofwar.io/html/play /var/www/ptr.shadowsofwar.io/html/assets"
  rsync -avz --delete --exclude='*.bin' "${ROOT}/dist/play/" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/play/" & w1=$!
  rsync -avz --delete "${ROOT}/dist/assets-publish/" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/assets/" & w6=$!
  rsync -avz "${ROOT}/dist/sow.svg" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/sow.svg" & w5=$!
  rsync -avz "${ROOT}/dist/boot.js" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/boot.js" & w8=$!
  rsync -avz "${ROOT}/dist/sw.js" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/sw.js" & w9=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar-ptr"
  rsync -avz "${sb}" "${u}@${h}:/home/bizkit/shadowsofwar-ptr/sow-server" & w2=$!
  rsync -avz "${rb}" "${u}@${h}:/home/bizkit/shadowsofwar-ptr/sow-relay" & w3=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar-ptr/assets/maps"
  rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' assets/maps/ "${u}@${h}:/home/bizkit/shadowsofwar-ptr/assets/maps/" & w4=$!
  deploy_prod_published_assets "${PROD_ASSETS_USER}" "${PROD_ASSETS_HOST}" & w7=$!
  wait "${w1}" "${w2}" "${w3}" "${w4}" "${w5}" "${w6}" "${w7}" "${w8}" "${w9}"
  verify_prod_published_assets
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/ptr.shadowsofwar.io" "${ROOT}/nginx_config_ptr.conf"
  deploy_sow_site_systemd "${u}" "${h}" "sow-site-ptr" "127.0.0.1:8788" \
    "/home/bizkit/shadowsofwar-ptr" "/home/bizkit/shadowsofwar-ptr/sow-site"
  ssh "${u}@${h}" "cat << 'SYSTEMD' | sudo tee /etc/systemd/system/sow-server-ptr.service > /dev/null
[Unit]
Description=Shadows of War Server (PTR)
After=network.target
[Service]
KillMode=process
Type=simple
User=bizkit
WorkingDirectory=/home/bizkit/shadowsofwar-ptr
ExecStart=/home/bizkit/shadowsofwar-ptr/sow-server
Restart=always
RestartSec=3
Environment="RUST_LOG=info"
Environment="SOW_WS_LISTEN=0.0.0.0:25575"
Environment="SOW_MAPS_HTTP_LISTEN=0.0.0.0:25576"
[Install]
WantedBy=multi-user.target
SYSTEMD"
  ssh -t "${u}@${h}" "sudo systemctl daemon-reload; sudo systemctl enable --now sow-server-ptr; sudo systemctl restart sow-server-ptr"
  ptr_dns_resolves && verify_prod_headers "https://ptr.shadowsofwar.io" || true
  echo "PTR deployed v${CLEAN_VERSION} -> https://ptr.shadowsofwar.io"
}

cmd_site() {
  export SOW_GAME_MANIFEST_PATH="${ROOT}/sow-site/game-manifest.json"
  echo "==> sow-site SSR on http://127.0.0.1:8787 (Ctrl+C to stop)"
  cargo run -p sow-site
}

cmd_local() {
  sow_bump_version
  killall sow-server sow-client sow-relay 2>/dev/null || true
  SERVER_PID="" CLIENT1_PID="" CLIENT2_PID="" REDIS_PID=""
  cleanup() {
    redis-cli DEL sow:ports >/dev/null 2>&1 || valkey-cli DEL sow:ports >/dev/null 2>&1 || true
    kill ${SERVER_PID:-} ${CLIENT1_PID:-} ${CLIENT2_PID:-} ${REDIS_PID:-} 2>/dev/null || true
  }
  trap cleanup EXIT INT TERM
  cargo build --features sow-core/mem_profiler -p sow-server -p sow-relay -p sow-client
  if ! redis-cli ping >/dev/null 2>&1; then
    command -v valkey-server >/dev/null && valkey-server & REDIS_PID=$! || { redis-server & REDIS_PID=$!; }
    sleep 1
  fi
  redis-cli DEL sow:ports >/dev/null 2>&1 || true
  export SOW_MAPS_ROOT="${ROOT}/assets/maps" SOW_WS_LISTEN="127.0.0.1:25565" SOW_MAPS_HTTP_LISTEN="127.0.0.1:25566" RUST_LOG=info
  cd "${ROOT}/target/debug"
  ./sow-server & SERVER_PID=$!
  sleep 1
  export SOW_WS_URL="ws://127.0.0.1:25565" SOW_MAPS_URL="http://127.0.0.1:25566/maps"
  ./sow-client & CLIENT1_PID=$!
  sleep 0.5
  ./sow-client & CLIENT2_PID=$!
  echo "Local cluster running (v${CLEAN_VERSION}). Ctrl+C to stop."
  wait
}


cmd_android() {
# ──────────────────────────────────────────────────────────────────
# Shadows of War - Dual Android Build Pipeline
# Supports:
#   1. Native GLES (Zero-overhead, ideal for low-RAM legacy hardware)
#   2. V8 WebView (High-performance, ideal for Vulkan 1.1+ devices)
# ──────────────────────────────────────────────────────────────────

# shellcheck source=web-assets.sh

green() { echo -e "\e[32m$1\e[0m"; }
cyan()  { echo -e "\e[36m$1\e[0m"; }
red()   { echo -e "\e[31m$1\e[0m"; }
yellow() { echo -e "\e[33m$1\e[0m"; }

fail() {
  red "❌ $1"
  shift
  for line in "$@"; do echo "   $line"; done
  exit 1
}

# ── Select Target ──────────────────────────────────────────────
TARGET="${1:-native}" # Default to native GLES for max compatibility

if [[ "${TARGET}" != "native" && "${TARGET}" != "webview" ]]; then
  fail "Invalid target: '${TARGET}'" \
       "Usage: ./scripts/sow.sh android [native|webview]" \
       "  native  : Build legacy GLES APK (optimized for low-RAM devices)" \
       "  webview : Build high-performance V8 WebView Vulkan APK"
fi

cyan "==> Build Target: ${TARGET^^}"

# ── SDK auto-detect ────────────────────────────────────────────
if [[ -z "${ANDROID_HOME:-}" && -n "${ANDROID_SDK_ROOT:-}" ]]; then
  export ANDROID_HOME="${ANDROID_SDK_ROOT}"
fi
if [[ -z "${ANDROID_HOME:-}" ]]; then
  for candidate in \
      "${HOME}/Android/Sdk" \
      "${HOME}/Library/Android/sdk" \
      "/opt/android-sdk" \
      "/usr/lib/android-sdk"; do
    if [[ -d "${candidate}" ]]; then
      export ANDROID_HOME="${candidate}"
      break
    fi
  done
fi
if [[ -z "${ANDROID_HOME:-}" || ! -d "${ANDROID_HOME}" ]]; then
  fail "Android SDK not found." \
       "Install Android Studio, let it provision \$HOME/Android/Sdk, then re-run."
fi
export ANDROID_SDK_ROOT="${ANDROID_HOME}"
cyan "==> Android SDK : ${ANDROID_HOME}"

# ── NDK auto-detect ────────────────────────────────────────────
if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
  if [[ -d "${ANDROID_HOME}/ndk" ]]; then
    LATEST_NDK=$(ls -1 "${ANDROID_HOME}/ndk" 2>/dev/null | sort -V | tail -n 1 || true)
    if [[ -n "${LATEST_NDK}" ]]; then
      export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/${LATEST_NDK}"
    fi
  fi
fi
if [[ -z "${ANDROID_NDK_ROOT:-}" || ! -d "${ANDROID_NDK_ROOT}" ]]; then
  if [[ -d "${ANDROID_HOME}/ndk-bundle" ]]; then
    export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk-bundle"
  fi
fi
if [[ -n "${ANDROID_NDK_ROOT:-}" ]]; then
  cyan "==> Android NDK : ${ANDROID_NDK_ROOT}"
  export NDK_HOME="${ANDROID_NDK_ROOT}"
fi

# ── JDK auto-detect ────────────────────────────────────────────
if [[ -z "${JAVA_HOME:-}" || ! -x "${JAVA_HOME}/bin/java" ]]; then
  JAVA_HOME=""
  for candidate in \
      "/opt/android-studio/jbr" \
      "/usr/lib/android-studio/jbr" \
      "${HOME}/.local/share/JetBrains/Toolbox/apps/AndroidStudio/jbr"; do
    if [[ -x "${candidate}/bin/java" ]]; then JAVA_HOME="${candidate}"; break; fi
  done
  if [[ -z "${JAVA_HOME}" ]] && command -v java >/dev/null 2>&1; then
    JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v java)")")")"
  fi
  if [[ -z "${JAVA_HOME}" ]]; then
    for candidate in /usr/lib/jvm/default /usr/lib/jvm/java-21-openjdk /usr/lib/jvm/java-17-openjdk; do
      if [[ -x "${candidate}/bin/java" ]]; then JAVA_HOME="${candidate}"; break; fi
    done
  fi
fi
if [[ -z "${JAVA_HOME}" || ! -x "${JAVA_HOME}/bin/java" ]]; then
  fail "No JDK found." \
       "Arch: sudo pacman -S jdk-openjdk" \
       "Or install Android Studio (it ships /opt/android-studio/jbr)."
fi
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${HOME}/.cargo/bin:${PATH}"
cyan "==> JDK         : ${JAVA_HOME}"

command -v cargo >/dev/null || fail "cargo not found"

# ── Execution Branch ───────────────────────────────────────────
if [[ "${TARGET}" == "native" ]]; then
  # 1. Compile Native GLES APK
  rustup target list --installed 2>/dev/null | grep -qx aarch64-linux-android || fail "run: rustup target add aarch64-linux-android"
  command -v cargo-apk >/dev/null 2>&1 || fail "cargo-apk not in PATH — install: cargo install cargo-apk"
  
  MANIFEST="${ROOT}/sow-client/Cargo.toml"
  [[ -f "$MANIFEST" ]] && grep -q '^\[package.metadata.android\]' "$MANIFEST" 2>/dev/null || fail "missing sow-client/Cargo.toml with [package.metadata.android]"
  
  # Ensure release keystore and signing config exist (local only, gitignored)
  KEYSTORE="${ROOT}/keystores/release.keystore"
  SIGNING_CFG="${ROOT}/sow-client/signing.local.toml"
  KS_PASS="${SOW_KEYSTORE_PASSWORD:?Set SOW_KEYSTORE_PASSWORD for Android release signing}"
  KEY_PASS="${SOW_KEY_PASSWORD:-$KS_PASS}"

  if [[ ! -f "${SIGNING_CFG}" ]]; then
    cat > "${SIGNING_CFG}" <<EOF
[package.metadata.android.signing.release]
path = "../keystores/release.keystore"
keystore_password = "${KS_PASS}"
key_password = "${KEY_PASS}"
key_alias = "shadows"
EOF
  fi

  if [[ ! -f "${KEYSTORE}" ]]; then
    cyan "==> Generating release keystore at ${KEYSTORE}"
    mkdir -p "$(dirname "${KEYSTORE}")"
    keytool -genkeypair -v \
      -keystore "${KEYSTORE}" \
      -alias shadows \
      -keyalg RSA -keysize 2048 -validity 10000 \
      -storepass "${KS_PASS}" -keypass "${KEY_PASS}" \
      -dname "CN=Shadows Of War, OU=Self, O=ShadowsOfWar, L=Local, S=NA, C=US" \
      >/dev/null
  fi

  cyan "📦 Building Native GLES APK..."
  RUSTFLAGS='--cfg gles' cargo apk build --release --lib -p sow-client --config "${SIGNING_CFG}"
  
  APK_SRC="${ROOT}/target/release/apk/sow-client.apk"
  APK_OUT="${ROOT}/build/sow-client-native.apk"

else
  # 2. Compile WebAssembly + WebView APK
  rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown || {
    cyan "==> Installing wasm32 Rust target..."
    rustup target add wasm32-unknown-unknown
  }

  if [[ ! -f "${ROOT}/android/gradlew" ]]; then
    cyan "==> Bootstrapping Gradle Wrapper..."
    curl -sSLo "${ROOT}/android/gradlew" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradlew
    chmod +x "${ROOT}/android/gradlew"
  fi
  if [[ ! -f "${ROOT}/android/gradle/wrapper/gradle-wrapper.jar" ]]; then
    mkdir -p "${ROOT}/android/gradle/wrapper"
    curl -sSLo "${ROOT}/android/gradle/wrapper/gradle-wrapper.jar" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradle/wrapper/gradle-wrapper.jar
  fi
  if [[ ! -f "${ROOT}/android/gradle/wrapper/gradle-wrapper.properties" ]]; then
    mkdir -p "${ROOT}/android/gradle/wrapper"
    cat << 'EOF' > "${ROOT}/android/gradle/wrapper/gradle-wrapper.properties"
distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://services.gradle.org/distributions/gradle-8.5-bin.zip
networkTimeout=10000
validateDistributionUrl=true
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
EOF
  fi

  cyan "📦 Compiling shadows-of-war for WASM..."
  RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --release -p sow-client --target wasm32-unknown-unknown

  WASM_IN="target/wasm32-unknown-unknown/release/sow_client.wasm"
  [[ -f "${WASM_IN}" ]] || fail "WASM binary not found at ${WASM_IN}"

  cyan "🔄 Packaging WASM and Web Assets..."
  ASSETS_DIR="${ROOT}/android/app/src/main/assets"
  mkdir -p "${ASSETS_DIR}"
  rm -rf "${ASSETS_DIR:?}"/*

  # Run wasm-bindgen (version matched to Cargo.lock)
  local wasm_bindgen_bin
  wasm_bindgen_bin=$(find_wasm_bindgen)
  "${wasm_bindgen_bin}" --out-dir "${ASSETS_DIR}" --target web --out-name "sow_client" --no-typescript "${WASM_IN}"

  # Compile HTML template
  CLEAN_VERSION=$(cat "${ROOT}/.version" 2>/dev/null || echo "0.1.0")
  BUILD_TS=$(date +%s)
  LOADER_TEMPLATE="${ROOT}/web/index.html.template"
  [[ -f "${LOADER_TEMPLATE}" ]] || fail "HTML template missing: ${LOADER_TEMPLATE}"

  sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
      -e "s/__JS_FILE__/sow_client.js/g" \
      -e "s/__WASM_FILE__/sow_client_bg.wasm/g" \
      -e "s/__BUILD_TS__/${BUILD_TS}/g" \
      "${LOADER_TEMPLATE}" > "${ASSETS_DIR}/index.html"

  python3 - "${ROOT}/web/loader.js" "${ASSETS_DIR}/index.html" <<'PY'
import sys
from pathlib import Path
loader = Path(sys.argv[1])
html_path = Path(sys.argv[2])
js = loader.read_text(encoding="utf-8").replace("</script>", "<\\/script>")
html = html_path.read_text(encoding="utf-8")
marker = "/* __INLINE_LOADER_JS__ */"
if marker not in html:
    raise SystemExit("index.html: no loader injection point")
html_path.write_text(html.replace(marker, js, 1), encoding="utf-8")
PY

  stage_core_assets "${ASSETS_DIR}"
  cp "${ROOT}/web/sow.svg" "${ASSETS_DIR}/sow.svg" || true

  cyan "📦 Compiling Android WebView App..."
  cd "${ROOT}/android"
  ./gradlew clean assembleDebug
  cd "${ROOT}"

  APK_SRC="${ROOT}/android/app/build/outputs/apk/debug/app-debug.apk"
  APK_OUT="${ROOT}/build/sow-client-webview.apk"
fi

# ── ADB Deployment ─────────────────────────────────────────────
if [[ -f "${APK_SRC}" ]]; then
    mkdir -p "${ROOT}/build"
    cp "${APK_SRC}" "${APK_OUT}"
    
    green "🎉 Android ${TARGET^^} build complete!"
    echo "   Generated APK : ${APK_OUT}"
    echo "   Size          : $(du -h "${APK_SRC}" | cut -f1)"
    echo ""
    if adb get-state 1>/dev/null 2>&1; then
        cyan "📱 Deploying to connected device..."
        adb push "${APK_SRC}" "/data/local/tmp/sow-client.apk" >/dev/null
        adb shell pm install -r -d "/data/local/tmp/sow-client.apk" || {
            yellow "⚠️ Normal installation failed. Attempting clean uninstall & reinstall..."
            adb uninstall rust.sow_client || true
            adb shell pm install "/data/local/tmp/sow-client.apk" || fail "Failed to install APK via ADB"
        }
        adb shell rm "/data/local/tmp/sow-client.apk"
        cyan "🚀 Launching application..."
        if [[ "${TARGET}" == "native" ]]; then
            adb shell monkey -p rust.sow_client -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
        else
            adb shell monkey -p rust.sow_client -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
        fi
        green "✅ Game started on device!"
    else
        echo -e "\033[1;33m⚠️  No ADB device detected. Skipping automatic deployment.\033[0m"
    fi
else
    fail "APK not found at ${APK_SRC}"
fi
}


main() {
  local cmd="${1:-}"
  shift || true
  case "${cmd}" in
    l|local)   cmd_local "$@" ;;
    p|ptr)     cmd_ptr "$@" ;;
    c|cloud)   cmd_cloud "$@" ;;
    package|pkg) cmd_package "$@" ;;
    site)    cmd_site "$@" ;;
    a|android)
      case "${1:-native}" in
        n|native)  shift || true; cmd_android native "$@" ;;
        w|webview) shift || true; cmd_android webview "$@" ;;
        *)         cmd_android "$@" ;;
      esac
      ;;
    ""|-h|--help|help) usage ;;
    *) usage; exit 1 ;;
  esac
}
main "$@"
