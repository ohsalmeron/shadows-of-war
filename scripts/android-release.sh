#!/usr/bin/env bash
set -Eeuo pipefail

# Android publication phase invoked automatically by ./sow p after the AAB
# has passed the local device smoke test.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/android"
PACKAGE="com.shadowsofwar"
TRACK="${SOW_ANDROID_PLAY_TRACK:-alpha}"
VERSION_NAME="${SOW_ANDROID_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
SOW_USER_HOME="$(getent passwd "$(id -u)" | cut -d: -f6)"
PLAY_KEY="${SOW_PLAY_KEY:-$SOW_USER_HOME/.config/shadows-of-war/google-play-service-account.json}"
SOW_RUBY_BIN="$SOW_USER_HOME/.local/share/gem/ruby/3.4.0/bin"
export PATH="$SOW_RUBY_BIN:$PATH"
export FASTLANE_OPT_OUT_USAGE=1
export FASTLANE_SKIP_UPDATE_CHECK=1
export FASTLANE_DISABLE_COLORS=1

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${1:-}" == "" || "${1:-}" == "--publish" || "${1:-}" == "--publish-existing" ]] \
    || die "usage: scripts/android-release.sh [--publish|--publish-existing]"
PUBLISH="${1:-}"

command -v fastlane >/dev/null || die "fastlane 2.238.0 is required"
if [[ "$PUBLISH" != "--publish-existing" ]]; then
    command -v adb >/dev/null || die "adb is required"
