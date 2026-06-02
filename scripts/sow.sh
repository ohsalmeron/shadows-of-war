#!/usr/bin/env bash
# Shadows of War — build & deploy (single entrypoint)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
export CARGO_TARGET_DIR="${ROOT}/target"
SOW_WEB_SHELL="${ROOT}/sow-web/shell"
SOW_WEB_SITE="${ROOT}/sow-web/site"

usage() {
  cat <<EOF
Usage: ./scripts/sow.sh <command> [args]

  local|l [port]     Browser WASM shell at http://127.0.0.1:8080 (builds dist/play if needed)
  native|n           Native sow-server + 2 sow-clients (Rust binaries, fast iteration)
  crazygames [--sync-cdn]  Build dist/crazygames/ (always rebuilds; --sync-cdn updates prod CDN)
  poki               Build dist/poki/ for portal upload (+ sync prod CDN)
  ptr|p              Deploy dist/ptr → ptr.shadowsofwar.io
  cloud|c [--force]  Full prod deploy (incremental skip; --force redeploys)
  cloud-game         Deploy dist/play → play.shadowsofwar.io + backend + CDN
  cloud-site         Deploy sow-web/site → shadowsofwar.io only
  site               Marketing pages locally (:8787)
  android|a [native|n|webview|w]

Wrappers (same as above): local.sh native.sh crazygames.sh poki.sh ptr.sh cloud-game.sh cloud.sh android.sh
EOF
}

sow_load_version() {
  CLEAN_VERSION=$(cat "${ROOT}/.version" 2>/dev/null || echo "0.1.0")
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

ASSETS_STATIC="${ROOT}/assets/static"
ASSETS_STREAMED="${ROOT}/assets/streamed"

DIST_PLAY="${ROOT}/dist/play"
DIST_PTR="${ROOT}/dist/ptr"
DIST_CRAZYGAMES="${ROOT}/dist/crazygames"
DIST_POKI="${ROOT}/dist/poki"
DIST_CLOUD_STAMP="${ROOT}/dist/.sow-cloud-stamp"

stage_static_maps_and_fonts() {
  local assets_root="$1"
  mkdir -p "${assets_root}/fonts" "${assets_root}/maps/northamerica"
  rsync -a "${ASSETS_STATIC}/fonts/" "${assets_root}/fonts/"
  cp -a "${ASSETS_STATIC}/maps/northamerica/map.bin.br" \
    "${ASSETS_STATIC}/maps/northamerica/thumbnail.webp" \
    "${assets_root}/maps/northamerica/"
  cargo run -q -p sow-tools --bin write-play-catalog -- \
    --maps-root "${ASSETS_STATIC}/maps" \
    --output "${assets_root}/maps/catalog.bin" \
    northamerica
}

# Android WebView APK assets tree (not web dist — web shells use shadowsofwar.io CDN).
stage_core_assets() {
  local dest_root="$1"
  echo "==> Staging Android assets under ${dest_root}/assets/"
  mkdir -p "${dest_root}/assets"
  stage_static_maps_and_fonts "${dest_root}/assets"
  copy_static_ui_webp "${dest_root}/assets/static/ui"
}

# Symlink repo assets/static into a shell dir (no copies). Used by portal dist + local dev.
link_shell_assets_symlink() {
  local shell_dir="$1"
  rm -rf "${shell_dir}/assets"
  mkdir -p "${shell_dir}/assets"
  ln -sfn "${ASSETS_STATIC}" "${shell_dir}/assets/static"
}

link_local_repo_assets() {
  link_shell_assets_symlink "$1"
  echo "==> $1/assets/static → ${ASSETS_STATIC}"
}

portal_shell_assets_ok() {
  local dir="$1"
  [[ -L "${dir}/assets/static" ]] \
    && [[ "$(readlink -f "${dir}/assets/static")" == "$(readlink -f "${ASSETS_STATIC}")" ]]
}

cloud_deploy_stamp() {
  {
    find "${ROOT}/sow-client" "${ROOT}/sow-ui" "${ROOT}/sow-core" "${SOW_WEB_SHELL}" \
      "${ROOT}/sow-web/site" "${ROOT}/sow-server" "${ROOT}/sow-relay" \
      -type f \( -name '*.rs' -o -name '*.toml' -o -name '*.js' -o -name '*.template' -o -name '*.html' -o -name '*.css' \) \
      2>/dev/null || true
    [[ -f "${ROOT}/Cargo.lock" ]] && echo "${ROOT}/Cargo.lock"
    echo "${ROOT}/scripts/sow.sh"
  } | LC_ALL=C sort -u | while read -r f; do
    [[ -f "${f}" ]] && sha256sum "${f}"
  done | sha256sum | awk '{print $1}'
}

cloud_deploy_up_to_date() {
  [[ -f "${DIST_CLOUD_STAMP}" ]] \
    && [[ "$(cat "${DIST_CLOUD_STAMP}")" == "$(cloud_deploy_stamp)" ]] || return 1
  [[ -f "${DIST_PLAY}/index.html" ]] || return 1
  compgen -G "${DIST_PLAY}/"*_bg.wasm >/dev/null 2>&1 || return 1
  compgen -G "${DIST_PLAY}/"*_bg.wasm.br >/dev/null 2>&1 || return 1
  [[ ! -e "${DIST_PLAY}/assets" ]]
}

echo_crazygames_upload_hint() {
  echo "Upload: every file inside ${DIST_CRAZYGAMES}/ (index.html at upload root)."
  echo "If the portal rejects symlinks: rsync -aL ${DIST_CRAZYGAMES}/ /tmp/cg-upload/ and upload that."
}

# Resized loader/splash webp for local shell (./assets/static/ui/) or CDN publish tree.
copy_static_ui_webp() {
  local dest="$1"
  local ui_src="${ASSETS_STATIC}/ui"
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

verify_dist_layout() {
  local dir="$1" profile="$2"
  case "${profile}" in
    crazygames|poki)
      compgen -G "${dir}/"*_bg.wasm.br >/dev/null \
        || { echo "❌ ${dir}: missing *_bg.wasm.br"; return 1; }
      if compgen -G "${dir}/"*_bg.wasm >/dev/null 2>&1; then
        echo "❌ ${dir}: raw .wasm must not be in portal dist"
        return 1
      fi
      portal_shell_assets_ok "${dir}" \
        || { echo "❌ ${dir}: missing assets/static symlink to ${ASSETS_STATIC}"; return 1; }
      ;;
    selfhosted)
      compgen -G "${dir}/"*_bg.wasm >/dev/null \
        || { echo "❌ ${dir}: missing raw *_bg.wasm"; return 1; }
      compgen -G "${dir}/"*_bg.wasm.br >/dev/null \
        || { echo "❌ ${dir}: missing *_bg.wasm.br"; return 1; }
      if [[ -e "${dir}/assets" ]]; then
        echo "❌ ${dir}: shell must not bundle assets/ (CDN: shadowsofwar.io/assets)"
        return 1
      fi
      ;;
    *)
      echo "❌ verify_dist_layout: unknown profile ${profile}"
      return 1
      ;;
  esac
  echo "✅ Dist layout OK (${dir}, ${profile})"
}

