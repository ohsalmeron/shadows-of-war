#!/usr/bin/env bash
# Builds the Rust static library for the active Xcode SDK (device or simulator),
# then lipo's per-arch artifacts for the Xcode linker.
# Open: shadows-of-war/sow-dist/deploy/ios/sow_ios.xcodeproj

set -euo pipefail

PATH="$PATH:$HOME/.cargo/bin"

PROFILE=debug
RELFLAG=
if [[ "$CONFIGURATION" != "Debug" ]]; then
    PROFILE=release
    RELFLAG=--release
fi

export PATH="$PATH:/opt/homebrew/bin"

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
export CARGO_TARGET_DIR="${SOW_IOS_CARGO_TARGET_DIR:-$REPO_ROOT/dist/ios/cargo-target}"
export CARGO_HOME="${SOW_IOS_CARGO_HOME:-${CARGO_HOME:-$HOME/.cargo}}"

# Avoid Rust + Xcode toolchain `ld: library 'System' not found` (rust#80817).
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
  IS_SIMULATOR=1
fi

LIBRARIES=
for arch in $ARCHS; do
  case "$arch" in
    x86_64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        echo "Building for x86_64, but not a simulator build. What's going on?" >&2
        exit 2
      fi
      export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
      TARGET=x86_64-apple-ios
      ;;

    arm64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        TARGET=aarch64-apple-ios
      else
        TARGET=aarch64-apple-ios-sim
      fi
      ;;

    *)
      echo "Unsupported Xcode architecture: $arch" >&2
      exit 2
      ;;
  esac

  cargo rustc --crate-type staticlib --locked $RELFLAG --target "$TARGET" \
    -p sow-client --lib --manifest-path "$REPO_ROOT/Cargo.toml"

  LIBRARIES="$LIBRARIES $CARGO_TARGET_DIR/$TARGET/$PROFILE/libsow_client.a"
done

lipo -create -output "$DERIVED_FILE_DIR/libsow_client.a" $LIBRARIES
