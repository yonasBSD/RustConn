//! Property-based tests for the monitoring module
//!
//! Tests cover `MetricsParser`, `MetricsComputer`, `MonitoringConfig`,
//! and `MonitoringSettings` from `rustconn_core::monitoring`.

use proptest::prelude::*;
use rustconn_core::monitoring::{
    MetricsComputer, MetricsParser, MonitoringConfig, MonitoringSettings,
};

// ---------------------------------------------------------------------------
// MonitoringSettings property tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn effective_interval_always_in_range(interval in 0u8..=255) {
        let settings = MonitoringSettings {
            interval_secs: interval,
            ..Default::default()
        };
        let effective = settings.effective_interval_secs();
        prop_assert!(effective >= 1);
        prop_assert!(effective <= 60);
    }
}

#[test]
fn default_settings_are_all_enabled() {
    let s = MonitoringSettings::default();
    assert!(s.enabled);
    assert!(s.show_cpu);
    assert!(s.show_memory);
    assert!(s.show_disk);
    assert!(s.show_network);
    assert!(s.show_load);
    assert!(s.show_system_info);
    assert_eq!(s.interval_secs, 3);
}

// ---------------------------------------------------------------------------
// MonitoringConfig override tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn config_override_interval_clamped(
        global_interval in 1u8..=60,
        override_interval in 0u8..=255,
    ) {
        let global = MonitoringSettings {
            interval_secs: global_interval,
            ..Default::default()
        };
        let config = MonitoringConfig {
            enabled: None,
            interval_secs: Some(override_interval),
        };
        let effective = config.effective_interval(&global);
        prop_assert!(effective >= 1);
        prop_assert!(effective <= 60);
    }

    #[test]
    fn config_none_falls_back_to_global(
        global_enabled in proptest::bool::ANY,
        global_interval in 1u8..=60,
    ) {
        let global = MonitoringSettings {
            enabled: global_enabled,
            interval_secs: global_interval,
            ..Default::default()
        };
        let config = MonitoringConfig {
            enabled: None,
            interval_secs: None,
        };
        prop_assert_eq!(config.is_enabled(&global), global_enabled);
        prop_assert_eq!(config.effective_interval(&global), global_interval);
    }

    #[test]
    fn config_override_takes_precedence(
        global_enabled in proptest::bool::ANY,
        override_enabled in proptest::bool::ANY,
    ) {
        let global = MonitoringSettings {
            enabled: global_enabled,
            ..Default::default()
        };
        let config = MonitoringConfig {
            enabled: Some(override_enabled),
            interval_secs: None,
        };
        prop_assert_eq!(config.is_enabled(&global), override_enabled);
    }
}

// ---------------------------------------------------------------------------
// MetricsParser tests
// ---------------------------------------------------------------------------

/// Realistic output matching the actual METRICS_COMMAND marker format
const SAMPLE_OUTPUT: &str = "\
---RUSTCONN_PROC_STAT---
cpu  10132153 290696 3084719 46828483 16683 0 25195 0 0 0
---RUSTCONN_MEMINFO---
MemTotal:       16384000 kB
MemAvailable:    8192000 kB
SwapTotal:       4194304 kB
SwapFree:        3145728 kB
---RUSTCONN_LOADAVG---
0.52 0.38 0.41 2/345 12345
---RUSTCONN_NET_DEV---
  eth0: 98765432   65432    0    0    0     0          0         0 12345678   43210    0    0    0     0       0          0
    lo: 1234567    1234    0    0    0     0          0         0  1234567    1234    0    0    0     0       0          0
---RUSTCONN_DF---
/dev/sda1      102400000 51200000  46080000  53% /
/dev/sda2       51200000 25600000  23040000  53% /home
---RUSTCONN_END---
";

#[test]
fn parser_handles_realistic_output() {
    let parsed = MetricsParser::parse(SAMPLE_OUTPUT);
    assert!(parsed.is_ok(), "parse failed: {parsed:?}");
    let parsed = parsed.unwrap();
    assert!(parsed.memory.total_kib > 0);
    assert!(!parsed.disks.is_empty());
    assert!(parsed.load_average.one > 0.0);
}

#[test]
fn parser_rejects_empty_input() {
    let result = MetricsParser::parse("");
    assert!(result.is_err());
}

#[test]
fn parser_handles_missing_df_gracefully() {
    let output = "\
---RUSTCONN_PROC_STAT---
cpu  100 20 30 400 5 0 2 0 0 0
---RUSTCONN_MEMINFO---
MemTotal:       8192000 kB
MemAvailable:   4096000 kB
SwapTotal:            0 kB
SwapFree:             0 kB
---RUSTCONN_LOADAVG---
0.10 0.20 0.30 1/100 999
---RUSTCONN_NET_DEV---
    lo: 0    0    0    0    0     0          0         0  0    0    0    0    0     0       0          0
---RUSTCONN_DF---
---RUSTCONN_END---
";
    let parsed = MetricsParser::parse(output);
    assert!(parsed.is_ok());
    let parsed = parsed.unwrap();
    // df section empty → empty disks list
    assert!(parsed.disks.is_empty());
}

// ---------------------------------------------------------------------------
// MetricsComputer tests
// ---------------------------------------------------------------------------

#[test]
fn computer_first_call_returns_zero_cpu() {
    let mut computer = MetricsComputer::new();
    let parsed = MetricsParser::parse(SAMPLE_OUTPUT).unwrap();
    let metrics = computer.compute(&parsed);
    // First call has no previous snapshot → 0% CPU
    assert!((metrics.cpu_percent - 0.0).abs() < f32::EPSILON);
    assert!((metrics.network.rx_bytes_per_sec - 0.0).abs() < f64::EPSILON);
}

#[test]
fn computer_second_call_computes_delta() {
    let mut computer = MetricsComputer::new();
    let parsed = MetricsParser::parse(SAMPLE_OUTPUT).unwrap();
    let _ = computer.compute(&parsed);
    // Second call with same data → 0% CPU (no change)
    let metrics = computer.compute(&parsed);
    assert!((metrics.cpu_percent - 0.0).abs() < f32::EPSILON);
}

#[test]
fn computer_reset_clears_state() {
    let mut computer = MetricsComputer::new();
    let parsed = MetricsParser::parse(SAMPLE_OUTPUT).unwrap();
    let _ = computer.compute(&parsed);
    computer.reset();
    // After reset, first call again → 0% CPU
    let metrics = computer.compute(&parsed);
    assert!((metrics.cpu_percent - 0.0).abs() < f32::EPSILON);
}

#[test]
fn computer_preserves_memory_and_disk() {
    let mut computer = MetricsComputer::new();
    let parsed = MetricsParser::parse(SAMPLE_OUTPUT).unwrap();
    let metrics = computer.compute(&parsed);
    assert_eq!(metrics.memory.total_kib, 16_384_000);
    assert!(!metrics.disks.is_empty());
    assert_eq!(metrics.disk.mount_point, "/");
}
