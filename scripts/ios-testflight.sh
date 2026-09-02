#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/ios/sow_ios.xcodeproj"
EXPORT_OPTIONS="$ROOT/sow-dist/deploy/ios/ExportOptions.plist"
UPLOAD_EXPORT_OPTIONS="${TMPDIR:-/tmp}/ShadowsOfWar-UploadExportOptions.plist"
ARCHIVE="${SOW_IOS_ARCHIVE_PATH:-$ROOT/dist/ios/ShadowsOfWar.xcarchive}"
EXPORT_DIR="${SOW_IOS_EXPORT_DIR:-$ROOT/dist/ios/export}"
DERIVED_DATA_PATH="${SOW_IOS_DERIVED_DATA_PATH:-$ROOT/dist/ios/DerivedData}"
VERSION_NAME="${SOW_IOS_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
BUILD_NUMBER="${SOW_IOS_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
TEAM_ID="${SOW_IOS_TEAM_ID:-HS8F4NGXWN}"

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${1:-}" == "" || "${1:-}" == "--upload" ]] || die "usage: scripts/ios-testflight.sh [--upload]"
[[ "$VERSION_NAME" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || die "invalid iOS version name: $VERSION_NAME"
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid iOS build number: $BUILD_NUMBER"

command -v xcodebuild >/dev/null || die "xcodebuild is required"
command -v xcrun >/dev/null || die "xcrun is required"
command -v cargo >/dev/null || die "cargo is required; install Rust and run: rustup target add aarch64-apple-ios"
rustup target list --installed 2>/dev/null | grep -qx aarch64-apple-ios \
    || die "missing Rust iOS target; run: rustup target add aarch64-apple-ios"
security find-identity -v -p codesigning \
    | grep -Eq "Apple (Distribution|Development)|iPhone (Distribution|Developer)" \
    || die "no iOS App Store/TestFlight signing identity found; create Apple Distribution or Apple Development in Xcode"

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
    CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

echo "==> Export IPA"
ACTIVE_EXPORT_OPTIONS="$EXPORT_OPTIONS"
if [[ "${1:-}" == "--upload" ]]; then
    ACTIVE_EXPORT_OPTIONS="$UPLOAD_EXPORT_OPTIONS"
    cp "$EXPORT_OPTIONS" "$ACTIVE_EXPORT_OPTIONS"
    /usr/libexec/PlistBuddy -c "Set :destination upload" "$ACTIVE_EXPORT_OPTIONS"
fi

xcodebuild \
    -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportPath "$EXPORT_DIR" \
    -exportOptionsPlist "$ACTIVE_EXPORT_OPTIONS" \
    -allowProvisioningUpdates

if [[ "${1:-}" == "--upload" ]]; then
    echo "PASS: upload submitted to App Store Connect/TestFlight"
else
    IPA="$(find "$EXPORT_DIR" -maxdepth 1 -name '*.ipa' -print -quit)"
    [[ -n "$IPA" && -s "$IPA" ]] || die "IPA export did not produce an .ipa"
    echo "IPA ready: $IPA"
    echo "PASS: iOS archive/export completed; nothing uploaded"
fi
