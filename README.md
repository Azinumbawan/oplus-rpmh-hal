# OplusRpmh

A pure-Rust implementation of the OPlus RPMH AIDL HAL
(`vendor.oplus.hardware.rpmh.IRpmh` V1).

## What it does

The service reports subsystem sleep-state information over AIDL. It registers
`vendor.oplus.hardware.rpmh.IRpmh/default`, answers per-subsystem queries with
the subsystem name, transition count and accumulated residency read from the
platform's kernel statistics, and provides a trigger-dump method. The binder
transport is implemented directly against the stable `libbinder_ndk` C API, so
the crate contains no C or C++ sources and no generated backend library.

## Layout

- `aidl/` — the interface, parcelable and result-enum definitions.
- `rust/` — the crate: platform parser core, host-testable I/O shim, wire
  helpers, transaction dispatch and the service entry point.
- `scripts/` — build and staging helpers.

## Requirements

- The Rust toolchain pinned in `rust/rust-toolchain.toml`.
- An Android NDK providing `aarch64-linux-android35-clang`. Set
  `ANDROID_NDK_HOME` to the NDK root.
- A platform `libbinder_ndk.so` exporting `AServiceManager_addService` and the
  `ABinderProcess_*` entry points. Set `LIBBINDER_NDK` to its path.

## Prebuilt

[Get Prebuilt](https://github.com/Xia-s-Projekt/oplus-rpmh-hal/releases/latest) — AArch64 ODM archive containing the service under `odm/bin/hw/`, its init rc under `odm/etc/init/`, and the VINTF manifest under `odm/etc/vintf/manifest/`.

## Build

```bash
export ANDROID_NDK_HOME=/path/to/android-ndk
export LIBBINDER_NDK=/path/to/libbinder_ndk.so
scripts/build_rust.sh
```

The script runs the host unit tests, then cross-builds the release binary to
`rust/target/aarch64-linux-android/release/vendor.oplus.hardware.rpmh-V1-service`.

## Staging

```bash
ROM_TREE=/path/to/unpacked/rom scripts/stage_vendor.sh
```

`ROM_TREE` is the root of an unpacked ROM tree. The script installs the binary
at `vendor/bin/hw/vendor.oplus.hardware.rpmh-V1-service`; the init rc and VINTF
manifest that reference that path are supplied by the target platform.
