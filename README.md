# OplusRpmh Sleep Statistics

A conservative Rust implementation of the Oplus subsystem sleep-statistics
interface `vendor.oplus.hardware.rpmh.IRpmh/default`. This is not Qualcomm
hardware RPMh resource voting/control and does not require Qualcomm RPMh drivers.

## Contract

- Interface version **1**, hash `acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05`.
- Transactions 1/2 query one state/the state list; transaction 3 is dump type.
- Replies are **status, return Result, then out state/list**, matching the
  checked donor Proxy. Failed queries still serialize the required default
  state or empty list, but with an explicit non-OK Result.
- Parcelable order is `name`, `version`, `residencyInMsecSinceBoot`,
  `totalTransitions`, `lastEntryTimestampMs`, `supportedOnlyInSuspend`.
  The non-null marker and bounded size header precede each state, including
  list elements; the four numeric state fields are signed 64-bit and bool is
  encoded as int32.
- Unknown transactions return NDK `STATUS_UNKNOWN_TRANSACTION` (`-74`).
  Null non-nullable names, malformed/oversized strings and trailing data are
  rejected before telemetry access. UTF-16 transport conversion is performed
  by libbinder_ndk, not a raw UTF-8 parcel implementation.
- AIDL types and the created binder are explicitly VINTF-stable. This remains
  manual transport, not a regenerated frozen API; original annotation recovery
  from Java type descriptors alone is not claimed.

## Accurate Data Policy

**No cumulative backend is enabled by default.** On the inspected CK8n,
`/proc/mtk_lpm/suspend/spm_sleep_count` is 5 while
`/sys/power/suspend_stats/success` is 6. These are not interchangeable counters.
`/sys/kernel/wakeup_reasons/last_suspend_time` reports awake suspend/resume
overhead followed by last sleep duration, not start/end times. Its second field
is not cumulative since boot. Neither source is used to invent residency.

The reviewed native-source table in `core::verified_source` is empty until a
real driver contract is established. Queries therefore return `UN_SUPPORTED`,
including the empty list; no MPSS/VMIN/connectivity data is synthesized.
There is no since-daemon-start accumulator and no claim to have observed every
suspend from boot count zero.

A future reviewed `CounterSource` must identify exact read-only native paths for
transition count, **complete cumulative since-boot** residency, and last-entry
timestamp on the **CLOCK_BOOTTIME** epoch. It must document each native unit,
version/suspend-only meaning and whether counters can reset. Caller-selected
paths and guessed tick frequencies are not supported. This backend:

- Parses whole unsigned decimal or hexadecimal integers; negative, malformed,
  nonfinite and overflowing data is not converted to zero.
- Converts explicitly declared ns/us/ms/seconds/ticks using checked integer
  arithmetic, truncating fractional milliseconds. Tick frequency must be nonzero.
- Reads count before/after a snapshot, retries at most twice on updates, and
  refuses incoherent data instead of mixing epochs.
- Checks residency and entry timestamp against real BOOTTIME, count-zero
  consistency and signed AIDL field limits.
- Detects regressions in raw counters and boot clock; a reset/wrap latches
  `INVALID_OPERATION`. No guessed rollover extension or new baseline is reported
  as a complete boot total. A driver reset while the daemon was absent cannot be
  detected from one snapshot; only an independently verified monotonic-from-boot
  driver contract can establish completeness.
- Distinguishes missing/unverified data (`UN_SUPPORTED`), invalid data
  (`BAD_VALUE`), reset/incoherence (`INVALID_OPERATION`) and caller denial.

Count bracketing is valid only with a verified driver publication protocol that
makes both values coherent with that count. It cannot make independently updated
nodes atomic; an appropriate native snapshot/sequence facility may be required.

Only Binder UIDs 0 and 1000 may obtain telemetry. SELinux/DAC access to actual
native counters and the correctness of a future profile require platform review.
Missing Oplus paths normally call for verified userspace telemetry mapping; kernel
work is justified only if the required real counters are genuinely unavailable.

## No Crash Or Reset Dump

`triggerDumpType` always returns `UN_SUPPORTED`, for every caller/type/property
combination. ADSP restart, SCP watchdog reset and sysrq panic implementations have
been removed, not merely hidden behind a property gate. No node writer exists.
This service does not provide a supposedly safe diagnostic dump by triggering a
kernel/subsystem reset.

## Host Tests

From `rust/`, with already installed stable Rust 1.95 or newer:

```powershell
rustup run stable cargo test --offline --lib --target-dir "C:\Users\Azareel\AppData\Local\Temp\kilo\oplus-rpmh-host-tests"
```

Node/property/clock fixtures are thread-local in memory. Tests cover unsupported
CK8n inputs, complete native snapshots, unit/range/reset behavior and the same
reply encoder used by production. They do not run HALs/Binder, open host/device
nodes, create Unix symlinks or perform crash/reset writes. Android Binder
integration and actual hardware telemetry remain separately untested.

## Android Build And Deployment

An existing Android Rust target/NDK compiler and actual target-platform
`libbinder_ndk.so` are required. `build.rs` accepts `LIBBINDER_NDK` (full path) or
`OPLUS_PLATFORM_LIB_DIR` (platform lib64 directory) and links the exact library,
not the NDK public stub. API35 is the default cross-link target; runtime manager,
process, VINTF and parcel-size APIs require the real platform library (API31+).
Windows callers may set `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` to their
installed NDK clang `.cmd`. No toolchain/target is automatically installed.

`scripts/build_rust.sh` uses `ANDROID_NDK_HOME`, `LIBBINDER_NDK` and optional
`NDK_HOST_TAG`/`RUST_TOOLCHAIN`/`CARGO_TARGET_DIR`, runs host library tests and
cross-builds `vendor.oplus.hardware.rpmh-V1-service`; it never executes it.
The staging helper requires `ROM_TREE` and an explicit reviewed
`RPMH_RELATIVE_EXECUTABLE` under `vendor/bin/hw/` or `odm/bin/hw/`.
`RPMH_ARTIFACT` can select an external artifact. No rc/manifest/policy/image is
generated or modified. Historical upstream prebuilts do not contain these fixes.
