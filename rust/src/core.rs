// The Rpmh HAL core: platform parsers, subsystem sleep-state adapters and
// trigger-dump paths.
//
// A parser is selected from ro.hardware and reads the corresponding kernel
// statistics nodes. Subsystems with no readable source are reported as
// UN_SUPPORTED.

use crate::io_shim;

/// vendor.oplus.hardware.rpmh.Result
pub const RESULT_UN_SUPPORTED: i32 = -1;
pub const RESULT_PERMISSION_DENIED: i32 = -2;
pub const RESULT_OK: i32 = 0;

/// Callers at or above this uid are refused with PERMISSION_DENIED before any
/// work. system_server (uid 1000) is below the gate; vendor services at uid
/// 1002 and above are refused.
pub const MAX_ALLOWED_UID_EXCLUSIVE: i32 = 1002;

/// vendor.oplus.hardware.rpmh.PowerStateSubsystemSleepState. Only the fields
/// the parsers fill (name, transitions, residency) are ever non-zero;
/// version, lastEntryTimestampMs and supportedOnlyInSuspend stay at their
/// parcelable defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PowerStateSubsystemSleepState {
    pub name: String,
    pub version: i64,
    pub residency_in_msec_since_boot: i64,
    pub total_transitions: i64,
    pub last_entry_timestamp_ms: i64,
    pub supported_only_in_suspend: bool,
}

/// The subsystem names the platform parser declares and the framework polls
/// for. Keeping the set visible matters: the framework's suspend ratio
/// is computed over the subsystem list it receives.
pub const SUBSYSTEMS: [&str; 7] = ["APSS", "MPSS", "VMIN", "CONNINFRA", "WIFI", "BT", "GPS"];

