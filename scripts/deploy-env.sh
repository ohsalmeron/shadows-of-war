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
    local cwebp_bin terser_cmd

    if ! cwebp_bin=$(find_cwebp); then
        install_cwebp_if_missing || missing+=("cwebp (Arch: libwebp-utils, Debian: webp)")
        cwebp_bin=$(find_cwebp) || true
    fi

    command -v brotli >/dev/null 2>&1 || missing+=("brotli")
    command -v wasm-bindgen >/dev/null 2>&1 || [[ -x "${HOME}/.cargo/bin/wasm-bindgen" ]] \
        || missing+=("wasm-bindgen (cargo install wasm-bindgen-cli)")
    find_terser >/dev/null || missing+=("terser or npx")

    if ((${#missing[@]})); then
        echo "❌ Missing local build tools:"
        printf '   - %s\n' "${missing[@]}"
        exit 1
    fi

    terser_cmd=$(find_terser)
    wasm_opt_hint=""
    find_wasm_opt >/dev/null || wasm_opt_hint=" (wasm-opt/binaryen optional, recommended)"
    echo "✅ Build tools: cwebp=${cwebp_bin} brotli=$(command -v brotli) terser=${terser_cmd}${wasm_opt_hint}"
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
    local wasm js
    wasm=$(curl -fsS "${base_url}/" | grep -oE 'sow_client_[0-9]+_bg\.wasm' | head -1) || return 1
    js=$(curl -fsS "${base_url}/" | grep -oE 'sow_client_[0-9]+\.js' | head -1) || return 1

    echo "==> Verifying live headers..."
    curl -fsSI -H 'Accept-Encoding: br' "${base_url}/${wasm}" | grep -qi 'content-encoding: br' \
        || { echo "❌ WASM not served with brotli"; return 1; }
    curl -fsSI "${base_url}/index.html" | grep -qi 'cache-control:.*must-revalidate' \
        || { echo "❌ index.html missing no-cache header"; return 1; }
    curl -fsSI "${base_url}/assets/ui/loader_empty.webp" | grep -qi 'cache-control:.*max-age' \
        || { echo "❌ loader webp missing cache header"; return 1; }
    echo "✅ Live: brotli WASM, cache headers OK (${wasm}, ${js})"
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
