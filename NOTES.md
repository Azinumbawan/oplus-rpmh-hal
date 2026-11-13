# OplusRpmh — pure-Rust OPlus RPMH HAL

`vendor.oplus.hardware.rpmh.IRpmh` (V1) for the X6739 (MT6893). Reconstructed
from the shipped OPlus artifacts, then rewritten in Rust with no C or C++
code and no prebuilt library.

## Contract

- Descriptor: `vendor.oplus.hardware.rpmh.IRpmh`
- Instance: `vendor.oplus.hardware.rpmh.IRpmh/default`
- Interface version: 1
- Interface hash: `acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05`
- Three methods (transaction codes 1..3, `aidl/vendor/oplus/hardware/rpmh/`):
  1. `getPowerStateSubsystemSleepState(String, out PowerStateSubsystemSleepState) -> Result`
  2. `getPowerStateSubsystemSleepStateList(out List<PowerStateSubsystemSleepState>) -> Result`
  3. `triggerDumpType(int) -> int`
  plus the reserved pair `getInterfaceVersion` (16777215) / `getInterfaceHash`
  (16777214).
- `Result` is `@Backing(type="int")`: UN_SUPPORTED -1, PERMISSION_DENIED -2,
  BAD_VALUE -3, INVALID_OPERATION -4, OK 0 (the width is load-bearing; AIDL
  defaults to byte).
- Parcelable layout (versioned format `[int32 1][int32 size][fields]`):
  String name, long version, long residencyInMsecSinceBoot, long
  totalTransitions, long lastEntryTimestampMs, boolean
  supportedOnlyInSuspend. Only name, transitions and residency are ever
  non-zero; the rest keep their parcelable defaults, as the shipped service
  wrote them.
- A caller at or above uid 1002 is refused with PERMISSION_DENIED before any
  work (verified against the shipped binary's `if (uid < 0x3ea)` branch).
  Calling uid comes from `AIBinder_getCallingUid`, exported by the ROM tree's
  `libbinder_ndk.so`.
- The reply always carries the out-parameter (default-constructed when the
  request failed) followed by the Result; the binder status is always OK.

## X6739 node adaptation

The shipped service picked a platform parser from `ro.hardware` and scraped
that platform's kernel statistics file. Qualcomm `rpmh_stats`, the OPlus
`/data/oplus/system_stats/stats` dump, the MT6985 `/proc/mtk_lpm/spm/
system_stat` node and the MT6985 parser's `/dev/conn_dbg_dev` reads all fail
on this board (audited on the device: `/proc/mtk_lpm/spm/` carries only debug
flags, `conn_dbg_dev` is 0600 root and SELinux-blocked for the HAL's system
uid).

This port's `X6739Parser` therefore feeds APSS from the closest source that
exists and publishes what the framework expects:

| Field | Source |
|---|---|
| `totalTransitions` | `/proc/mtk_lpm/suspend/spm_sleep_count` (fallback `/sys/power/suspend_stats/success`) |
| `residencyInMsecSinceBoot` | last suspend duration from `/sys/kernel/wakeup_reasons/last_suspend_time` ("start end" seconds, delta x 1000) |

The board publishes no cumulative APSS residency, so the last suspend's
duration stands in for it — the same shape, not the same accumulation.
MPSS, VMIN and the connectivity subsystems have no readable source on
MT6893 and yield nothing, exactly as the shipped MediaTek parser did when
its source was missing; the framework computes over whatever list it
receives.

`triggerDumpType` keeps the shipped prop gate
(`sys.oplus.rpmh.trigger_dump` or `persist.vendor.oplus.rpmh.trigger_dump`)
and its three targets: the ADSP node (absent here), the MediaTek SCP
watchdog (`/sys/class/misc/scp/wdt_reset`, present but SELinux-blocked), and
`/proc/sysrq-trigger`. The result is 0 after the attempt regardless of
whether the write itself succeeded, as the shipped service did.

## Build and stage

```bash
scripts/build_rust.sh    # host unit tests + aarch64 cross-build
scripts/stage_vendor.sh  # copies the binary into erofs/vendor/bin/hw/
```

The crate pins rustc via `rust/rust-toolchain.toml` (1.99.1). The link uses
the ROM tree's real `libbinder_ndk.so` for the platform-only
`AServiceManager_addService` / `ABinderProcess_*` entry points (see
`rust/.cargo/config.toml`).

## Layout

- `aidl/` — the interface contract (method order and the Result backing type
  are load-bearing).
- `rust/src/core.rs` — the platform parsers, the X6739 APSS adapter and the
  trigger-dump paths, host-testable.
- `rust/src/wire.rs` — the hand-rolled `libbinder_ndk` C API binding and the
  versioned-parcelable helpers.
- `rust/src/hal.rs` — the `onTransact` dispatch, the uid gate.
- `rust/src/bin/rpmh_service.rs` — registration and the thread pool.

The C++ reconstruction the port replaced is preserved in the OOS root's
history (under `OplusPowerRpmhOrms/`), alongside the powermonitor service
that shared that tree.
