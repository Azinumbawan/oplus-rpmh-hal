#!/bin/bash
# Build the Rust RPMH HAL for aarch64 Android, then verify the artifact.
# ANDROID_NDK_HOME is the NDK root; LIBBINDER_NDK is the platform library to
# link against. Both are documented in README.md.
set -euo pipefail

: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the Android NDK root}"
: "${LIBBINDER_NDK:?set LIBBINDER_NDK to the platform libbinder_ndk.so}"

W=$(cd "$(dirname "$0")/.." && pwd)
cd "$W/rust"

NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
[[ -x "$NDK_BIN/aarch64-linux-android35-clang" ]] || {
    echo "missing aarch64-linux-android35-clang under $NDK_BIN" >&2
    exit 1
}
export PATH="$NDK_BIN:$PATH"

echo "=== host unit tests (parsers, adapters) ==="
cargo test --offline
echo

echo "=== cross-build aarch64-linux-android (release) ==="
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="\
-C link-arg=-Wl,--no-undefined \
-C link-arg=-Wl,-z,relro,-z,now \
-C link-arg=-Wl,--build-id=none \
-C link-arg=-llog \
-C link-arg=$LIBBINDER_NDK"
cargo build --release --offline --target aarch64-linux-android --bins

OUT=target/aarch64-linux-android/release
SRC="$OUT/rpmh_service"
DST="$OUT/vendor.oplus.hardware.rpmh-V1-service"
[[ -f "$SRC" ]] || { echo "build produced no binary" >&2; exit 1; }
install -m 0755 "$SRC" "$DST"

echo
echo "=== built ==="
ls -la "$DST"
"$NDK_BIN/llvm-readelf" -d "$DST" | grep NEEDED
"$NDK_BIN/llvm-readelf" -p .comment "$DST" | head -5
echo BUILD_OK