/// Parses the sleep/residency statistics for one subsystem. Returns None
/// when no source yields data for it.
pub trait SubsystemStateParser {
    fn parse_subsystem_sleep_state(&self, name: &str) -> Option<PowerStateSubsystemSleepState>;
    fn support_connectivity_subsys(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Qualcomm parser (selected on Qualcomm platforms)
// ---------------------------------------------------------------------------

/// The Qualcomm parser reads /sys/power/rpmh_stats/master_stats for APSS and
/// /sys/power/soc_sleep/stats for the island subsystems.
pub struct QcomParser;

impl SubsystemStateParser for QcomParser {
    fn parse_subsystem_sleep_state(&self, name: &str) -> Option<PowerStateSubsystemSleepState> {
        let file = match name {
            "APSS" => "/sys/power/rpmh_stats/master_stats",
            "SLPI_ISLAND" | "ADSP_ISLAND" => "/sys/power/soc_sleep/stats",
            _ => return None,
        };
        parse_stats_text(name, file, &[])
    }

    fn support_connectivity_subsys(&self) -> bool {
        false
    }
}

fn parse_stats_text(
    name: &str,
    file: &str,
    keywords: &[&str],
) -> Option<PowerStateSubsystemSleepState> {
    let content = io_shim::read_file(file)?;
    for line in content.lines() {
        if !line.contains(name) && !keywords.iter().any(|k| !k.is_empty() && line.contains(k)) {
            continue;
        }
        return parse_stats_line(name, line);
    }
    None
}

/// "<name> <transitions> <residency seconds>" as the platform statistics
/// files lay it out, with residency converted to milliseconds
/// (strtod(value) * 1000).
fn parse_stats_line(name: &str, line: &str) -> Option<PowerStateSubsystemSleepState> {
    let mut parts = line.split_whitespace();
    let _first = parts.next()?;
    let transitions: i64 = parts.next()?.parse().ok()?;
    let residency_seconds: f64 = parts.next()?.parse().ok()?;
    Some(PowerStateSubsystemSleepState {
        name: name.to_string(),
        total_transitions: transitions,
        residency_in_msec_since_boot: (residency_seconds * 1000.0) as i64,
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// MT6985 parser (reference shape)
// ---------------------------------------------------------------------------

/// The MT6985 parser reads /proc/mtk_lpm/spm/system_stat and the connectivity
/// debug device.
pub struct Mtk6985Parser;

impl SubsystemStateParser for Mtk6985Parser {
    fn parse_subsystem_sleep_state(&self, name: &str) -> Option<PowerStateSubsystemSleepState> {
        match name {
            "APSS" | "MPSS" | "VMIN" => parse_stats_text(
                name,
                "/proc/mtk_lpm/spm/system_stat",
                &["Suspend AP", "Suspend 26M"],
            ),
            "CONNINFRA" | "WIFI" | "BT" | "GPS" => parse_stats_text(name, "/dev/conn_dbg_dev", &[]),
            _ => None,
        }
    }

    fn support_connectivity_subsys(&self) -> bool {
        true
    }
}

/// Generic MediaTek parser. When its connectivity statistics sources are not
/// readable, it yields no subsystem states.
pub struct MtkParser;

impl SubsystemStateParser for MtkParser {
    fn parse_subsystem_sleep_state(&self, _name: &str) -> Option<PowerStateSubsystemSleepState> {
        None
    }

    fn support_connectivity_subsys(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// MediaTek SPM parser
// ---------------------------------------------------------------------------

/// MediaTek SPM adapter. APSS is fed from the platform's SPM suspend counters:
///
///   totalTransitions        <- /proc/mtk_lpm/suspend/spm_sleep_count
///                              (fallback /sys/power/suspend_stats/success)
///   residencyInMsecSinceBoot<- the last suspend duration read out of
///                              /sys/kernel/wakeup_reasons/last_suspend_time
///                              ("start end" in seconds, delta * 1000)
///
/// Subsystems with no readable source are omitted from the reported list.
pub struct X6739Parser;

impl X6739Parser {
    pub fn new() -> Self {
        X6739Parser
    }

    /// "0.000000000 0.000000000" -> 0 ms; "12.5 14.5" -> 2000 ms.
    fn last_suspend_ms(&self) -> i64 {
        let text = match io_shim::read_file("/sys/kernel/wakeup_reasons/last_suspend_time") {
            Some(t) => t,
            None => return 0,
        };
        let mut parts = text.split_whitespace();
        let start: f64 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => return 0,
        };
        let end: f64 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => return 0,
        };
        ((end - start) * 1000.0) as i64
    }

    fn apss(&self) -> Option<PowerStateSubsystemSleepState> {
        let transitions = io_shim::read_file("/proc/mtk_lpm/suspend/spm_sleep_count")
            .and_then(|s| s.trim().parse::<i64>().ok())
            .or_else(|| {
                io_shim::read_file("/sys/power/suspend_stats/success")
                    .and_then(|s| s.trim().parse::<i64>().ok())
            })?;
        Some(PowerStateSubsystemSleepState {
            name: "APSS".to_string(),
            total_transitions: transitions,
            residency_in_msec_since_boot: self.last_suspend_ms(),
            ..Default::default()
        })
    }
}

impl SubsystemStateParser for X6739Parser {
    fn parse_subsystem_sleep_state(&self, name: &str) -> Option<PowerStateSubsystemSleepState> {
        if name == "APSS" {
            return self.apss();
        }
        None
    }

    fn support_connectivity_subsys(&self) -> bool {
        false
    }
}

/// Selects a parser from the ro.hardware property.
pub fn select_parser(hardware: &str) -> Box<dyn SubsystemStateParser> {
    if hardware.contains("qcom") {
        Box::new(QcomParser)
    } else if hardware.contains("mt6985") {
        Box::new(Mtk6985Parser)
    } else if hardware.contains("mt6893") {
        Box::new(X6739Parser::new())
    } else {
        Box::new(MtkParser)
    }
}

// ---------------------------------------------------------------------------
// Trigger-dump paths
// ---------------------------------------------------------------------------

/// Trigger dumps are gated on either of the two trigger properties; requests
/// without one are refused.
pub fn support_trigger_dump() -> bool {
    ["sys.oplus.rpmh.trigger_dump", "persist.vendor.oplus.rpmh.trigger_dump"]
        .iter()
        .any(|p| io_shim::property_get(p).as_deref() == Some("true"))
}

/// Type 0: ADSP subsystem restart (Qualcomm-only path).
/// Type 1: the MediaTek SCP watchdog reset.
/// Type 2: the full kernel dump via sysrq.
pub fn trigger_dump_type(dump_type: i32) -> i32 {
    if !support_trigger_dump() {
        return RESULT_UN_SUPPORTED;
    }
    let path = match dump_type {
        0 => "/sys/kernel/boot_adsp/ssr",
        1 => "/sys/class/misc/scp/wdt_reset",
        2 => "/proc/sysrq-trigger",
        _ => return dump_type,
    };
    let value = if dump_type == 2 { "c" } else { "1" };
    // The service reports success after the attempt regardless of whether the
    // write itself succeeded.
    let _ = io_shim::write_file(path, value);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn setup(name: &str) -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
        let guard = io_shim::test_lock();
        let root = format!("/tmp/opencode/rpmh_fixture_{}", name);
        let _ = fs::remove_dir_all(&root);
        for d in [
            "proc/mtk_lpm/suspend",
            "sys/power/suspend_stats",
            "sys/kernel/wakeup_reasons",
            "sys/class/misc/scp",
        ] {
            fs::create_dir_all(format!("{}/{}", root, d)).unwrap();
        }
        io_shim::set_test_root(&root);
        (PathBuf::from(root), guard)
    }

    fn teardown(root: &PathBuf) {
        io_shim::clear_test_root();
        let _ = fs::remove_dir_all(root);
    }

    fn w(root: &PathBuf, rel: &str, v: &str) {
        fs::write(format!("{}/{}", root.display(), rel), v).unwrap();
    }

    #[test]
    fn x6739_apss_reads_spm_sleep_count() {
        let (r, _g) = setup("apss");
        let p = X6739Parser::new();
        assert!(p.parse_subsystem_sleep_state("APSS").is_none());
        w(&r, "proc/mtk_lpm/suspend/spm_sleep_count", "3\n");
        let s = p.parse_subsystem_sleep_state("APSS").unwrap();
        assert_eq!(s.name, "APSS");
        assert_eq!(s.total_transitions, 3);
        assert_eq!(s.residency_in_msec_since_boot, 0);
        teardown(&r);
    }

    #[test]
    fn x6739_apss_falls_back_to_pm_suspend_stats() {
        let (r, _g) = setup("apss_fallback");
        w(&r, "sys/power/suspend_stats/success", "5\n");
        let p = X6739Parser::new();
        let s = p.parse_subsystem_sleep_state("APSS").unwrap();
        assert_eq!(s.total_transitions, 5);
        teardown(&r);
    }

    #[test]
    fn x6739_apss_residency_from_last_suspend_time() {
        let (r, _g) = setup("apss_residency");
        w(&r, "proc/mtk_lpm/suspend/spm_sleep_count", "1\n");
        w(
            &r,
            "sys/kernel/wakeup_reasons/last_suspend_time",
            "100.000000000 102.500000000\n",
        );
        let p = X6739Parser::new();
        let s = p.parse_subsystem_sleep_state("APSS").unwrap();
        assert_eq!(s.residency_in_msec_since_boot, 2500);
        teardown(&r);
    }

    #[test]
    fn x6739_unsupported_subsystems_yield_none() {
        let (r, _g) = setup("unsupported");
        let p = X6739Parser::new();
        for n in ["MPSS", "VMIN", "CONNINFRA", "WIFI", "BT", "GPS", "ADSP"] {
            assert!(p.parse_subsystem_sleep_state(n).is_none(), "{}", n);
        }
        assert!(!p.support_connectivity_subsys());
        teardown(&r);
    }

    #[test]
    fn parser_select_follows_ro_hardware() {
        assert!(!select_parser("qcom").support_connectivity_subsys());
        assert!(select_parser("mt6985").support_connectivity_subsys());
        assert!(!select_parser("mt6893").support_connectivity_subsys());
        assert!(select_parser("mt6789").support_connectivity_subsys());
    }

    #[test]
    fn stats_line_parses_transitions_and_residency_ms() {
        let s = parse_stats_line("APSS", "APSS 12 4.5").unwrap();
        assert_eq!(s.total_transitions, 12);
        assert_eq!(s.residency_in_msec_since_boot, 4500);
    }

    #[test]
    fn trigger_dump_refused_without_prop() {
        let (r, _g) = setup("trigger_off");
        assert_eq!(trigger_dump_type(2), RESULT_UN_SUPPORTED);
        teardown(&r);
    }

    #[test]
    fn trigger_dump_type_gates_and_routing() {
        let (r, _g) = setup("trigger_on");
        io_shim::property_set("sys.oplus.rpmh.trigger_dump", "true");
        w(&r, "proc/sysrq-trigger", "");
        w(&r, "sys/class/misc/scp/wdt_reset", "");
        assert_eq!(trigger_dump_type(2), 0);
        assert_eq!(trigger_dump_type(1), 0);
        assert_eq!(fs::read_to_string(r.join("proc/sysrq-trigger")).unwrap(), "c");
        assert_eq!(fs::read_to_string(r.join("sys/class/misc/scp/wdt_reset")).unwrap(), "1");
        assert_eq!(trigger_dump_type(9), 9);
        teardown(&r);
    }
}
