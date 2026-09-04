#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT/scripts/check-legacy-ui-freeze.sh"
"$ROOT/scripts/vendor-blade.sh"
PROJECT="$ROOT/sow-dist/deploy/ios/sow_ios.xcodeproj"
EXPORT_OPTIONS="$ROOT/sow-dist/deploy/ios/ExportOptions.plist"
ARCHIVE="${SOW_IOS_ARCHIVE_PATH:-$ROOT/dist/ios/ShadowsOfWar.xcarchive}"
EXPORT_DIR="${SOW_IOS_EXPORT_DIR:-$ROOT/dist/ios/export}"
DERIVED_DATA_PATH="${SOW_IOS_DERIVED_DATA_PATH:-$ROOT/dist/ios/DerivedData}"
VERSION_NAME="${SOW_IOS_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
BUILD_NUMBER="${SOW_IOS_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
TEAM_ID="${SOW_IOS_TEAM_ID:-HS8F4NGXWN}"
REVENUECAT_IOS_PUBLIC_KEY="${SOW_REVENUECAT_IOS_PUBLIC_KEY:-}"
ASC_API_KEY="${SOW_ASC_API_KEY:-}"
ASC_API_ISSUER="${SOW_ASC_API_ISSUER:-}"
ASC_P8_PATH="${SOW_ASC_P8_PATH:-}"
ACTIVE_EXPORT_OPTIONS=""
IPA_STAGE=""
DSYM_SYMBOLS=""

cleanup() {
    [[ -z "$ACTIVE_EXPORT_OPTIONS" ]] || rm -f "$ACTIVE_EXPORT_OPTIONS"
    [[ -z "$IPA_STAGE" ]] || rm -rf "$IPA_STAGE"
    [[ -z "$DSYM_SYMBOLS" ]] || rm -f "$DSYM_SYMBOLS"
}
trap cleanup EXIT

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${1:-}" == "" || "${1:-}" == "--upload" ]] || die "usage: scripts/ios-testflight.sh [--upload]"
[[ "$VERSION_NAME" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || die "invalid iOS version name: $VERSION_NAME"
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid iOS build number: $BUILD_NUMBER"
[[ "$REVENUECAT_IOS_PUBLIC_KEY" == appl_* ]] \
    || die "SOW_REVENUECAT_IOS_PUBLIC_KEY must be the RevenueCat iOS public SDK key (appl_...)"

command -v xcodebuild >/dev/null || die "xcodebuild is required"
command -v xcrun >/dev/null || die "xcrun is required"
command -v cargo >/dev/null || die "cargo is required; install Rust and run: rustup target add aarch64-apple-ios"
rustup target list --installed 2>/dev/null | grep -qx aarch64-apple-ios \
    || die "missing Rust iOS target; run: rustup target add aarch64-apple-ios"
security find-identity -v -p codesigning \
    | grep -Eq "Apple (Distribution|Development)|iPhone (Distribution|Developer)" \
    || die "no iOS signing identity found; add the Apple account in Xcode"

if [[ "${1:-}" == "--upload" ]]; then
    [[ -n "$ASC_API_KEY" && -n "$ASC_API_ISSUER" && -n "$ASC_P8_PATH" ]] \
        || die "--upload requires SOW_ASC_API_KEY, SOW_ASC_API_ISSUER, and SOW_ASC_P8_PATH"
    [[ -f "$ASC_P8_PATH" ]] || die "App Store Connect private key not found: $ASC_P8_PATH"
fi

mkdir -p "$(dirname "$ARCHIVE")" "$EXPORT_DIR" "$DERIVED_DATA_PATH"
rm -rf "$ARCHIVE" "$EXPORT_DIR"
export SOW_IOS_CARGO_HOME="${SOW_IOS_CARGO_HOME:-${CARGO_HOME:-$HOME/.cargo}}"

echo "==> Archive iOS"
echo "==> version=$VERSION_NAME build=$BUILD_NUMBER team=$TEAM_ID"
xcodebuild \
    -project "$PROJECT" \
    -scheme ShadowsOfWar \
    -configuration Release \
    -destination "generic/platform=iOS" \
    -derivedDataPath "$DERIVED_DATA_PATH" \
    -archivePath "$ARCHIVE" \
    archive \
    -allowProvisioningUpdates \
    CODE_SIGN_STYLE=Automatic \
    DEVELOPMENT_TEAM="$TEAM_ID" \
    MARKETING_VERSION="$VERSION_NAME" \
    CURRENT_PROJECT_VERSION="$BUILD_NUMBER" \
    SOW_REVENUECAT_IOS_PUBLIC_KEY="$REVENUECAT_IOS_PUBLIC_KEY"

echo "==> Export IPA"
ACTIVE_EXPORT_OPTIONS="${TMPDIR:-/tmp}/ShadowsOfWar-ExportOptions.$$.plist"
cp "$EXPORT_OPTIONS" "$ACTIVE_EXPORT_OPTIONS"
/usr/libexec/PlistBuddy -c "Set :destination export" "$ACTIVE_EXPORT_OPTIONS"

xcodebuild \
    -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportPath "$EXPORT_DIR" \
    -exportOptionsPlist "$ACTIVE_EXPORT_OPTIONS"

IPA="$(find "$EXPORT_DIR" -maxdepth 1 -name '*.ipa' -print -quit)"
[[ -n "$IPA" && -s "$IPA" ]] || die "IPA export did not produce an .ipa"

IPA_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/ShadowsOfWar-ipa.XXXXXX")"
unzip -q "$IPA" -d "$IPA_STAGE"
APP_PATH="$(find "$IPA_STAGE/Payload" -maxdepth 2 -type d -name '*.app' -print -quit)"
[[ -n "$APP_PATH" ]] || die "IPA export did not contain an app bundle"
codesign --verify --deep --strict "$APP_PATH"

APP_INFO="$APP_PATH/Info.plist"
APP_EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_INFO")"
PACKAGED_REVENUECAT_KEY="$(/usr/libexec/PlistBuddy -c 'Print :SOWRevenueCatIOSPublicKey' "$APP_INFO")"
[[ "$PACKAGED_REVENUECAT_KEY" == "$REVENUECAT_IOS_PUBLIC_KEY" ]] \
    || die "packaged app contains the wrong RevenueCat iOS public SDK key"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_INFO")" == "$VERSION_NAME" ]] \
    || die "packaged app contains the wrong marketing version"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP_INFO")" == "$BUILD_NUMBER" ]] \
    || die "packaged app contains the wrong build number"
