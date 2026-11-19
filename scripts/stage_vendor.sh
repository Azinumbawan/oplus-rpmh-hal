#!/bin/bash
# Stage the built HAL binary into an unpacked ROM tree.
#
# Staged path (relative to ROM_TREE):
#   vendor/bin/hw/vendor.oplus.hardware.rpmh-V1-service
#
# The init rc (vendor/etc/init/vendor.oplus.hardware.rpmh-V1-service.rc) and
# the VINTF fragment (vendor/etc/vintf/manifest/manifest_rpmh_aidl.xml) that
# reference this path are supplied by the target platform and are not touched.
#
# This script only installs the binary into the unpacked tree. It does not
# repack an image, flash a device, or modify fs_config / file_contexts.
set -euo pipefail

ROM_TREE=${ROM_TREE:-}
if [[ -z "$ROM_TREE" ]]; then
    echo "set ROM_TREE to the root of the unpacked ROM tree" >&2
    exit 1
fi

W=$(cd "$(dirname "$0")/.." && pwd)
SRC="$W/rust/target/aarch64-linux-android/release/vendor.oplus.hardware.rpmh-V1-service"

GRN=$'\033[32m'; RED=$'\033[31m'; NC=$'\033[0m'

[[ -f "$SRC" ]] || { echo -e "${RED}missing $SRC: run scripts/build_rust.sh first${NC}"; exit 1; }

live="$ROM_TREE/vendor/bin/hw/vendor.oplus.hardware.rpmh-V1-service"
install -m 0755 "$SRC" "$live"
echo -e "${GRN}staged${NC} $live"
echo "done"
