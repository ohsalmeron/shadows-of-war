#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/macos/sow_macos.xcodeproj"
EXPORT_OPTIONS="$ROOT/sow-dist/deploy/macos/ExportOptions.plist"
VERSION_NAME="${SOW_MACOS_VERSION_NAME:-$(tr -d '[:space:]' < "$ROOT/.version")}"
BUILD_NUMBER="${SOW_MACOS_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
MIN_BUILD_NUMBER="${SOW_MACOS_MIN_BUILD_NUMBER:-408}"
RUN_ROOT="${SOW_MACOS_RUN_ROOT:-$ROOT/dist/macos-testflight/$VERSION_NAME-$BUILD_NUMBER}"
ARCHIVE="${SOW_MACOS_ARCHIVE_PATH:-$RUN_ROOT/ShadowsOfWar.xcarchive}"
EXPORT_DIR="${SOW_MACOS_EXPORT_DIR:-$RUN_ROOT/export}"
DERIVED_DATA_PATH="${SOW_MACOS_DERIVED_DATA_PATH:-$RUN_ROOT/DerivedData}"
TEAM_ID="${SOW_MACOS_TEAM_ID:-HS8F4NGXWN}"
USER_HOME="${HOME:?}"
PROFILE_DIR="${SOW_MACOS_PROFILE_DIR:-$USER_HOME/Library/Developer/Xcode/UserData/Provisioning Profiles}"
MACOS_PROFILE_UUID="${SOW_MACOS_PROVISIONING_PROFILE_UUID:-}"
ASC_API_KEY="${SOW_ASC_API_KEY:-}"
ASC_API_ISSUER="${SOW_ASC_API_ISSUER:-}"
ASC_P8_PATH="${SOW_ASC_P8_PATH:-}"
ACTIVE_EXPORT_OPTIONS=""
BUILD_SETTINGS=""
PACKAGE_STAGE=""

