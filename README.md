# Oplus RPMh HAL

Rust compatibility implementation of `vendor.oplus.hardware.rpmh.IRpmh/default`, based on the reverse-engineered Oplus HAL in [Xia-s-Projekt/oplus-rpmh-hal](https://github.com/Xia-s-Projekt/oplus-rpmh-hal).

The interface is V1, hash `acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05`. It reports subsystem sleep statistics; it does not implement Qualcomm RPMh resource voting or supply missing hardware drivers. This is independent software, not official Oplus source.

## Status

**No hardware statistics backend is enabled by default.** Requests return `UN_SUPPORTED` until a complete native counter source has been verified. An unsupported query is not reported as a successful empty result.

A backend must provide cumulative residency since boot, transition counts and a last-entry timestamp with documented units and clock epoch. A last-suspend duration or a counter accumulated only since daemon startup is not an equivalent replacement.

The native-counter adapter validates bounds, units, clock ranges, counter regression and snapshot consistency. It does not synthesize missing subsystem data. Crash, watchdog-reset and subsystem-restart dump operations are disabled.

## Interface

Replies preserve AIDL ordering: status, returned `Result`, then OUT state or list. Sleep-state fields are `name`, `version`, `residencyInMsecSinceBoot`, `totalTransitions`, `lastEntryTimestampMs` and `supportedOnlyInSuspend`.

The service explicitly marks its Binder VINTF-stable and rejects malformed, oversized or trailing input. Root or system Binder identity is required for telemetry queries. Actual SELinux and linker integration remains device-specific.

## Build

Requirements: installed Rust 1.95 or newer, the Android ARM64 target, Android NDK, and the target platform's actual Binder library. The default cross-build API is 35.

```sh
export ANDROID_NDK_HOME=/path/to/android-ndk
export LIBBINDER_NDK=/path/to/platform/lib64/libbinder_ndk.so
bash scripts/build_rust.sh
```

`OPLUS_PLATFORM_LIB_DIR` may supply the platform library directory instead. Building does not run the HAL or install anything on a device.

## Tests

```sh
cargo +stable test --manifest-path rust/Cargo.toml --lib --locked
cargo +stable clippy --manifest-path rust/Cargo.toml --lib -- -D warnings
```

Tests use in-memory telemetry and clock fixtures. They cover reply ordering, invalid data, unit conversion and resets, but not live Binder or hardware behavior.

## Integration

Provide the appropriate init service, V1 VINTF declaration, service labels and read-only counter permissions. Keep reset and crash controls unavailable. The optional staging helper copies only to an explicitly selected, reviewed ROM-tree destination; it does not supply policy, repack images or flash a device.

## Attribution

Initial reverse-engineering and implementation are by Xia-s-Projekt and its contributors. This fork corrects protocol handling and limits reporting to verified telemetry. Existing copyright and license notices remain applicable.