[[ -d "$APP_PATH/RevenueCat_RevenueCat.bundle" ]] \
    || die "packaged app is missing the official RevenueCat resource bundle"
if find "$APP_PATH" -type f \( -name 'libSOWRevenueCatBridge.dylib' -o -name '*RevenueCatBridge*.dylib' \) \
    | grep -q .; then
    die "packaged app still contains the removed manual RevenueCat bridge dylib"
fi
otool -L "$APP_PATH/$APP_EXECUTABLE" | grep -q '/StoreKit.framework/StoreKit' \
    || die "packaged app is not linked with StoreKit"

DSYM_BINARY="$ARCHIVE/dSYMs/$(basename "$APP_PATH").dSYM/Contents/Resources/DWARF/$APP_EXECUTABLE"
[[ -f "$DSYM_BINARY" ]] || die "archive is missing the app dSYM"
DSYM_SYMBOLS="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-symbols.XXXXXX")"
nm -gj "$DSYM_BINARY" >"$DSYM_SYMBOLS"
for symbol in \
    _sow_ios_main \
    _sow_ios_config_value \
    _sow_revenuecat_open_store \
    _sow_ios_revenuecat_purchase_completed; do
    grep -qx "$symbol" "$DSYM_SYMBOLS" \
        || die "archive is missing required bridge symbol: $symbol"
done

echo "PASS: signed IPA contains one static RevenueCat integration and all Rust/Swift bridge symbols"

if [[ "${1:-}" == "--upload" ]]; then
    echo "==> Upload IPA to App Store Connect"
    xcrun altool \
        --upload-package "$IPA" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "PASS: upload accepted by App Store Connect; processing status still requires verification"
else
    echo "IPA ready: $IPA"
    echo "PASS: iOS archive/export completed; nothing uploaded"
fi