# profile: selfhosted | crazygames | poki — out_dir: dist/play, dist/ptr, dist/crazygames, etc.
sow_assemble_game_shell() {
  local profile="${1:?profile required}"
  local out_dir="${2:?out_dir required}"
  local portal=""
  case "${profile}" in
    selfhosted) portal="" ;;
    crazygames|poki) portal="${profile}" ;;
    *)
      echo "❌ unknown shell profile: ${profile}"
      return 1
      ;;
  esac
  echo "==> Packaging game shell (${out_dir}/)..."
  mkdir -p "${out_dir}" && rm -rf "${out_dir:?}"/*
  sow_load_version
  BUILD_TS=$(date +%s)
  JS_FILE="sow_client_${BUILD_TS}.js"
  WASM_FILE="sow_client_${BUILD_TS}_bg.wasm"
  local wasm_bindgen_bin
  wasm_bindgen_bin=$(find_wasm_bindgen)
  "${wasm_bindgen_bin}" --out-dir "${out_dir}" --target web --out-name "sow_client_${BUILD_TS}" --no-typescript "${WASM_IN}"
  cp -a "${SOW_WEB_SHELL}/favicon_io/"* "${out_dir}/" 2>/dev/null || true
  cp "${SOW_WEB_SHELL}/sow.svg" "${out_dir}/sow.svg"
  mkdir -p "${out_dir}/sdk" && cp -a "${SOW_WEB_SHELL}/sdk/." "${out_dir}/sdk/"
  build_index_html "${SOW_WEB_SHELL}/index.html.template" "${out_dir}/index.html" \
    "${CLEAN_VERSION}" "${JS_FILE}" "${WASM_FILE}" "${BUILD_TS}" "${portal}"
  [[ "${portal}" == "crazygames" ]] && inject_crazygames_portal "${out_dir}/index.html"
  [[ "${portal}" == "poki" ]] && inject_poki_portal "${out_dir}/index.html"
  minify_js_shim "${out_dir}/${JS_FILE}"
  optimize_wasm_bundle "${out_dir}/${WASM_FILE}"
  local wasm_load="${WASM_FILE}" js_load="${JS_FILE}"
  if command -v brotli >/dev/null 2>&1; then
    brotli -f -Z "${out_dir}/${WASM_FILE}" & p1=$!
    brotli -f -Z "${out_dir}/${JS_FILE}" & p2=$!
    wait "${p1}" "${p2}"
    local wasm_br_kb js_br_kb
    wasm_br_kb=$(( $(stat -c%s "${out_dir}/${WASM_FILE}.br") / 1024 ))
    js_br_kb=$(( $(stat -c%s "${out_dir}/${JS_FILE}.br") / 1024 ))
    echo "Brotli: ${WASM_FILE}.br=${wasm_br_kb} KB, ${JS_FILE}.br=${js_br_kb} KB"
    case "${portal}" in
      crazygames|poki)
        echo "==> Portal: .br only (~${wasm_br_kb} KB wasm); assets/static symlinked from repo"
        rm -f "${out_dir}/${WASM_FILE}" "${out_dir}/${JS_FILE}"
        wasm_load="${WASM_FILE}.br"
        js_load="${JS_FILE}.br"
        sed -i \
          -e "s|./${JS_FILE}|./${js_load}|g" -e "s|${JS_FILE}|${js_load}|g" \
          -e "s|./${WASM_FILE}|./${wasm_load}|g" -e "s|${WASM_FILE}|${wasm_load}|g" \
          "${out_dir}/index.html"
        ;;
      *)
        echo "Self-hosted: raw + .br; nginx brotli_static serves .br at .wasm/.js when Accept-Encoding: br"
        ;;
    esac
  else
    echo "⚠️  brotli not found — shipping uncompressed WASM/JS only"
  fi
  case "${profile}" in
    selfhosted)
      echo "==> Shell-only dist; static art/maps/fonts from CDN (SOW_ASSETS_URL / SOW_MAPS_URL)"
      sed -e "s/__VERSION__/${CLEAN_VERSION}/g" -e "s/__JS_FILE__/${js_load}/g" \
        -e "s/__WASM_FILE__/${wasm_load}/g" -e "s/__BUILD_TS__/${BUILD_TS}/g" \
        "${SOW_WEB_SHELL}/sw.js.template" > "${out_dir}/sw.js"
      write_game_manifest "${out_dir}" "${js_load}" "${wasm_load}" "${BUILD_TS}"
      ;;
    crazygames|poki)
      link_shell_assets_symlink "${out_dir}"
      ;;
  esac
  prune_querystring_artifacts "${out_dir}"
  echo "Load paths: ${wasm_load}, ${js_load}"
}

write_game_manifest() {
  local play="$1" js="$2" wasm="$3" ts="$4"
  cat > "${play}/game-manifest.json" <<EOF
{"js":"${js}","wasm":"${wasm}","build_ts":"${ts}","version":"${CLEAN_VERSION}"}
EOF
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

# /var/www/* is root-owned on the VPS; deploy user needs sudo + chown before rsync.
ensure_vps_dirs() {
    local u="$1" h="$2"
    shift 2
    local d quoted=""
    for d in "$@"; do
        quoted+="$(printf ' %q' "${d}")"
    done
    ssh "${u}@${h}" "sudo mkdir -p${quoted} && sudo chown -R ${u}:${u}${quoted}"
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

play_host_url() {
  local base_url="${1%/}"
  if [[ "${base_url}" == "https://ptr.shadowsofwar.io" ]]; then
    echo "${base_url}/"
  else
    echo "https://play.shadowsofwar.io/"
  fi
}

verify_play_tls_cert() {
  local host="${1:-play.shadowsofwar.io}"
  echo "==> Verifying TLS certificate for ${host}..."
  local subject
  subject=$(echo | openssl s_client -connect "${host}:443" -servername "${host}" 2>/dev/null \
    | openssl x509 -noout -subject 2>/dev/null) || true
  if [[ -z "${subject}" ]]; then
    echo "❌ Could not read TLS certificate for ${host}"
    return 1
  fi
  if [[ "${subject}" != *"${host}"* ]]; then
    echo "❌ Wrong TLS certificate for ${host}: ${subject}"
    echo "   Run ./scripts/sow.sh cloud-game (cloud-site does not issue the play subdomain cert)."
    return 1
  fi
  echo "✅ TLS certificate OK (${subject})"
}

verify_play_host() {
  local play_url="$1"
  local html wasm js
  if [[ "${play_url}" == *"play.shadowsofwar.io"* ]]; then
    verify_play_tls_cert "play.shadowsofwar.io" || return 1
  fi
  echo "==> Verifying game shell at ${play_url}..."
  curl -fsS "${play_url}game-manifest.json" | grep -q 'sow_client_' \
    || { echo "❌ game-manifest.json missing or invalid on play host"; return 1; }
  html=$(curl -fsS "${play_url}")
  echo "${html}" | grep -q 'id="web-loader"' \
    || { echo "❌ play index.html missing #web-loader"; return 1; }
  echo "${html}" | grep -q 'sow_client_[0-9]' \
    || { echo "❌ play index.html missing sow_client bundle reference"; return 1; }
  if echo "${html}" | grep -q 'boot.js'; then
    echo "❌ play host must not ship boot.js embed"
    return 1
  fi
  wasm=$(curl -fsS "${play_url}game-manifest.json" | grep -oE 'sow_client_[0-9]+_bg\.wasm' | head -1) \
    || wasm=$(echo "${html}" | grep -oE 'sow_client_[0-9]+_bg\.wasm' | head -1) \
    || return 1
  js=$(curl -fsS "${play_url}game-manifest.json" | grep -oE 'sow_client_[0-9]+\.js' | head -1) \
    || js=$(echo "${html}" | grep -oE 'sow_client_[0-9]+\.js' | head -1) \
    || return 1
  curl -fsSI -H 'Accept-Encoding: br' "${play_url}${wasm}" | grep -qi 'content-encoding: br' \
    || { echo "❌ WASM not served with brotli on play host"; return 1; }
  curl -fsSI "https://shadowsofwar.io/assets/static/ui/loader_empty.webp" | grep -qi 'cache-control:.*max-age' \
    || { echo "❌ CDN loader webp missing cache header (run cloud-game or crazygames to sync assets)"; return 1; }
  echo "✅ Play host OK (${wasm}, ${js})"
}

verify_marketing_landing_only() {
  local marketing_url="${1%/}"
  local html
  echo "==> Verifying marketing host is landing-only (no game shell)..."
  html=$(curl -fsS "${marketing_url}/")
  if echo "${html}" | grep -qE 'game-stage|boot\.js|sow-game-manifest|sow_client_[0-9]'; then
    echo "❌ marketing page still ships game shell — run ./scripts/sow.sh cloud-site"
    return 1
  fi
  echo "${html}" | grep -q 'play.shadowsofwar.io' \
    || { echo "❌ marketing page missing play subdomain link"; return 1; }
  curl -fsS "${marketing_url}/health" | grep -qi '^ok$' \
    || { echo "❌ marketing /health failed"; return 1; }
  echo "✅ Marketing host is landing-only"
}

verify_prod_headers() {
  local base_url="${1%/}"
  local play_url
  play_url=$(play_host_url "${base_url}")
  verify_play_host "${play_url}" || return 1
  if [[ "${base_url}" == "https://shadowsofwar.io" ]]; then
    verify_marketing_landing_only "${base_url}" || return 1
  fi
}

play_tls_cert_present() {
  local u="$1" h="$2"
  ssh "${u}@${h}" "sudo test -f /etc/letsencrypt/live/play.shadowsofwar.io/fullchain.pem"
}

ensure_play_tls_cert() {
  local u="$1" h="$2"
  if play_tls_cert_present "${u}" "${h}"; then
    echo "✅ TLS: play.shadowsofwar.io certificate present"
    return 0
  fi
  echo "==> Issuing TLS certificate for play.shadowsofwar.io (one-time certbot)..."
  ssh "${u}@${h}" "sudo certbot certonly --webroot \
    -w /var/www/play.shadowsofwar.io/html \
    -d play.shadowsofwar.io \
    --non-interactive --agree-tos --register-unsafely-without-email \
    || sudo certbot certonly --nginx -d play.shadowsofwar.io --non-interactive --agree-tos --register-unsafely-without-email"
  play_tls_cert_present "${u}" "${h}" \
    || { echo "❌ certbot failed for play.shadowsofwar.io — see README Web hosts"; return 1; }
  echo "✅ TLS: play.shadowsofwar.io certificate installed"
}

cleanup_legacy_marketing_static() {
  local u="$1" h="$2"
  echo "==> Removing legacy marketing static game files (WASM lives on play subdomain)..."
  ssh "${u}@${h}" "rm -f /var/www/shadowsofwar.io/html/boot.js \
    /var/www/shadowsofwar.io/html/sw.js \
    /var/www/shadowsofwar.io/html/game-manifest.json \
    && rm -rf /var/www/shadowsofwar.io/html/play"
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
    local leaders_src="${ROOT}/assets/streamed/leaders"
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

verify_cdn_url() {
    local url="$1"
    curl -fsSI --connect-timeout 10 --max-time 30 "${url}" >/dev/null 2>&1
}

# Pass if canonical OR legacy path returns 200 (bridges pre/post static-streamed migration).
verify_cdn_asset_pair() {
    local base="https://shadowsofwar.io/assets"
    local canonical="$1"
    local legacy="${2:-}"
    if verify_cdn_url "${base}/${canonical}"; then
        return 0
    fi
    if [[ -n "${legacy}" ]] && verify_cdn_url "${base}/${legacy}"; then
        return 0
    fi
    echo "❌ CDN asset missing: ${base}/${canonical}${legacy:+, ${base}/${legacy}}"
    return 1
}

verify_prod_published_assets() {
    echo "==> Verifying prod CDN assets..."
    verify_cdn_asset_pair "streamed/leaders/caesar_desktop.webp" "ui/leaders/caesar_desktop.webp" || return 1
    verify_cdn_asset_pair "streamed/leaders/richard_the_lionheart_desktop.webp" "ui/leaders/richard_the_lionheart_desktop.webp" || return 1
    verify_cdn_asset_pair "static/ui/loader_empty.webp" "ui/loader_empty.webp" || return 1
    if ! verify_cdn_url "https://shadowsofwar.io/assets/fonts/JockeyOne-Regular.ttf"; then
        echo "❌ CDN asset missing: fonts/JockeyOne-Regular.ttf"
        return 1
    fi
    echo "✅ prod CDN assets OK"
}

# Rsync only assets/streamed/ to prod (leader portraits). No game shell, portal SDK, or --delete on /assets/.
sow_sync_streamed_cdn() {
    local u="${1:-${PROD_ASSETS_USER}}" h="${2:-${PROD_ASSETS_HOST}}"
    echo "==> Syncing streamed assets (${u}@${h}:${PROD_ASSETS_PATH}/streamed/)"
    check_leader_portraits_complete
    ssh "${u}@${h}" "mkdir -p ${PROD_ASSETS_PATH}/streamed/leaders"
    rsync -avz "${ASSETS_STREAMED}/" "${u}@${h}:${PROD_ASSETS_PATH}/streamed/"
    sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/shadowsofwar.io" \
        "${ROOT}/deploy/nginx/shadowsofwar.io.conf"
}

# Boot loader/splash webp only (cloud-game / cloud). Not used for portal zip builds.
sow_sync_static_ui_cdn() {
    local u="${1:-${PROD_ASSETS_USER}}" h="${2:-${PROD_ASSETS_HOST}}"
    echo "==> Syncing static boot UI (${u}@${h}:${PROD_ASSETS_PATH}/static/ui/)"
    check_local_build_tools
    local ui_stage
    ui_stage=$(mktemp -d)
    copy_static_ui_webp "${ui_stage}"
    ssh "${u}@${h}" "mkdir -p ${PROD_ASSETS_PATH}/static/ui"
    rsync -avz "${ui_stage}/" "${u}@${h}:${PROD_ASSETS_PATH}/static/ui/"
    rm -rf "${ui_stage}"
}

# Inline sow-web/shell/loader.js into the loader marker of a built dist/index.html.
inline_loader_into_index() {
    local html_path="$1"
    python3 - "${SOW_WEB_SHELL}/loader.js" "${html_path}" <<'PY'
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

# Render sow-web/shell/index.html.template -> dist/index.html with build tokens, then inline loader.js.
# Portal SDK / boot vars are NOT templated here (one HTML for website + PTR + CrazyGames base).
# CrazyGames-only injection is handled separately by inject_crazygames_portal.
build_index_html() {
    local template="$1" out="$2" version="$3" js_file="$4" wasm_file="$5" build_ts="$6"
    # $7 portal profile unused here — static UI always on prod CDN (shells do not ship assets/).
    local assets_ui_base="https://shadowsofwar.io/assets/static/ui/"
    sed -e "s/__VERSION__/${version}/g" \
        -e "s/__JS_FILE__/${js_file}/g" \
        -e "s/__WASM_FILE__/${wasm_file}/g" \
        -e "s/__BUILD_TS__/${build_ts}/g" \
        -e "s|__ASSETS_UI_BASE__|${assets_ui_base}|g" \
        "${template}" > "${out}"
    inline_loader_into_index "${out}"
}

# CrazyGames package build only: replace the PORTAL_SDK_SLOT / PORTAL_BOOT_SLOT marker
# lines in dist/index.html with the real SDK <script> tag and portal boot vars.
inject_crazygames_portal() {
    local html_path="$1"
    local sdk_tag='    <script src="https://sdk.crazygames.com/crazygames-sdk-v3.js"></script>'
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

# Poki package build: set portal boot vars only (Poki injects PokiSDK from the host page).
inject_poki_portal() {
    local html_path="$1"
    local boot_js='        window.SOW_PORTAL = "poki"; window.SOW_WS_URL = "wss://shadowsofwar.io/ws/"; window.SOW_MAPS_URL = "https://shadowsofwar.io/maps"; window.SOW_ASSETS_URL = "https://shadowsofwar.io/assets";'
    python3 - "${html_path}" "${boot_js}" <<'PY'
import sys
from pathlib import Path

html_path = Path(sys.argv[1])
boot_js = sys.argv[2]
lines = html_path.read_text(encoding="utf-8").splitlines()
out = []
replaced_boot = False
for line in lines:
    if "PORTAL_BOOT_SLOT" in line:
        out.append(boot_js)
        replaced_boot = True
    elif "PORTAL_SDK_SLOT" in line:
        continue
    else:
        out.append(line)
if not replaced_boot:
    raise SystemExit("inject_poki_portal: missing PORTAL_BOOT_SLOT")
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

# dist/play + dist/assets — no SSH, no version bump (cloud-game bumps before deploy).
build_play_dist() {
  check_local_build_tools
  rm -rf "${ROOT}/dist/assets"
  sow_compile_wasm_release
  sow_assemble_game_shell selfhosted "${DIST_PLAY}"
  verify_dist_layout "${DIST_PLAY}" selfhosted
  echo "Play dist ready: ${DIST_PLAY}/ ($(du -sh "${DIST_PLAY}" | cut -f1), shell only — assets on CDN)"
}

_portal_sync_cdn_prereq() {
  check_vps_ready "${PROD_ASSETS_USER}" "${PROD_ASSETS_HOST}" \
    "/etc/nginx/sites-available/shadowsofwar.io"
  sow_sync_streamed_cdn
  verify_prod_published_assets
}

_portal_build_prereq() {
  check_local_build_tools
  sow_bump_version
  _portal_sync_cdn_prereq
}

cmd_crazygames() {
  local sync_cdn=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --sync-cdn) sync_cdn=1; shift ;;
      -h|--help)
        echo "Usage: ./scripts/sow.sh crazygames [--sync-cdn]"
        echo "  --sync-cdn  Push streamed leaders to prod CDN before building (optional)"
        exit 0
        ;;
      *)
        echo "❌ Unknown option: $1"
        exit 1
        ;;
    esac
  done
  check_local_build_tools
  if [[ "${sync_cdn}" -eq 1 ]]; then
    _portal_sync_cdn_prereq
  fi
  sow_bump_version
  sow_compile_wasm_release
  sow_assemble_game_shell crazygames "${DIST_CRAZYGAMES}"
  verify_dist_layout "${DIST_CRAZYGAMES}" crazygames
  echo "CrazyGames ready: ${DIST_CRAZYGAMES}/ ($(du -sh "${DIST_CRAZYGAMES}" | cut -f1))"
  if compgen -G "${DIST_CRAZYGAMES}/"*_bg.wasm.br >/dev/null; then
    echo "WASM brotli: $(du -ch "${DIST_CRAZYGAMES}/"*_bg.wasm.br 2>/dev/null | tail -1 | cut -f1)"
  fi
  echo "==> ${DIST_CRAZYGAMES}/assets/static → ${ASSETS_STATIC}"
  echo_crazygames_upload_hint
  print_agpl_release_steps
}

cmd_poki() {
  _portal_build_prereq
  sow_compile_wasm_release
  sow_assemble_game_shell poki "${DIST_POKI}"
  verify_dist_layout "${DIST_POKI}" poki
  echo "Poki ready: ${DIST_POKI}/ ($(du -sh "${DIST_POKI}" | cut -f1))"
  print_agpl_release_steps
}

cloud_build_server_binaries() {
  local sb rb
  if cargo build --release -p sow-server --target x86_64-unknown-linux-musl \
    && cargo build --release -p sow-relay --target x86_64-unknown-linux-musl; then
    sb=target/x86_64-unknown-linux-musl/release/sow-server
    rb=target/x86_64-unknown-linux-musl/release/sow-relay
  else
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    cargo build --release -p sow-relay --target x86_64-unknown-linux-gnu
    sb=target/x86_64-unknown-linux-gnu/release/sow-server
    rb=target/x86_64-unknown-linux-gnu/release/sow-relay
  fi
  echo "${sb}" "${rb}"
}

_deploy_cloud_game() {
  local u=bizkit h=35.239.160.167
  build_play_dist
  read -r sb rb <<< "$(cloud_build_server_binaries)"
  sow_sync_streamed_cdn "${u}" "${h}"
  ensure_vps_dirs "${u}" "${h}" \
    /var/www/play.shadowsofwar.io/html \
    /home/bizkit/shadowsofwar/assets/maps
  rsync -avzL --delete --exclude='*.bin' "${DIST_PLAY}/" "${u}@${h}:/var/www/play.shadowsofwar.io/html/" & w1=$!
  sow_sync_static_ui_cdn "${u}" "${h}" & w6=$!
  rsync -avz "${sb}" "${u}@${h}:/home/bizkit/shadowsofwar/sow-server" & w2=$!
  rsync -avz "${rb}" "${u}@${h}:/home/bizkit/shadowsofwar/sow-relay" & w3=$!
  rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' \
    assets/static/maps/ "${u}@${h}:/home/bizkit/shadowsofwar/assets/maps/" & w4=$!
  wait "${w1}" "${w2}" "${w3}" "${w4}" "${w6}"
  verify_prod_published_assets
  local play_nginx="${ROOT}/deploy/nginx/play.conf"
  if ! play_tls_cert_present "${u}" "${h}"; then
    play_nginx="${ROOT}/deploy/nginx/play-bootstrap.conf"
  fi
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/play.shadowsofwar.io" "${play_nginx}"
  ensure_play_tls_cert "${u}" "${h}"
  play_tls_cert_present "${u}" "${h}" \
    || { echo "❌ play.shadowsofwar.io TLS cert missing — fix certbot/DNS before enabling play.conf"; return 1; }
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/play.shadowsofwar.io" "${ROOT}/deploy/nginx/play.conf"
  verify_play_tls_cert "play.shadowsofwar.io" || return 1
  ssh -t "${u}@${h}" "sudo systemctl enable --now sow-redis 2>/dev/null; sudo systemctl restart sow-server"
  verify_play_host "$(play_host_url "https://shadowsofwar.io")"
  echo "Game deployed v${CLEAN_VERSION} -> https://play.shadowsofwar.io/"
}

cmd_cloud_game() {
  check_local_build_tools
  check_vps_ready "bizkit" "35.239.160.167" "/etc/nginx/sites-available/shadowsofwar.io"
  sow_bump_version
  _deploy_cloud_game
  print_agpl_release_steps
}

_deploy_cloud_site() {
  local u=bizkit h=35.239.160.167
  cleanup_legacy_marketing_static "${u}" "${h}"
  ssh "${u}@${h}" "sudo systemctl disable --now sow-site 2>/dev/null || true"
  ensure_vps_dirs "${u}" "${h}" /var/www/shadowsofwar.io/html "${PROD_ASSETS_PATH}"
  # Never --delete under html/assets/ — CDN (streamed leaders, static ui, fonts) lives there.
  rsync -avz --delete --exclude 'assets/' "${SOW_WEB_SITE}/" "${u}@${h}:/var/www/shadowsofwar.io/html/"
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/shadowsofwar.io" "${ROOT}/deploy/nginx/shadowsofwar.io.conf"
  sow_sync_streamed_cdn "${PROD_ASSETS_USER}" "${PROD_ASSETS_HOST}"
  sow_sync_static_ui_cdn "${PROD_ASSETS_USER}" "${PROD_ASSETS_HOST}"
  ssh "${u}@${h}" "mkdir -p ${PROD_ASSETS_PATH}/fonts"
  rsync -avz "${ASSETS_STATIC}/fonts/" "${u}@${h}:${PROD_ASSETS_PATH}/fonts/"
  verify_prod_published_assets
  verify_marketing_landing_only "https://shadowsofwar.io"
  echo "Site deployed v${CLEAN_VERSION} -> https://shadowsofwar.io/"
}

cmd_cloud_site() {
  check_vps_ready "bizkit" "35.239.160.167" "/etc/nginx/sites-available/shadowsofwar.io"
  sow_bump_version
  _deploy_cloud_site
}

cmd_cloud() {
  local force=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --force) force=1; shift ;;
      -h|--help)
        echo "Usage: ./scripts/sow.sh cloud [--force]"
        echo "  Full prod: play host + marketing + backend + CDN"
        echo "  Skips rebuild/rsync when inputs unchanged (only incremental command)."
        echo "  --force   Redeploy even if stamp matches"
        exit 0
        ;;
      *)
        echo "❌ Unknown option: $1"
        exit 1
        ;;
    esac
  done
  check_local_build_tools
  check_vps_ready "bizkit" "35.239.160.167" "/etc/nginx/sites-available/shadowsofwar.io"
  if [[ "${force}" -eq 0 ]] && cloud_deploy_up_to_date; then
    sow_load_version
    verify_prod_headers "https://shadowsofwar.io" || return 1
    echo "✅ Cloud deploy up to date — no rebuild/rsync (use --force to redeploy)"
    return 0
  fi
  sow_bump_version
  _deploy_cloud_game
  _deploy_cloud_site
  mkdir -p "${ROOT}/dist"
  cloud_deploy_stamp > "${DIST_CLOUD_STAMP}"
  verify_prod_headers "https://shadowsofwar.io"
  echo "Full production deploy v${CLEAN_VERSION} complete."
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
  sow_assemble_game_shell selfhosted "${DIST_PTR}"
  verify_dist_layout "${DIST_PTR}" selfhosted
  local u=bizkit h=shadowsofwar.io
  sow_sync_streamed_cdn "${PROD_ASSETS_USER}" "${PROD_ASSETS_HOST}"
  ensure_vps_dirs "${u}" "${h}" /var/www/ptr.shadowsofwar.io/html
  rsync -avzL --delete --exclude='*.bin' "${DIST_PTR}/" "${u}@${h}:/var/www/ptr.shadowsofwar.io/html/" & w1=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar-ptr"
  rsync -avz "${sb}" "${u}@${h}:/home/bizkit/shadowsofwar-ptr/sow-server" & w2=$!
  rsync -avz "${rb}" "${u}@${h}:/home/bizkit/shadowsofwar-ptr/sow-relay" & w3=$!
  ssh "${u}@${h}" "mkdir -p /home/bizkit/shadowsofwar-ptr/assets/maps"
  rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' assets/static/maps/ "${u}@${h}:/home/bizkit/shadowsofwar-ptr/assets/maps/" & w4=$!
  wait "${w1}" "${w2}" "${w3}" "${w4}"
  sync_vps_nginx "${u}" "${h}" "/etc/nginx/sites-available/ptr.shadowsofwar.io" "${ROOT}/deploy/nginx/ptr.conf"
  ssh "${u}@${h}" "systemctl is-active --quiet sow-site-ptr 2>/dev/null && sudo systemctl disable --now sow-site-ptr || true"
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
  ptr_dns_resolves && verify_prod_headers "https://ptr.shadowsofwar.io"
  echo "PTR deployed v${CLEAN_VERSION} -> https://ptr.shadowsofwar.io"
}

cmd_site() {
  echo "==> Marketing site (sow-web/site) on http://127.0.0.1:8787 — refresh browser after edits"
  echo "    Game shell: ./scripts/sow.sh local  ->  http://127.0.0.1:8080/"
  cd "${SOW_WEB_SITE}"
  python3 -m http.server 8787
}

# Browser WASM shell (dist/play). Prod deploy is cloud-game, not this.
cmd_local() {
  local port="${1:-8080}"
  if [[ ! -f "${DIST_PLAY}/index.html" ]]; then
    echo "==> Building ${DIST_PLAY}/ for local browser..."
    build_play_dist
  fi
  link_local_repo_assets "${DIST_PLAY}"
  echo "==> Local browser shell http://127.0.0.1:${port}/ (Ctrl+C to stop)"
  echo "    Boot art: CDN + optional ${DIST_PLAY}/assets/static → repo (symlink)"
  cd "${DIST_PLAY}"
  if command -v python3 >/dev/null 2>&1; then
    python3 -m http.server "${port}" --bind 127.0.0.1
  else
    python -m http.server "${port}" --bind 127.0.0.1
  fi
}

cmd_native() {
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
  export SOW_MAPS_ROOT="${ROOT}/assets/static/maps" SOW_WS_LISTEN="127.0.0.1:25565" SOW_MAPS_HTTP_LISTEN="127.0.0.1:25566" RUST_LOG=info
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
  KEYSTORE="${ROOT}/deploy/keystores/release.keystore"
  SIGNING_CFG="${ROOT}/sow-client/signing.local.toml"
  KS_PASS="${SOW_KEYSTORE_PASSWORD:?Set SOW_KEYSTORE_PASSWORD for Android release signing}"
  KEY_PASS="${SOW_KEY_PASSWORD:-$KS_PASS}"

  if [[ ! -f "${SIGNING_CFG}" ]]; then
    cat > "${SIGNING_CFG}" <<EOF
[package.metadata.android.signing.release]
path = "../deploy/keystores/release.keystore"
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

  if [[ ! -f "${ROOT}/deploy/android/gradlew" ]]; then
    cyan "==> Bootstrapping Gradle Wrapper..."
    curl -sSLo "${ROOT}/deploy/android/gradlew" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradlew
    chmod +x "${ROOT}/deploy/android/gradlew"
  fi
  if [[ ! -f "${ROOT}/deploy/android/gradle/wrapper/gradle-wrapper.jar" ]]; then
    mkdir -p "${ROOT}/deploy/android/gradle/wrapper"
    curl -sSLo "${ROOT}/deploy/android/gradle/wrapper/gradle-wrapper.jar" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradle/wrapper/gradle-wrapper.jar
  fi
  if [[ ! -f "${ROOT}/deploy/android/gradle/wrapper/gradle-wrapper.properties" ]]; then
    mkdir -p "${ROOT}/deploy/android/gradle/wrapper"
    cat << 'EOF' > "${ROOT}/deploy/android/gradle/wrapper/gradle-wrapper.properties"
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
  ASSETS_DIR="${ROOT}/deploy/android/app/src/main/assets"
  mkdir -p "${ASSETS_DIR}"
  rm -rf "${ASSETS_DIR:?}"/*

  # Run wasm-bindgen (version matched to Cargo.lock)
  local wasm_bindgen_bin
  wasm_bindgen_bin=$(find_wasm_bindgen)
  "${wasm_bindgen_bin}" --out-dir "${ASSETS_DIR}" --target web --out-name "sow_client" --no-typescript "${WASM_IN}"

  # Compile HTML template
  CLEAN_VERSION=$(cat "${ROOT}/.version" 2>/dev/null || echo "0.1.0")
  BUILD_TS=$(date +%s)
  LOADER_TEMPLATE="${SOW_WEB_SHELL}/index.html.template"
  [[ -f "${LOADER_TEMPLATE}" ]] || fail "HTML template missing: ${LOADER_TEMPLATE}"

  sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
      -e "s/__JS_FILE__/sow_client.js/g" \
      -e "s/__WASM_FILE__/sow_client_bg.wasm/g" \
      -e "s/__BUILD_TS__/${BUILD_TS}/g" \
      "${LOADER_TEMPLATE}" > "${ASSETS_DIR}/index.html"

  python3 - "${SOW_WEB_SHELL}/loader.js" "${ASSETS_DIR}/index.html" <<'PY'
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
  cp "${SOW_WEB_SHELL}/sow.svg" "${ASSETS_DIR}/sow.svg" || true

  cyan "📦 Compiling Android WebView App..."
  cd "${ROOT}/deploy/android"
  ./gradlew clean assembleDebug
  cd "${ROOT}"

  APK_SRC="${ROOT}/deploy/android/app/build/outputs/apk/debug/app-debug.apk"
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
    l|local)        cmd_local "$@" ;;
    n|native)       cmd_native "$@" ;;
    crazygames)     cmd_crazygames "$@" ;;
    poki)           cmd_poki "$@" ;;
    p|ptr)          cmd_ptr "$@" ;;
    c|cloud)        cmd_cloud "$@" ;;
    cloud-game)     cmd_cloud_game "$@" ;;
    cloud-site)     cmd_cloud_site "$@" ;;
    site)           cmd_site "$@" ;;
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
