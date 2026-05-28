# Shared web loader asset helpers (sourced by cloud.sh, ptr.sh, android.sh).

_SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy-env.sh
source "${_SCRIPTS_DIR}/deploy-env.sh"

copy_leader_portraits() {
    local dest="${1:-dist/assets/ui/leaders}"
    local leaders_src="${ROOT}/sow-ui/assets/ui/leaders"
    if [[ ! -d "${leaders_src}" ]]; then
        echo "❌ Missing leader portraits: ${leaders_src}"
        exit 1
    fi
    mkdir -p "${dest}"
    cp -a "${leaders_src}/." "${dest}/"
}

copy_web_loader_assets() {
    local dest="${1:-dist/assets/ui}"
    local ui_src="${ROOT}/sow-ui/assets/ui"
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
