#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────
# Shadows of War - Dual Android Build Pipeline
# Supports:
#   1. Native GLES (Zero-overhead, ideal for low-RAM legacy hardware)
#   2. V8 WebView (High-performance, ideal for Vulkan 1.1+ devices)
# ──────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

green() { echo -e "\e[32m$1\e[0m"; }
cyan()  { echo -e "\e[36m$1\e[0m"; }
red()   { echo -e "\e[31m$1\e[0m"; }
yellow() { echo -e "\e[33m$1\e[0m"; }

fail() {
  red "❌ $1"
  shift
  for line in "$@"; do echo "   $line"; done
  exit 1
}

# ── Select Target ──────────────────────────────────────────────
TARGET="${1:-native}" # Default to native GLES for max compatibility

if [[ "${TARGET}" != "native" && "${TARGET}" != "webview" ]]; then
  fail "Invalid target: '${TARGET}'" \
       "Usage: ./scripts/android.sh [native|webview]" \
       "  native  : Build legacy GLES APK (optimized for low-RAM devices)" \
       "  webview : Build high-performance V8 WebView Vulkan APK"
fi

cyan "==> Build Target: ${TARGET^^}"

# ── SDK auto-detect ────────────────────────────────────────────
if [[ -z "${ANDROID_HOME:-}" && -n "${ANDROID_SDK_ROOT:-}" ]]; then
  export ANDROID_HOME="${ANDROID_SDK_ROOT}"
fi
if [[ -z "${ANDROID_HOME:-}" ]]; then
  for candidate in \
      "${HOME}/Android/Sdk" \
      "${HOME}/Library/Android/sdk" \
      "/opt/android-sdk" \
      "/usr/lib/android-sdk"; do
    if [[ -d "${candidate}" ]]; then
      export ANDROID_HOME="${candidate}"
      break
    fi
  done
fi
if [[ -z "${ANDROID_HOME:-}" || ! -d "${ANDROID_HOME}" ]]; then
  fail "Android SDK not found." \
       "Install Android Studio, let it provision \$HOME/Android/Sdk, then re-run."
fi
export ANDROID_SDK_ROOT="${ANDROID_HOME}"
cyan "==> Android SDK : ${ANDROID_HOME}"

# ── NDK auto-detect ────────────────────────────────────────────
if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
  if [[ -d "${ANDROID_HOME}/ndk" ]]; then
    LATEST_NDK=$(ls -1 "${ANDROID_HOME}/ndk" 2>/dev/null | sort -V | tail -n 1 || true)
    if [[ -n "${LATEST_NDK}" ]]; then
      export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk/${LATEST_NDK}"
    fi
  fi
fi
if [[ -z "${ANDROID_NDK_ROOT:-}" || ! -d "${ANDROID_NDK_ROOT}" ]]; then
  if [[ -d "${ANDROID_HOME}/ndk-bundle" ]]; then
    export ANDROID_NDK_ROOT="${ANDROID_HOME}/ndk-bundle"
  fi
fi
if [[ -n "${ANDROID_NDK_ROOT:-}" ]]; then
  cyan "==> Android NDK : ${ANDROID_NDK_ROOT}"
  export NDK_HOME="${ANDROID_NDK_ROOT}"
fi

# ── JDK auto-detect ────────────────────────────────────────────
if [[ -z "${JAVA_HOME:-}" || ! -x "${JAVA_HOME}/bin/java" ]]; then
  JAVA_HOME=""
  for candidate in \
      "/opt/android-studio/jbr" \
      "/usr/lib/android-studio/jbr" \
      "${HOME}/.local/share/JetBrains/Toolbox/apps/AndroidStudio/jbr"; do
    if [[ -x "${candidate}/bin/java" ]]; then JAVA_HOME="${candidate}"; break; fi
  done
  if [[ -z "${JAVA_HOME}" ]] && command -v java >/dev/null 2>&1; then
    JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v java)")")")"
  fi
  if [[ -z "${JAVA_HOME}" ]]; then
    for candidate in /usr/lib/jvm/default /usr/lib/jvm/java-21-openjdk /usr/lib/jvm/java-17-openjdk; do
      if [[ -x "${candidate}/bin/java" ]]; then JAVA_HOME="${candidate}"; break; fi
    done
  fi
fi
if [[ -z "${JAVA_HOME}" || ! -x "${JAVA_HOME}/bin/java" ]]; then
  fail "No JDK found." \
       "Arch: sudo pacman -S jdk-openjdk" \
       "Or install Android Studio (it ships /opt/android-studio/jbr)."
fi
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${HOME}/.cargo/bin:${PATH}"
cyan "==> JDK         : ${JAVA_HOME}"

command -v cargo >/dev/null || fail "cargo not found"