fi
[[ -x "$PROJECT/gradlew" ]] || die "Android Gradle wrapper missing: $PROJECT/gradlew"
[[ -f "$PROJECT/key.properties" ]] || die "Android signing properties missing"
[[ -f "$PLAY_KEY" ]] || die "Google Play service-account key missing: $PLAY_KEY"
[[ "$PLAY_KEY" != "$ROOT"/* ]] || die "Google Play key must stay outside the repository"
[[ "$(stat -c '%a' "$PLAY_KEY")" == "600" ]] || die "Google Play key must have mode 600"
[[ "$(stat -c '%a' "$PROJECT/key.properties")" == "600" ]] || die "Android signing properties must have mode 600"
[[ "$VERSION_NAME" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "invalid Android version name: $VERSION_NAME"

highest_play_code=0
for play_track in internal alpha beta open production; do
    track_output="$(fastlane run google_play_track_version_codes \
        package_name:"$PACKAGE" track:"$play_track" json_key:"$PLAY_KEY" 2>&1)" \
        || die "could not read Google Play track: $play_track"
    result_line="$(printf '%s\n' "$track_output" | sed -n 's/.*Result: //p' | tail -n 1)"
    [[ "$result_line" == \[*\] ]] || die "Google Play returned no version list for track: $play_track"
    codes="${result_line#[}"
    codes="${codes%]}"
    [[ -z "$codes" || "$codes" =~ ^[0-9]+(,[[:space:]]*[0-9]+)*$ ]] \
        || die "Google Play returned malformed version list for track: $play_track"
    for code in ${codes//,/ }; do
        [[ -z "$code" ]] && continue
        (( code > highest_play_code )) && highest_play_code="$code"
    done
done

LOCAL_COUNTER="$(tr -d '[:space:]' <"$ROOT/.android-version-code")"
[[ "$LOCAL_COUNTER" =~ ^[1-9][0-9]*$ ]] || die "invalid local Android version counter"
(( LOCAL_COUNTER > highest_play_code )) && highest_play_code="$LOCAL_COUNTER"
VERSION_CODE=$((highest_play_code + 1))
if [[ -n "${SOW_ANDROID_EXPECTED_VERSION_CODE:-}" ]]; then
    [[ "$SOW_ANDROID_EXPECTED_VERSION_CODE" =~ ^[1-9][0-9]*$ ]] \
        || die "invalid SOW_ANDROID_EXPECTED_VERSION_CODE"
    VERSION_CODE="$SOW_ANDROID_EXPECTED_VERSION_CODE"
fi
(( VERSION_CODE > 0 && VERSION_CODE < 2100000000 )) || die "Android versionCode exhausted"
echo "==> Android $PACKAGE"
echo "==> Play track: $TRACK"
echo "==> versionName=$VERSION_NAME versionCode=$VERSION_CODE"

AAB="$PROJECT/app/build/outputs/bundle/release/app-release.aab"
if [[ "$PUBLISH" == "--publish-existing" ]]; then
    (( VERSION_CODE >= highest_play_code )) || die "existing AAB versionCode is already used in Google Play"
    [[ -s "$AAB" ]] || die "existing release AAB missing: $AAB"
    echo "==> Reusing previously device-tested AAB"
else
    echo "==> Local device smoke test"
    SOW_ANDROID_TEST_VERSION_NAME="$VERSION_NAME" \
    SOW_ANDROID_TEST_VERSION_CODE="$VERSION_CODE" \
        "$ROOT/scripts/android-local-test.sh"

    echo "==> Build release AAB"
    (
        cd "$PROJECT"
        ./gradlew --warning-mode fail --no-daemon --no-configuration-cache :app:bundleRelease \
            "-PsowVersionName=$VERSION_NAME" \
            "-PsowVersionCode=$VERSION_CODE" \
            "-PrevenueCatAndroidPublicKey=${SOW_REVENUECAT_ANDROID_PUBLIC_KEY:-}"
    )
fi

[[ -s "$AAB" ]] || die "release AAB missing: $AAB"
METADATA=""
for candidate in \
    "$PROJECT/app/build/intermediates/merged_manifests/release/processReleaseManifest/output-metadata.json" \
    "$PROJECT/app/build/intermediates/merged_manifests/release/output-metadata.json" \
    "$PROJECT/app/build/outputs/apk/release/output-metadata.json"; do
    if [[ -s "$candidate" ]]; then
        METADATA="$candidate"
        break
    fi
done
[[ -n "$METADATA" ]] || die "Android output metadata missing"

python3 - "$METADATA" "$VERSION_NAME" "$VERSION_CODE" <<'PY'
import json
import sys

metadata_path, expected_name, expected_code = sys.argv[1:]
with open(metadata_path, encoding="utf-8") as f:
    metadata = json.load(f)
if metadata.get("applicationId") != "com.shadowsofwar":
    raise SystemExit(f"wrong applicationId: {metadata.get('applicationId')!r}")
elements = metadata.get("elements") or []
if not elements:
    raise SystemExit("AAB metadata has no elements")
actual_code = int(elements[0].get("versionCode", -1))
actual_name = str(elements[0].get("versionName", ""))
if actual_code != int(expected_code) or actual_name != expected_name:
    raise SystemExit(
        f"version mismatch: built {actual_name} ({actual_code}), "
        f"expected {expected_name} ({expected_code})"
    )
print("AAB metadata: OK")
PY

UPLOAD_DIR="$ROOT/dist/android/upload"
UPLOAD_AAB="$UPLOAD_DIR/$PACKAGE.aab"
install -d "$UPLOAD_DIR"
install -m 0644 "$AAB" "$UPLOAD_AAB"
echo "AAB ready: $UPLOAD_AAB"

echo "==> Google Play validation (no release commit)"
fastlane supply \
    --aab "$UPLOAD_AAB" \
    --package_name "$PACKAGE" \
    --track "$TRACK" \
    --json_key "$PLAY_KEY" \
    --skip_upload_metadata true \
    --skip_upload_images true \
    --skip_upload_screenshots true \
    --skip_upload_changelogs true \
    --validate_only true

if [[ "$PUBLISH" != "--publish" && "$PUBLISH" != "--publish-existing" ]]; then
    echo "PASS: Android build, device test, and Play validation completed; nothing published"
    exit 0
fi

echo "==> Publish to Google Play $TRACK"
fastlane supply \
    --aab "$UPLOAD_AAB" \
    --package_name "$PACKAGE" \
    --track "$TRACK" \
    --release_status completed \
    --json_key "$PLAY_KEY" \
    --skip_upload_metadata true \
    --skip_upload_images true \
    --skip_upload_screenshots true \
    --skip_upload_changelogs true

verify_output="$(fastlane run google_play_track_version_codes \
    package_name:"$PACKAGE" track:"$TRACK" json_key:"$PLAY_KEY" 2>&1)" \
    || die "release uploaded but track verification failed"
verified_codes="$(printf '%s\n' "$verify_output" | sed -n 's/.*Result: //p' | tail -n 1)"
[[ "$verified_codes" =~ (^|[^0-9])$VERSION_CODE([^0-9]|$) ]] \
    || die "release uploaded but versionCode $VERSION_CODE is not visible in $TRACK"
echo "PASS: versionCode $VERSION_CODE is visible in Google Play $TRACK"
