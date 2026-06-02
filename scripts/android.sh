#!/usr/bin/env bash
# Android APK (native or webview). Usage: ./scripts/android.sh [native|webview]
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" android "$@"
