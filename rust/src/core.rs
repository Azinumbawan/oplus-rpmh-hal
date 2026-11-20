// Only complete native since-boot counters may back IRpmh residency.
// A last-suspend duration or a daemon-start accumulator is not such a counter.

use crate::io_shim;
use std::sync::Mutex;

pub const RESULT_UN_SUPPORTED: i32 = -1;
pub const RESULT_PERMISSION_DENIED: i32 = -2;
pub const RESULT_BAD_VALUE: i32 = -3;
pub const RESULT_INVALID_OPERATION: i32 = -4;
pub const RESULT_OK: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    PermissionDenied,
    BadValue,
    InvalidOperation,
}
impl Error {
    pub fn result(self) -> i32 {
        match self {
            Self::Unsupported => RESULT_UN_SUPPORTED,
            Self::PermissionDenied => RESULT_PERMISSION_DENIED,
            Self::BadValue => RESULT_BAD_VALUE,
            Self::InvalidOperation => RESULT_INVALID_OPERATION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PowerStateSubsystemSleepState {
    pub name: String,
    pub version: i64,
    pub residency_in_msec_since_boot: i64,
    pub total_transitions: i64,
    pub last_entry_timestamp_ms: i64,
    pub supported_only_in_suspend: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Unit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Ticks(u64),
}
impl Unit {
    pub fn milliseconds(self, value: u64) -> Result<i64, Error> {
        let value = u128::from(value);
        let converted = match self {
            Self::Nanoseconds => value / 1_000_000,
            Self::Microseconds => value / 1000,
            Self::Milliseconds => value,
            Self::Seconds => value * 1000,
            Self::Ticks(0) => return Err(Error::BadValue),
            Self::Ticks(frequency) => value * 1000 / u128::from(frequency),
        };
        i64::try_from(converted).map_err(|_| Error::BadValue)
    }
}

pub fn unsigned_counter(text: &str) -> Result<u64, Error> {
    let text = text.trim();
    let (digits, radix) = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(digits) => (digits, 16),
        None => (text, 10),
    };
    if digits.is_empty()
        || !digits.bytes().all(|b| {
            if radix == 16 {
                b.is_ascii_hexdigit()
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return Err(Error::BadValue);
    }
    u64::from_str_radix(digits, radix).map_err(|_| Error::BadValue)
}

// A profile is a reviewed driver contract, never a caller-selected path.
// All three counters must be monotonic from this boot; last_entry is BOOTTIME.
// The driver's count publication must also bracket coherent updates of both
// values; rereading independent counters cannot create an atomic snapshot.
#[derive(Clone, Copy)]
pub struct CounterSource {
    pub count_path: &'static str,
    pub residency_path: &'static str,
    pub last_entry_path: &'static str,
    pub residency_unit: Unit,
    pub timestamp_unit: Unit,
    pub version: i64,
    pub supported_only_in_suspend: bool,
    pub complete_since_boot: bool,
}

pub fn verified_source(_hardware: &str) -> Option<CounterSource> {
    // CK8n exposes a count and last_suspend_time, but no verified cumulative
    // residency/last-entry source. Do not substitute suspend_stats/success.
    None
}

#[derive(Clone, Copy)]
struct Sample {
    count: i64,
    residency: i64,
    entry: i64,
    boot: i64,
    raw_residency: u64,
    raw_entry: u64,
}
#[derive(Default)]
struct History {
    previous: Option<Sample>,
    reset: bool,
}

pub struct Statistics {
    source: Option<CounterSource>,
    history: Mutex<History>,
}
impl Statistics {
    pub fn new(source: Option<CounterSource>) -> Self {
        Self {
            source,
            history: Mutex::new(History::default()),
        }
    }

    pub fn get(&self, uid: u32, name: &str) -> Result<PowerStateSubsystemSleepState, Error> {
        if !matches!(uid, 0 | 1000) {
            return Err(Error::PermissionDenied);
        }
        if name.len() > 32 || name.chars().any(char::is_control) {
            return Err(Error::BadValue);
        }
        if name != "APSS" {
            return Err(Error::Unsupported);
        }
        let source = self
            .source
            .filter(|s| s.complete_since_boot && s.version >= 0)
            .ok_or(Error::Unsupported)?;
        let mut history = self.history.lock().map_err(|_| Error::InvalidOperation)?;
        if history.reset {
            return Err(Error::InvalidOperation);
        }
        let read = |path| {
            io_shim::read_file(path, 128)
                .map_err(|e| match e {
                    io_shim::ReadError::Unavailable => Error::Unsupported,
                    io_shim::ReadError::InvalidData => Error::BadValue,
                })
                .and_then(|s| unsigned_counter(&s))
        };
        let mut snapshot = None;
        // A counter update during reads is retried, never combined with an old duration.
        for _ in 0..2 {
            let before = read(source.count_path)?;
            let raw_residency = read(source.residency_path)?;
            let raw_entry = read(source.last_entry_path)?;
            let residency = source.residency_unit.milliseconds(raw_residency)?;
            let entry = source.timestamp_unit.milliseconds(raw_entry)?;
            let after = read(source.count_path)?;
            if after < before {
                history.reset = true;
                return Err(Error::InvalidOperation);
            }
            if before != after {
                continue;
            }
            let count = i64::try_from(after).map_err(|_| Error::BadValue)?;
            let boot = io_shim::boot_time_ms()
                .filter(|t| *t >= 0)
                .ok_or(Error::Unsupported)?;
            if residency > boot
                || entry > boot
                || (count == 0 && (raw_residency != 0 || raw_entry != 0))
                || (count > 0 && raw_entry == 0)
            {
                return Err(Error::BadValue);
            }
            snapshot = Some(Sample {
                count,
                residency,
                entry,
                boot,
                raw_residency,
                raw_entry,
            });
            break;
        }
        let current = snapshot.ok_or(Error::InvalidOperation)?;
        if let Some(previous) = history.previous {
            if current.count < previous.count
                || current.raw_residency < previous.raw_residency
                || current.raw_entry < previous.raw_entry
                || current.boot < previous.boot
            {
                // Unknown counter width/epoch: guessing wrap extension would fabricate totals.
                history.reset = true;
                return Err(Error::InvalidOperation);
            }
            if current.count == previous.count && current.raw_entry != previous.raw_entry {
                return Err(Error::InvalidOperation);
            }
        }
        history.previous = Some(current);
        Ok(PowerStateSubsystemSleepState {
            name: "APSS".to_owned(),
            version: source.version,
            residency_in_msec_since_boot: current.residency,
            total_transitions: current.count,
            last_entry_timestamp_ms: current.entry,
            supported_only_in_suspend: source.supported_only_in_suspend,
        })
    }

    pub fn list(&self, uid: u32) -> Result<Vec<PowerStateSubsystemSleepState>, Error> {
        self.get(uid, "APSS").map(|s| vec![s])
    }
}

// There is intentionally no crash/reset implementation or property opt-in.
pub fn trigger_dump_type(_kind: i32) -> i32 {
    RESULT_UN_SUPPORTED
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> CounterSource {
        CounterSource {
            count_path: "/mock/count",
            residency_path: "/mock/residency",
            last_entry_path: "/mock/entry",
            residency_unit: Unit::Microseconds,
            timestamp_unit: Unit::Milliseconds,
            version: 1,
            supported_only_in_suspend: true,
            complete_since_boot: true,
        }
    }
    fn sample(count: &str, residency: &str, entry: &str, boot: i64) {
        io_shim::mock_file("/mock/count", count);
        io_shim::mock_file("/mock/residency", residency);
        io_shim::mock_file("/mock/entry", entry);
        io_shim::mock_clock(boot);
    }

    #[test]
    fn ck8n_count_and_last_sleep_do_not_become_boot_totals() {
        io_shim::mock_file("/proc/mtk_lpm/suspend/spm_sleep_count", "5");
        io_shim::mock_file("/sys/power/suspend_stats/success", "6");
        io_shim::mock_file(
            "/sys/kernel/wakeup_reasons/last_suspend_time",
            "0.118588462 0.254318922",
        );
        let stats = Statistics::new(verified_source("mt6893"));
        assert_eq!(stats.get(1000, "APSS"), Err(Error::Unsupported));
        assert_eq!(stats.list(1000), Err(Error::Unsupported));
        assert!(io_shim::mock_reads().is_empty());
    }

    #[test]
    fn complete_native_counters_convert_units_without_accumulating() {
        sample("0x5", "254318", "1000", 5000);
        let stats = Statistics::new(Some(source()));
        let first = stats.get(1000, "APSS").unwrap();
        assert_eq!(
            (
                first.total_transitions,
                first.residency_in_msec_since_boot,
                first.last_entry_timestamp_ms
            ),
            (5, 254, 1000)
        );
        sample("6", "300000", "1500", 6000);
        assert_eq!(
            stats
                .get(1000, "APSS")
                .unwrap()
                .residency_in_msec_since_boot,
            300
        );
        assert_eq!(
            stats
                .get(1000, "APSS")
                .unwrap()
                .residency_in_msec_since_boot,
            300
        );
    }

    #[test]
    fn rejects_partial_sources_bad_timestamps_and_incomplete_epoch() {
        let stats = Statistics::new(Some(source()));
        assert_eq!(stats.get(1000, "APSS"), Err(Error::Unsupported));
        sample("5", "254318", "6001", 6000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::BadValue));
        sample("0", "1", "1", 6000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::BadValue));
        let mut incomplete = source();
        incomplete.complete_since_boot = false;
        assert_eq!(
            Statistics::new(Some(incomplete)).get(1000, "APSS"),
            Err(Error::Unsupported)
        );
    }

    #[test]
    fn counter_reset_or_wrap_invalidates_future_snapshots() {
        sample("5", "254318", "1000", 5000);
        let stats = Statistics::new(Some(source()));
        assert!(stats.get(1000, "APSS").is_ok());
        sample("1", "1000", "2000", 6000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
        sample("10", "500000", "3000", 7000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
    }

    #[test]
    fn raw_submillisecond_regressions_and_zero_count_are_checked() {
        sample("5", "254999", "1000", 5000);
        let stats = Statistics::new(Some(source()));
        assert!(stats.get(1000, "APSS").is_ok());
        sample("5", "254998", "1000", 6000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
        sample("0", "1", "0", 6000);
        assert_eq!(
            Statistics::new(Some(source())).get(1000, "APSS"),
            Err(Error::BadValue)
        );
    }

    #[test]
    fn count_bracketing_retries_updates_but_rejects_resets_and_churn() {
        sample("6", "300000", "1500", 5000);
        io_shim::mock_sequence("/mock/count", &["5", "6", "6", "6"]);
        let stats = Statistics::new(Some(source()));
        assert_eq!(stats.get(1000, "APSS").unwrap().total_transitions, 6);
        io_shim::mock_sequence("/mock/count", &["6", "7", "8", "9"]);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
        io_shim::mock_sequence("/mock/count", &["9", "8"]);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
        sample("10", "400000", "2000", 6000);
        assert_eq!(stats.get(1000, "APSS"), Err(Error::InvalidOperation));
    }

    #[test]
    fn negative_overflow_and_nonnumeric_values_are_not_zero() {
        for text in [
            "-1",
            "+1",
            "NaN",
            "inf",
            "5 extra",
            "18446744073709551616",
            "0x",
        ] {
            assert_eq!(unsigned_counter(text), Err(Error::BadValue));
        }
        assert_eq!(
            Unit::Milliseconds.milliseconds(u64::MAX),
            Err(Error::BadValue)
        );
        assert_eq!(Unit::Ticks(0).milliseconds(1), Err(Error::BadValue));
        assert_eq!(Unit::Ticks(32768).milliseconds(32768), Ok(1000));
        sample("9223372036854775808", "0", "0", 5000);
        assert_eq!(
            Statistics::new(Some(source())).get(1000, "APSS"),
            Err(Error::BadValue)
        );
        sample("NaN", "0", "0", 5000);
        assert_eq!(
            Statistics::new(Some(source())).get(1000, "APSS"),
            Err(Error::BadValue)
        );
        sample("\0", "0", "0", 5000);
        assert_eq!(
            Statistics::new(Some(source())).get(1000, "APSS"),
            Err(Error::BadValue)
        );
        sample(&"9".repeat(129), "0", "0", 5000);
        assert_eq!(
            Statistics::new(Some(source())).get(1000, "APSS"),
            Err(Error::BadValue)
        );
    }

    #[test]
    fn caller_and_name_checks_precede_reads_and_dump_is_always_disabled() {
        let stats = Statistics::new(Some(source()));
        assert_eq!(stats.get(10001, "APSS"), Err(Error::PermissionDenied));
        assert_eq!(stats.get(1000, "MPSS"), Err(Error::Unsupported));
        assert_eq!(stats.get(u32::MAX, "APSS"), Err(Error::PermissionDenied));
        io_shim::mock_property("sys.oplus.rpmh.trigger_dump", "true");
        io_shim::mock_property("persist.vendor.oplus.rpmh.trigger_dump", "true");
        for kind in [-1, 0, 1, 2, 9, i32::MAX] {
            assert_eq!(trigger_dump_type(kind), RESULT_UN_SUPPORTED);
        }
        assert!(io_shim::mock_reads().is_empty());
    }
}
