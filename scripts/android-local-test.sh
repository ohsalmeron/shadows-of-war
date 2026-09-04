#!/usr/bin/env bash
set -Eeuo pipefail

# Local Android smoke test. ./sow a calls this before publishing the AAB.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/android"
VARIANT="${SOW_ANDROID_TEST_VARIANT:-debug}"
ACTIVITY="com.google.androidbrowserhelper.trusted.LauncherActivity"
OUT="$ROOT/dist/android/local-test"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$OUT/logcat-$STAMP.txt"
START="$OUT/start-$STAMP.txt"
VERSION_NAME="${SOW_ANDROID_TEST_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
VERSION_CODE="${SOW_ANDROID_TEST_VERSION_CODE:-$(tr -d '[:space:]' <"$ROOT/.android-version-code")}"
SKIP_BUILD="${SOW_ANDROID_SKIP_BUILD:-0}"

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${SOW_REVENUECAT_ANDROID_PUBLIC_KEY:-}" == goog_* ]] \
    || die "SOW_REVENUECAT_ANDROID_PUBLIC_KEY must be a Google Play public key"
REVENUECAT_KEY_ARG="-PrevenueCatAndroidPublicKey=$SOW_REVENUECAT_ANDROID_PUBLIC_KEY"

case "$VARIANT" in
    release)
        PACKAGE="com.shadowsofwar"
        TASK=":app:assembleRelease"
        APK="$PROJECT/app/build/outputs/apk/release/app-release.apk"
        ;;
    debug)
        PACKAGE="com.shadowsofwar.debug"
        TASK=":app:assembleDebug"
        APK="$PROJECT/app/build/outputs/apk/debug/app-debug.apk"
        ;;
    *)
        echo "SOW_ANDROID_TEST_VARIANT must be release or debug" >&2
        exit 1
        ;;
esac

mkdir -p "$OUT"

command -v adb >/dev/null || die "adb is required; install android-tools first"

if ! adb get-state 2>/dev/null | grep -qx device; then
    echo "No Android device is connected or USB debugging is unavailable." >&2
    adb devices -l >&2 || true
    exit 1
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
    (
        cd "$PROJECT"
        ./gradlew --warning-mode fail --no-daemon --no-configuration-cache "$TASK" \
            "-PsowVersionName=$VERSION_NAME" "-PsowVersionCode=$VERSION_CODE" "$REVENUECAT_KEY_ARG"
    )
fi

[[ -s "$APK" ]] || {
    echo "Android test APK missing: $APK" >&2
    exit 1
}

adb install -r "$APK" >"$OUT/install-$STAMP.txt"
adb shell am force-stop "$PACKAGE"
adb logcat -c

# Keep the logger alive while the app starts so async splash crashes are captured.
timeout 15s adb logcat -v threadtime -b main -b system -b crash >"$LOG" &
LOGGER_PID=$!
trap 'kill "$LOGGER_PID" 2>/dev/null || true; wait "$LOGGER_PID" 2>/dev/null || true' EXIT

adb shell am start -n "$PACKAGE/$ACTIVITY" >"$START"
wait "$LOGGER_PID" || true
trap - EXIT

if rg -n -F "AndroidRuntime: Process: $PACKAGE, PID:" "$LOG"; then
    echo "FAIL: Android startup crash detected. Log: $LOG" >&2
    exit 1
fi

APP_PID="$(adb shell pidof "$PACKAGE" 2>/dev/null | tr -d '\r' | awk '{print $1}' || true)"
APP_LOG="$OUT/app-$STAMP.txt"
APP_WARNINGS="$OUT/app-warnings-$STAMP.txt"
: >"$APP_LOG"
: >"$APP_WARNINGS"
if [ -n "$APP_PID" ]; then
    awk -v pid="$APP_PID" '$3 == pid {print}' "$LOG" >"$APP_LOG"
    # These are Android/Samsung graphics-runtime diagnostics, not app code.
    awk -v pid="$APP_PID" '$3 == pid && $5 ~ /^[WE]$/ && $6 !~ /^(Zygote|Gralloc3|libEGL):?$/ && $0 !~ /Not starting debugger since process cannot load the jdwp agent/ && $0 !~ /Unknown bits set in runtime_flags/ {print}' "$LOG" >"$APP_WARNINGS"
fi

if [ -s "$APP_WARNINGS" ]; then
    cat "$APP_WARNINGS" >&2
    echo "FAIL: app-originated warnings/errors detected. Log: $APP_WARNINGS" >&2
    exit 1
fi

if [ -s "$APP_LOG" ] && rg -q ' [WE] ' "$APP_LOG"; then
    echo "Device/framework diagnostics (not app-originated):"
    rg ' [WE] ' "$APP_LOG" || true
fi

echo "PASS: app started without a captured startup crash"
echo "Package: $PACKAGE"
echo "Launch result: $START"
echo "Logcat: $LOG"
echo "App log: $APP_LOG"