cleanup() {
    [[ -z "$ACTIVE_EXPORT_OPTIONS" ]] || rm -f "$ACTIVE_EXPORT_OPTIONS"
    [[ -z "$BUILD_SETTINGS" ]] || rm -f "$BUILD_SETTINGS"
    [[ -z "$PACKAGE_STAGE" ]] || rm -rf "$PACKAGE_STAGE"
}
trap cleanup EXIT

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${1:-}" == "" || "${1:-}" == "--upload" ]] || die "usage: scripts/macos-testflight.sh [--upload]"
[[ "$VERSION_NAME" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || die "invalid macOS version name: $VERSION_NAME"
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid macOS build number: $BUILD_NUMBER"
[[ "$MIN_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid minimum macOS build number: $MIN_BUILD_NUMBER"
(( BUILD_NUMBER >= MIN_BUILD_NUMBER )) \
    || die "macOS build $BUILD_NUMBER is not newer than the known failed build floor $MIN_BUILD_NUMBER"

command -v xcodebuild >/dev/null || die "xcodebuild is required"
command -v xcrun >/dev/null || die "xcrun is required"
command -v cargo >/dev/null || die "cargo is required"
command -v productbuild >/dev/null || die "productbuild is required"
command -v pkgutil >/dev/null || die "pkgutil is required"
security find-identity -v \
    | grep -Eq '"Apple Distribution:' \
    || die "no Apple Distribution identity found"
security find-identity -v \
    | grep -Eq '"3rd Party Mac Developer Installer:' \
    || die "no Mac App Store installer identity found"

if [[ -z "$MACOS_PROFILE_UUID" ]]; then
    for profile in "$PROFILE_DIR"/*.provisionprofile; do
        [[ -f "$profile" ]] || continue
        profile_name="$(security cms -D -i "$profile" 2>/dev/null \
            | plutil -extract Name raw -o - - 2>/dev/null || true)"
        profile_app_id="$(security cms -D -i "$profile" 2>/dev/null \
            | plutil -extract Entitlements.application-identifier raw -o - - 2>/dev/null || true)"
        profile_get_task_allow="$(security cms -D -i "$profile" 2>/dev/null \
            | plutil -extract Entitlements.get-task-allow raw -o - - 2>/dev/null || true)"
        profile_beta_reports="$(security cms -D -i "$profile" 2>/dev/null \
            | plutil -extract Entitlements.beta-reports-active raw -o - - 2>/dev/null || true)"
        if [[ "$profile_app_id" == "$TEAM_ID.games.shadowsofwar.app" \
            && "$profile_get_task_allow" == "false" \
            && "$profile_beta_reports" == "true" \
            && "$profile_name" == *Mac* ]]; then
            MACOS_PROFILE_UUID="$(security cms -D -i "$profile" 2>/dev/null \
                | plutil -extract UUID raw -o - - 2>/dev/null || true)"
            break
        fi
    done
fi
[[ -n "$MACOS_PROFILE_UUID" ]] \
    || die "no Mac App Store provisioning profile found for games.shadowsofwar.app; sign in to Xcode or set SOW_MACOS_PROVISIONING_PROFILE_UUID"

if [[ "${1:-}" == "--upload" ]]; then
    [[ -n "$ASC_API_KEY" && -n "$ASC_API_ISSUER" && -n "$ASC_P8_PATH" ]] \
        || die "--upload requires SOW_ASC_API_KEY, SOW_ASC_API_ISSUER, and SOW_ASC_P8_PATH"
    [[ -f "$ASC_P8_PATH" ]] || die "App Store Connect private key not found: $ASC_P8_PATH"
fi

mkdir -p "$(dirname "$ARCHIVE")" "$EXPORT_DIR" "$DERIVED_DATA_PATH"
rm -rf "$ARCHIVE" "$EXPORT_DIR"

BUILD_SETTINGS="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-mac-build-settings.XXXXXX")"
xcodebuild \
    -project "$PROJECT" \
    -scheme ShadowsOfWar \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -showBuildSettings >"$BUILD_SETTINGS"
grep -Eq '^    ARCHS = arm64$' "$BUILD_SETTINGS" \
    || die "macOS distribution target is not arm64"
grep -Eq '^    CODE_SIGN_STYLE = Automatic$' "$BUILD_SETTINGS" \
    || die "macOS distribution target is not using automatic signing"

echo "==> Archive macOS"
echo "==> version=$VERSION_NAME build=$BUILD_NUMBER team=$TEAM_ID architecture=arm64"
xcodebuild \
    -project "$PROJECT" \
    -scheme ShadowsOfWar \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -derivedDataPath "$DERIVED_DATA_PATH" \
    -archivePath "$ARCHIVE" \
    archive \
    -allowProvisioningUpdates \
    CODE_SIGN_STYLE=Automatic \
    DEVELOPMENT_TEAM="$TEAM_ID" \
    PROVISIONING_PROFILE_SPECIFIER="$MACOS_PROFILE_UUID" \
    MARKETING_VERSION="$VERSION_NAME" \
    CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

echo "==> Export macOS package"
ACTIVE_EXPORT_OPTIONS="${TMPDIR:-/tmp}/ShadowsOfWar-mac-ExportOptions.$$.plist"
cp "$EXPORT_OPTIONS" "$ACTIVE_EXPORT_OPTIONS"
/usr/libexec/PlistBuddy -c "Set :destination export" "$ACTIVE_EXPORT_OPTIONS"
xcodebuild \
    -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportPath "$EXPORT_DIR" \
    -exportOptionsPlist "$ACTIVE_EXPORT_OPTIONS" \
    -allowProvisioningUpdates

PACKAGE=""
PACKAGE_COUNT=0
while IFS= read -r candidate; do
    PACKAGE="$candidate"
    PACKAGE_COUNT=$((PACKAGE_COUNT + 1))
done < <(find "$EXPORT_DIR" -maxdepth 1 -type f -name '*.pkg' -print)

if [[ "$PACKAGE_COUNT" -eq 0 ]]; then
    APP_PATH=""
    APP_COUNT=0
    while IFS= read -r candidate; do
        APP_PATH="$candidate"
        APP_COUNT=$((APP_COUNT + 1))
    done < <(find "$EXPORT_DIR" -maxdepth 1 -type d -name '*.app' -print)
    [[ "$APP_COUNT" -eq 1 ]] || die "expected one exported macOS app or package, found $APP_COUNT apps and $PACKAGE_COUNT packages"
    PACKAGE="$EXPORT_DIR/ShadowsOfWar.pkg"
    productbuild \
        --sign "3rd Party Mac Developer Installer: Omar Hernandez Salmeron (HS8F4NGXWN)" \
        --component "$APP_PATH" /Applications "$PACKAGE"
elif [[ "$PACKAGE_COUNT" -ne 1 ]]; then
    die "expected exactly one exported macOS package, found $PACKAGE_COUNT"
fi

PACKAGE_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/ShadowsOfWar-mac-pkg.XXXXXX")"
pkgutil --expand "$PACKAGE" "$PACKAGE_STAGE/expanded"
APP_PATH="$(find "$PACKAGE_STAGE/expanded" -type d -name '*.app' -print -quit)"
[[ -n "$APP_PATH" ]] || die "macOS package does not contain an app bundle"

codesign --verify --deep --strict "$APP_PATH"
APP_INFO="$APP_PATH/Contents/Info.plist"
APP_EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_INFO")"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP_INFO")" == "games.shadowsofwar.app" ]] \
    || die "packaged macOS app has the wrong bundle identifier"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_INFO")" == "$VERSION_NAME" ]] \
    || die "packaged macOS app has the wrong marketing version"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP_INFO")" == "$BUILD_NUMBER" ]] \
    || die "packaged macOS app has the wrong build number"
[[ "$(lipo -archs "$APP_PATH/Contents/MacOS/$APP_EXECUTABLE")" == *arm64* ]] \
    || die "packaged macOS app is missing arm64"
[[ -f "$APP_PATH/Contents/embedded.provisionprofile" ]] \
    || die "packaged macOS app is missing its distribution provisioning profile"

SIGNING_DETAILS="$(codesign -dvvv "$APP_PATH" 2>&1 || true)"
grep -Eq '^Authority=(Apple Distribution|3rd Party Mac Developer Application):' <<<"$SIGNING_DETAILS" \
    || die "packaged macOS app is not signed for App Store distribution"
if grep -Eq '^Authority=(Apple Development|Developer ID Application):' <<<"$SIGNING_DETAILS"; then
    die "packaged macOS app has a development or Developer ID signature"
fi
grep -q '^TeamIdentifier=HS8F4NGXWN$' <<<"$SIGNING_DETAILS" \
    || die "packaged macOS app has the wrong signing team"
pkgutil --check-signature "$PACKAGE" \
    | grep -Eq '3rd Party Mac Developer Installer|Apple Distribution' \
    || die "macOS installer package is not signed for App Store distribution"

echo "PASS: signed macOS arm64 package is ready for App Store Connect"

if [[ "${1:-}" == "--upload" ]]; then
    echo "==> Upload macOS package to App Store Connect"
    xcrun altool \
        --upload-package "$PACKAGE" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "PASS: macOS upload accepted by App Store Connect; processing status still requires verification"
else
    echo "Package ready: $PACKAGE"
    echo "PASS: macOS archive/export completed; nothing uploaded"
fi
