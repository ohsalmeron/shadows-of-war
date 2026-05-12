#!/usr/bin/env bash
# Builds the Rust binary for the active Xcode SDK (device or simulator), then
# `lipo`s per-arch artifacts into the path Xcode expects as the app executable.
# Open: shadows-of-war/ios/sow_ios.xcodeproj

set -eux

PATH=$PATH:$HOME/.cargo/bin

PROFILE=debug
RELFLAG=
if [[ "$CONFIGURATION" != "Debug" ]]; then
    PROFILE=release
    RELFLAG=--release
fi

set -euvx

export PATH="$PATH:/opt/homebrew/bin"

export CARGO_TARGET_DIR="$DERIVED_FILE_DIR/cargo"

# Avoid Rust + Xcode toolchain `ld: library 'System' not found` (rust#80817).
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
  IS_SIMULATOR=1
fi

EXECUTABLES=
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
  esac

  REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
  cargo build $RELFLAG --target "$TARGET" --bin sow-client --manifest-path "$REPO_ROOT/Cargo.toml"

  EXECUTABLES="$EXECUTABLES $DERIVED_FILE_DIR/cargo/$TARGET/$PROFILE/sow-client"
done

lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" $EXECUTABLES
