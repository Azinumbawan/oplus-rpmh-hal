#!/bin/bash
# Host memory-fixture tests and cross-build only; no HAL execution.
set -euo pipefail
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to an existing NDK}"
: "${LIBBINDER_NDK:?set LIBBINDER_NDK to the real target-platform library}"
W=$(cd "$(dirname "$0")/.." && pwd)
cd "$W/rust"
NDK="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/${NDK_HOST_TAG:-linux-x86_64}"
export PATH="$NDK/bin:$PATH"
TOOLCHAIN=${RUST_TOOLCHAIN:-stable}
rustup run "$TOOLCHAIN" cargo test --offline --lib
rustup run "$TOOLCHAIN" cargo build --release --offline --target aarch64-linux-android --bin rpmh_service
OUT="${CARGO_TARGET_DIR:-target}/aarch64-linux-android/release"
SRC="$OUT/rpmh_service"
DST="$OUT/vendor.oplus.hardware.rpmh-V1-service"
[[ -f "$SRC" ]] || { echo "build produced no service" >&2; exit 1; }
install -m 0755 "$SRC" "$DST"
"$NDK/bin/llvm-readelf" -d "$DST"
echo "Built IRpmh V1; no cumulative backend is enabled without a verified driver profile."
