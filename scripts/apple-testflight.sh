#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_NAME="${SOW_APPLE_VERSION_NAME:-$(tr -d '[:space:]' < "$ROOT/.version")}"
BUILD_NUMBER="${SOW_APPLE_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
IOS_RUN_ROOT="${SOW_IOS_RUN_ROOT:-$ROOT/dist/ios/runs/$VERSION_NAME-$BUILD_NUMBER}"
MACOS_RUN_ROOT="${SOW_MACOS_RUN_ROOT:-$ROOT/dist/macos-testflight/$VERSION_NAME-$BUILD_NUMBER}"
ASC_API_KEY="${SOW_ASC_API_KEY:-}"
ASC_API_ISSUER="${SOW_ASC_API_ISSUER:-}"
ASC_P8_PATH="${SOW_ASC_P8_PATH:-}"
ASC_APP_ID="${SOW_ASC_APP_ID:-}"

die() {
    echo "ERROR: $*" >&2
    exit 1
}

usage() {
    cat >&2 <<'EOF'
usage: scripts/apple-testflight.sh prepare|upload|status

prepare  Build and validate iOS and macOS artifacts without uploading.
upload   Prepare both platforms, then upload both validated artifacts.
status   Read the real App Store Connect build-processing state.
EOF
    exit 2
}

require_asc_credentials() {
    [[ -n "$ASC_API_KEY" && -n "$ASC_API_ISSUER" && -n "$ASC_P8_PATH" ]] \
        || die "App Store Connect status/upload requires SOW_ASC_API_KEY, SOW_ASC_API_ISSUER, and SOW_ASC_P8_PATH"
    [[ -f "$ASC_P8_PATH" ]] || die "App Store Connect private key not found: $ASC_P8_PATH"
    command -v jq >/dev/null || die "jq is required for App Store Connect status"
    command -v curl >/dev/null || die "curl is required for App Store Connect status"
    command -v openssl >/dev/null || die "openssl is required for App Store Connect status"
}

base64url() {
    base64 | tr -d '\n' | tr '+/' '-_' | tr -d '='
}

asc_token() {
    local issued_at expiration header payload signing_input signature
    issued_at="$(date +%s)"
    expiration=$((issued_at + 1200))
    header="$(printf '{"'"'alg'"'":"'"'ES256'"'","'"'kid'"'":"'"'%s'"'","'"'typ'"'":"'"'JWT'"'}' "$ASC_API_KEY" | base64url)"
    payload="$(printf '{"'"'iss'"'":"'"'%s'"'","'"'iat'"'":%s,"'"'exp'"'":%s,"'"'aud'"'":"'"'appstoreconnect-v1'"'"}' "$ASC_API_ISSUER" "$issued_at" "$expiration" | base64url)"
    signing_input="$header.$payload"
    signature="$(printf '%s' "$signing_input" | openssl dgst -sha256 -sign "$ASC_P8_PATH" | base64url)"
    printf '%s.%s' "$signing_input" "$signature"
}

asc_get() {
    local token
    token="$(asc_token)"
    curl -fsS \
        -H "Authorization: Bearer $token" \
        -H 'Accept: application/json' \
        "$@"
}

resolve_app_id() {
    [[ -n "$ASC_APP_ID" ]] && return
    local response
    response="$(asc_get --get \
        --data-urlencode 'filter[bundleId]=games.shadowsofwar.app' \
        'https://api.appstoreconnect.apple.com/v1/apps')"
    ASC_APP_ID="$(jq -r '.data[0].id // empty' <<<"$response")"
    [[ -n "$ASC_APP_ID" ]] || die "App Store Connect has no app for bundle ID games.shadowsofwar.app"
}

prepare() {
    echo "==> Prepare iOS"
    SOW_IOS_RUN_ROOT="$IOS_RUN_ROOT" "$ROOT/scripts/ios-testflight.sh"
    echo "==> Prepare macOS"
    SOW_MACOS_RUN_ROOT="$MACOS_RUN_ROOT" "$ROOT/scripts/macos-testflight.sh"
    echo "PASS: iOS and macOS artifacts passed local distribution gates"
}

upload() {
    require_asc_credentials
    prepare
    local ipa pkg
    ipa="$IOS_RUN_ROOT/export/ShadowsOfWar.ipa"
    pkg="$MACOS_RUN_ROOT/export/ShadowsOfWar.pkg"
    [[ -s "$ipa" ]] || die "validated iOS IPA is missing: $ipa"
    [[ -s "$pkg" ]] || die "validated macOS package is missing: $pkg"
    echo "==> Upload iOS"
    xcrun altool \
        --upload-package "$ipa" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "==> Upload macOS"
    xcrun altool \
        --upload-package "$pkg" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "PASS: both uploads were accepted by App Store Connect; processing still requires status verification"
}

status() {
    require_asc_credentials
    resolve_app_id
    local response
    response="$(asc_get \
        --get \
        --data-urlencode 'include=preReleaseVersion' \
        --data-urlencode 'fields[builds]=version,uploadedDate,processingState,buildAudienceType,usesNonExemptEncryption' \
        --data-urlencode 'fields[preReleaseVersions]=platform,version' \
        --data-urlencode 'limit=200' \
        --data-urlencode 'sort=-uploadedDate' \
        "https://api.appstoreconnect.apple.com/v1/apps/$ASC_APP_ID/builds")"
    jq -e '.data | length > 0' >/dev/null <<<"$response" \
        || die "App Store Connect returned no processed or processing builds for app $ASC_APP_ID"
    echo "App Store Connect builds for $ASC_APP_ID"
    jq -r '
        .included as $included |
        .data[] |
        (.relationships.preReleaseVersion.data.id // "") as $pre_release_id |
        (($included[]? | select(.type == "preReleaseVersions" and .id == $pre_release_id) | .attributes.platform) // "UNKNOWN") as $platform |
        [ $platform,
          .attributes.version,
          .attributes.processingState,
          (.attributes.buildAudienceType // "-"),
          ((.attributes.usesNonExemptEncryption // "-") | tostring),
          (.attributes.uploadedDate // "-") ] |
        @tsv
    ' <<<"$response" | column -t -s $'\t'
}

case "${1:-}" in
    prepare) prepare ;;
    upload) upload ;;
    status) status ;;
    *) usage ;;
esac
