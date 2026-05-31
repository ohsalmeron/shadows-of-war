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

# Inline web/loader.js into the loader marker of a built dist/index.html.
inline_loader_into_index() {
    local html_path="$1"
    local root="${ROOT:-$(cd "${_SCRIPTS_DIR}/.." && pwd)}"
    python3 - "${root}/web/loader.js" "${html_path}" <<'PY'
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
    local boot_js='        window.SOW_PORTAL = "crazygames"; window.SOW_WS_URL = "wss://shadowsofwar.io/ws/"; window.SOW_MAPS_URL = window.location.origin + "/assets/maps";'
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