# ── Execution Branch ───────────────────────────────────────────
if [[ "${TARGET}" == "native" ]]; then
  # 1. Compile Native GLES APK
  rustup target list --installed 2>/dev/null | grep -qx aarch64-linux-android || fail "run: rustup target add aarch64-linux-android"
  command -v cargo-apk >/dev/null 2>&1 || fail "cargo-apk not in PATH — install: cargo install cargo-apk"
  
  MANIFEST="${ROOT}/sow-client/Cargo.toml"
  [[ -f "$MANIFEST" ]] && grep -q '^\[package.metadata.android\]' "$MANIFEST" 2>/dev/null || fail "missing sow-client/Cargo.toml with [package.metadata.android]"
  
  # Ensure release keystore is created
  KEYSTORE="${ROOT}/keystores/release.keystore"
  if [[ ! -f "${KEYSTORE}" ]]; then
    cyan "==> Generating release keystore at ${KEYSTORE}"
    mkdir -p "$(dirname "${KEYSTORE}")"
    keytool -genkeypair -v \
      -keystore "${KEYSTORE}" \
      -alias shadows \
      -keyalg RSA -keysize 2048 -validity 10000 \
      -storepass shadowswar -keypass shadowswar \
      -dname "CN=Shadows Of War, OU=Self, O=LegacyEngine, L=Local, S=NA, C=US" \
      >/dev/null
  fi

  cyan "📦 Building Native GLES APK..."
  RUSTFLAGS='--cfg gles' cargo apk build --release --lib -p sow-client
  
  APK_SRC="${ROOT}/target/release/apk/sow-client.apk"
  APK_OUT="${ROOT}/build/sow-client-native.apk"

else
  # 2. Compile WebAssembly + WebView APK
  rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown || {
    cyan "==> Installing wasm32 Rust target..."
    rustup target add wasm32-unknown-unknown
  }

  if [[ ! -f "${ROOT}/android/gradlew" ]]; then
    cyan "==> Bootstrapping Gradle Wrapper..."
    curl -sSLo "${ROOT}/android/gradlew" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradlew
    chmod +x "${ROOT}/android/gradlew"
  fi
  if [[ ! -f "${ROOT}/android/gradle/wrapper/gradle-wrapper.jar" ]]; then
    mkdir -p "${ROOT}/android/gradle/wrapper"
    curl -sSLo "${ROOT}/android/gradle/wrapper/gradle-wrapper.jar" https://raw.githubusercontent.com/gradle/gradle/v8.5.0/gradle/wrapper/gradle-wrapper.jar
  fi

  cyan "📦 Compiling shadows-of-war for WASM..."
  RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --release -p sow-client --target wasm32-unknown-unknown

  WASM_IN="target/wasm32-unknown-unknown/release/sow_client.wasm"
  [[ -f "${WASM_IN}" ]] || fail "WASM binary not found at ${WASM_IN}"

  cyan "🔄 Packaging WASM and Web Assets..."
  ASSETS_DIR="${ROOT}/android/app/src/main/assets"
  mkdir -p "${ASSETS_DIR}"
  rm -rf "${ASSETS_DIR:?}"/*

  # Run wasm-bindgen
  ~/.cargo/bin/wasm-bindgen --out-dir "${ASSETS_DIR}" --target web --out-name "sow_client" --no-typescript "${WASM_IN}"

  # Compile HTML template
  CLEAN_VERSION=$(cat "${ROOT}/.version" 2>/dev/null || echo "0.1.0")
  BUILD_TS=$(date +%s)
  LOADER_TEMPLATE="${ROOT}/web/index.html.template"
  [[ -f "${LOADER_TEMPLATE}" ]] || fail "HTML template missing: ${LOADER_TEMPLATE}"

  sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
      -e "s/__JS_FILE__/sow_client.js/g" \
      -e "s/__WASM_FILE__/sow_client_bg.wasm/g" \
      -e "s/__BUILD_TS__/${BUILD_TS}/g" \
      "${LOADER_TEMPLATE}" > "${ASSETS_DIR}/index.html"

  rsync -a "${ROOT}/assets/" "${ASSETS_DIR}/assets/" || true
  cp "${ROOT}/web/sow.svg" "${ASSETS_DIR}/sow.svg" || true

  cyan "📦 Compiling Android WebView App..."
  cd "${ROOT}/android"
  ./gradlew clean assembleDebug
  cd "${ROOT}"

  APK_SRC="${ROOT}/android/app/build/outputs/apk/debug/app-debug.apk"
  APK_OUT="${ROOT}/build/sow-client-webview.apk"
fi

# ── ADB Deployment ─────────────────────────────────────────────
if [[ -f "${APK_SRC}" ]]; then
    mkdir -p "${ROOT}/build"
    cp "${APK_SRC}" "${APK_OUT}"
    
    green "🎉 Android ${TARGET^^} build complete!"
    echo "   Generated APK : ${APK_OUT}"
    echo "   Size          : $(du -h "${APK_SRC}" | cut -f1)"
    echo ""
    if adb get-state 1>/dev/null 2>&1; then
        cyan "📱 Deploying to connected device..."
        adb push "${APK_SRC}" "/data/local/tmp/sow-client.apk" >/dev/null
        adb shell pm install -r -d "/data/local/tmp/sow-client.apk" || {
            yellow "⚠️ Normal installation failed. Attempting clean uninstall & reinstall..."
            adb uninstall rust.sow_client || true
            adb shell pm install "/data/local/tmp/sow-client.apk" || fail "Failed to install APK via ADB"
        }
        adb shell rm "/data/local/tmp/sow-client.apk"
        cyan "🚀 Launching application..."
        if [[ "${TARGET}" == "native" ]]; then
            adb shell monkey -p rust.sow_client -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
        else
            adb shell monkey -p rust.sow_client -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
        fi
        green "✅ Game started on device!"
    else
        echo -e "\033[1;33m⚠️  No ADB device detected. Skipping automatic deployment.\033[0m"
    fi
else
    fail "APK not found at ${APK_SRC}"
fi
