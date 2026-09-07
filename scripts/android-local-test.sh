#!/usr/bin/env bash
set -Eeuo pipefail

# Local Android smoke test. ./sow a calls this before publishing the AAB.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/android"

# Load the Play Games app/client IDs without coupling this identity test to
# store billing, achievements, or the Play publication step.
if [[ -f "$ROOT/sow-dist/.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    source "$ROOT/sow-dist/.env"
    set +a
fi

VARIANT="${SOW_ANDROID_TEST_VARIANT:-debug}"
ACTIVITY="com.shadowsofwar.TwaLauncherActivity"
OUT="$ROOT/dist/android/local-test"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$OUT/logcat-$STAMP.txt"
START="$OUT/start-$STAMP.txt"
VERSION_NAME="${SOW_ANDROID_TEST_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
VERSION_CODE="${SOW_ANDROID_TEST_VERSION_CODE:-$(tr -d '[:space:]' <"$ROOT/.android-version-code")}"
SKIP_BUILD="${SOW_ANDROID_SKIP_BUILD:-0}"
WEB_CACHE_BUST="${SOW_ANDROID_TEST_CACHE_BUST:-local-$STAMP}"
TEST_ROTATION="${SOW_ANDROID_TEST_ROTATION:-}"

ROTATION_LOCKED=0
restore_test_rotation() {
    if ((ROTATION_LOCKED)); then
        adb shell cmd window set-user-rotation free >/dev/null 2>&1 || true
        ROTATION_LOCKED=0
    fi
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

case "$TEST_ROTATION" in
    ""|portrait|landscape) ;;
    *) die "SOW_ANDROID_TEST_ROTATION must be portrait or landscape" ;;
esac

missing_play_games=()
for key in \
    SOW_PLAY_GAMES_APP_ID \
    SOW_PLAY_GAMES_WEB_CLIENT_ID; do
    [[ -n "${!key:-}" ]] || missing_play_games+=("$key")
done
if ((${#missing_play_games[@]} > 0)); then
    die "missing Play Games configuration in sow-dist/.env: ${missing_play_games[*]}"
fi
PLAY_GAMES_APP_ID_ARG="-PsowPlayGamesAppId=$SOW_PLAY_GAMES_APP_ID"
PLAY_GAMES_CLIENT_ID_ARG="-PsowPlayGamesWebClientId=$SOW_PLAY_GAMES_WEB_CLIENT_ID"
PLAY_GAMES_AUTH_URL_ARG="-PsowPlayGamesAuthUrl=${SOW_PLAY_GAMES_AUTH_URL:-https://shadowsofwar.io/api}"

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

ensure_device_awake() {
    POWER_STATE="$(adb shell dumpsys power 2>/dev/null | tr -d '\r' || true)"
    if grep -Eq 'mWakefulness=(Asleep|Dozing)|Display Power: state=OFF' <<<"$POWER_STATE"; then
        adb shell input keyevent KEYCODE_WAKEUP
        sleep 1
        POWER_STATE="$(adb shell dumpsys power 2>/dev/null | tr -d '\r' || true)"
        if grep -Eq 'mWakefulness=(Asleep|Dozing)|Display Power: state=OFF' <<<"$POWER_STATE"; then
            die "Android device screen could not be awakened over USB"
        fi
    fi
    WINDOW_STATE="$(adb shell dumpsys window 2>/dev/null | tr -d '\r' || true)"
    if grep -q 'mDreamingLockscreen=true' <<<"$WINDOW_STATE"; then
        die "Android device is locked; unlock it before the local test"
    fi
}

if [[ "$SKIP_BUILD" != "1" ]]; then
    (
        cd "$PROJECT"
        ./gradlew --warning-mode fail --no-daemon --no-configuration-cache "$TASK" \
            "-PsowVersionName=$VERSION_NAME" "-PsowVersionCode=$VERSION_CODE" \
            "-PsowWebCacheBust=$WEB_CACHE_BUST" \
            "$PLAY_GAMES_APP_ID_ARG" "$PLAY_GAMES_CLIENT_ID_ARG" "$PLAY_GAMES_AUTH_URL_ARG"
    )
fi

[[ -s "$APK" ]] || {
    echo "Android test APK missing: $APK" >&2
    exit 1
}

MERGED_MANIFEST="$PROJECT/app/build/intermediates/merged_manifests/$VARIANT/process${VARIANT^}Manifest/AndroidManifest.xml"
if ! rg -q 'com\.google\.android\.gms\.games\.SUPPRESS_GAME_PROFILE_CREATION' "$MERGED_MANIFEST"; then
    die "Play Games profile-creation suppression is missing from $MERGED_MANIFEST"
fi
if rg -q 'SPLASH_IMAGE_DRAWABLE|FILE_PROVIDER_AUTHORITY|sow_splash' "$MERGED_MANIFEST"; then
    die "native TWA splash is still configured in $MERGED_MANIFEST"
fi
if rg -q '\.signIn\(' "$PROJECT/app/src/main/java/com/shadowsofwar/TwaLauncherActivity.java"; then
    die "Play Games launcher must not invoke an interactive sign-in"
fi

ensure_device_awake
adb install -r "$APK" >"$OUT/install-$STAMP.txt"
WINDOW_STATE="$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)"
if rg -q 'mResumedActivity:.*CustomTabActivity' <<<"$WINDOW_STATE" \
    && rg -q "TaskRecord.*A=$PACKAGE" <<<"$WINDOW_STATE"; then
    adb shell am force-stop com.android.chrome
fi
adb shell am force-stop "$PACKAGE"
adb logcat -c
if [[ "$TEST_ROTATION" == "portrait" ]]; then
    adb shell cmd window set-user-rotation lock 0 >/dev/null
    ROTATION_LOCKED=1
elif [[ "$TEST_ROTATION" == "landscape" ]]; then
    adb shell cmd window set-user-rotation lock 1 >/dev/null
    ROTATION_LOCKED=1
fi

stdbuf -oL adb logcat -v threadtime -b main -b system -b crash \
    SOW_PGS:I AndroidRuntime:E chromium:I '*:S' >"$LOG" &
LOGGER_PID=$!
stop_logger() {
    kill "$LOGGER_PID" 2>/dev/null || true
    wait "$LOGGER_PID" 2>/dev/null || true
}
cleanup_local_test() {
    stop_logger
    restore_test_rotation
}
trap cleanup_local_test EXIT

# The TWA launcher is already foreground while Play Games identity work runs
# asynchronously, with a handoff only when a Play Games session is available.
if ! adb shell am start -n "$PACKAGE/$ACTIVITY" >"$START"; then
    die "Android activity failed to start; see $START"
fi
if ! grep -q '^Starting:' "$START"; then
    cat "$START" >&2
    die "Android activity did not start; see $START"
fi

echo "Waiting up to 45s for automatic Play Games authentication and TWA launch..."
deadline=$((SECONDS + 45))
while (( SECONDS < deadline )); do
    if rg -q "SOW_PGS.*TWA launched|SOW_PGS.*launching TWA" "$LOG"; then
        break
    fi
    sleep 1
done

if rg -q "SOW_PGS.*TWA launched|SOW_PGS.*launching TWA" "$LOG"; then
    deadline=$((SECONDS + 45))
    while (( SECONDS < deadline )); do
        WINDOW_STATE="$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)"
        if rg -q 'TwaLauncherActivity|com\.google\.androidbrowserhelper\.trusted\.LauncherActivity' <<<"$WINDOW_STATE" \
            && rg -q 'mResumedActivity:.*CustomTabActivity' <<<"$WINDOW_STATE"; then
            break
        fi
        sleep 1
    done
fi

stop_logger
trap - EXIT
restore_test_rotation

if rg -n -F "AndroidRuntime: Process: $PACKAGE, PID:" "$LOG"; then
    echo "FAIL: Android startup crash detected. Log: $LOG" >&2
    exit 1
fi
APP_LOG="$OUT/app-$STAMP.txt"
rg 'SOW_PGS' "$LOG" >"$APP_LOG" || true

if ! rg -q 'SOW_PGS' "$APP_LOG"; then
    echo "FAIL: Play Games startup produced no SOW_PGS diagnostic lines. Log: $APP_LOG" >&2
    exit 1
fi

if ! rg -q "SOW_PGS.*TWA launched|SOW_PGS.*launching TWA" "$LOG"; then
    WINDOW_STATE="$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)"
    if rg -q 'GamesResolutionActivity|SignInActivity' <<<"$WINDOW_STATE"; then
        echo "FAIL: Google Play Games displayed interactive account UI despite profile-creation suppression. Log: $APP_LOG" >&2
    else
        echo "FAIL: Android did not reach the TWA launch. Log: $APP_LOG" >&2
    fi
    exit 1
fi

WINDOW_STATE="$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)"
if rg -q 'GamesResolutionActivity|SignInActivity' <<<"$WINDOW_STATE"; then
    echo "FAIL: unexpected interactive Play Games UI remained after handoff. Log: $APP_LOG" >&2
    exit 1
fi
if ! rg -q 'TwaLauncherActivity|com\.google\.androidbrowserhelper\.trusted\.LauncherActivity' <<<"$WINDOW_STATE" \
    || ! rg -q 'mResumedActivity:.*CustomTabActivity' <<<"$WINDOW_STATE"; then
    echo "FAIL: TWA was not foreground after launch. Log: $APP_LOG" >&2
    exit 1
fi
if rg -q ' [WE] SOW_PGS :' "$LOG"; then
    rg ' [WE] SOW_PGS :' "$LOG" >&2
    echo "FAIL: Play Games launcher emitted an app warning/error. Log: $APP_LOG" >&2
    exit 1
fi

SCREENSHOT="$OUT/screenshot-$STAMP.png"
if ! adb exec-out screencap -p >"$SCREENSHOT"; then
    die "failed to capture the Android screen; reconnect the device and rerun the local test"
fi
if [[ ! -s "$SCREENSHOT" ]]; then
    die "Android screenshot is empty; reconnect the device and rerun the local test"
fi

if rg -q 'SOW_PGS.*rendezvous is ready' "$LOG"; then
    echo "PASS: Play Games handoff launched the TWA"
else
    echo "PASS: anonymous fallback launched the TWA without interactive Play Games UI"
fi
echo "Package: $PACKAGE"
echo "Launch result: $START"
echo "Logcat: $LOG"
echo "App log: $APP_LOG"
echo "Screenshot: $SCREENSHOT"
