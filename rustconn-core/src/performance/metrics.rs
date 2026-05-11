//! Performance metrics collection and timing utilities.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::lock_mutex;

/// Performance metrics collector for tracking application performance
///
/// Provides timing measurements for various operations including startup,
/// rendering, and search operations.
pub struct PerformanceMetrics {
    /// Startup timing measurements
    startup_timings: Mutex<HashMap<String, Duration>>,
    /// Operation timing measurements
    operation_timings: Mutex<HashMap<String, Vec<Duration>>>,
    /// Startup start time
    startup_start: Mutex<Option<Instant>>,
    /// Whether profiling is enabled
    profiling_enabled: AtomicBool,
}

impl PerformanceMetrics {
    /// Creates a new performance metrics instance
    #[must_use]
    pub fn new() -> Self {
        Self {
            startup_timings: Mutex::new(HashMap::new()),
            operation_timings: Mutex::new(HashMap::new()),
            startup_start: Mutex::new(None),
            profiling_enabled: AtomicBool::new(cfg!(debug_assertions)),
        }
    }

    /// Enables or disables profiling
    pub fn set_profiling_enabled(&self, enabled: bool) {
        self.profiling_enabled.store(enabled, Ordering::SeqCst);
    }

    /// Returns whether profiling is enabled
    #[must_use]
    pub fn is_profiling_enabled(&self) -> bool {
        self.profiling_enabled.load(Ordering::SeqCst)
    }

    /// Marks the start of application startup
    pub fn start_startup(&self) {
        if self.is_profiling_enabled()
            && let Some(mut guard) = lock_mutex(&self.startup_start, "startup_start")
        {
            *guard = Some(Instant::now());
        }
    }

    /// Records a startup phase timing
    pub fn record_startup_phase(&self, phase: &str) {
        if !self.is_profiling_enabled() {
            return;
        }

        let start = lock_mutex(&self.startup_start, "startup_start").and_then(|g| *g);
        if let Some(start) = start {
            let elapsed = start.elapsed();
            if let Some(mut timings) = lock_mutex(&self.startup_timings, "startup_timings") {
                timings.insert(phase.to_string(), elapsed);
            }
        }
    }

    /// Records an operation timing
    pub fn record_operation(&self, operation: &str, duration: Duration) {
        if !self.is_profiling_enabled() {
            return;
        }

        if let Some(mut timings) = lock_mutex(&self.operation_timings, "operation_timings") {
            timings
                .entry(operation.to_string())
                .or_default()
                .push(duration);
        }
    }

    /// Gets the total startup time
    #[must_use]
    pub fn total_startup_time(&self) -> Option<Duration> {
        lock_mutex(&self.startup_timings, "startup_timings")
            .and_then(|t| t.get("complete").copied())
    }

    /// Gets all startup phase timings
    #[must_use]
    pub fn startup_phases(&self) -> HashMap<String, Duration> {
        lock_mutex(&self.startup_timings, "startup_timings")
            .map(|t| t.clone())
            .unwrap_or_default()
    }

    /// Gets average duration for an operation
    #[must_use]
    pub fn average_operation_time(&self, operation: &str) -> Option<Duration> {
        let timings = lock_mutex(&self.operation_timings, "operation_timings")?;
        timings.get(operation).and_then(|durations| {
            if durations.is_empty() {
                None
            } else {
                let total: Duration = durations.iter().sum();
                Some(total / durations.len() as u32)
            }
        })
    }

    /// Gets operation statistics
    #[must_use]
    pub fn operation_stats(&self, operation: &str) -> Option<OperationStats> {
        let timings = lock_mutex(&self.operation_timings, "operation_timings")?;
        timings.get(operation).and_then(|durations| {
            if durations.is_empty() {
                return None;
            }

            let mut sorted: Vec<_> = durations.clone();
            sorted.sort();

            let count = sorted.len();
            let total: Duration = sorted.iter().sum();
            let avg = total / count as u32;
            let min = sorted[0];
            let max = sorted[count - 1];
            let median = sorted[count / 2];
            let p95 = sorted[(count as f64 * 0.95) as usize];

            Some(OperationStats {
                count,
                total,
                average: avg,
                min,
                max,
                median,
                p95,
            })
        })
    }

    /// Clears all recorded metrics
    pub fn clear(&self) {
        if let Some(mut t) = lock_mutex(&self.startup_timings, "startup_timings") {
            t.clear();
        }
        if let Some(mut t) = lock_mutex(&self.operation_timings, "operation_timings") {
            t.clear();
        }
        if let Some(mut s) = lock_mutex(&self.startup_start, "startup_start") {
            *s = None;
        }
    }

    /// Creates a timing guard for measuring operation duration
    #[must_use]
    pub fn time_operation(&self, operation: &str) -> TimingGuard<'_> {
        TimingGuard::new(self, operation)
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for an operation
#[derive(Debug, Clone)]
pub struct OperationStats {
    /// Number of measurements
    pub count: usize,
    /// Total time across all measurements
    pub total: Duration,
    /// Average time per operation
    pub average: Duration,
    /// Minimum time
    pub min: Duration,
    /// Maximum time
    pub max: Duration,
    /// Median time
    pub median: Duration,
    /// 95th percentile time
    pub p95: Duration,
}

/// RAII guard for timing operations
pub struct TimingGuard<'a> {
    metrics: &'a PerformanceMetrics,
    operation: String,
    start: Instant,
}

impl<'a> TimingGuard<'a> {
    fn new(metrics: &'a PerformanceMetrics, operation: &str) -> Self {
        Self {
            metrics,
            operation: operation.to_string(),
            start: Instant::now(),
        }
    }
}

impl Drop for TimingGuard<'_> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.metrics.record_operation(&self.operation, duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_metrics_startup() {
        let metrics = PerformanceMetrics::new();
        metrics.set_profiling_enabled(true);
        metrics.start_startup();

        std::thread::sleep(Duration::from_millis(10));
        metrics.record_startup_phase("config_load");

        std::thread::sleep(Duration::from_millis(10));
        metrics.record_startup_phase("complete");

        let phases = metrics.startup_phases();
        assert!(phases.contains_key("config_load"));
        assert!(phases.contains_key("complete"));
        assert!(phases["complete"] > phases["config_load"]);
    }

    #[test]
    fn test_performance_metrics_operations() {
        let metrics = PerformanceMetrics::new();
        metrics.set_profiling_enabled(true);

        metrics.record_operation("search", Duration::from_millis(10));
        metrics.record_operation("search", Duration::from_millis(20));
        metrics.record_operation("search", Duration::from_millis(15));

        let avg = metrics.average_operation_time("search").unwrap();
        assert!(avg >= Duration::from_millis(14) && avg <= Duration::from_millis(16));

        let stats = metrics.operation_stats("search").unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min, Duration::from_millis(10));
        assert_eq!(stats.max, Duration::from_millis(20));
    }

    #[test]
    fn test_timing_guard() {
        let metrics = PerformanceMetrics::new();
        metrics.set_profiling_enabled(true);

        {
            let _guard = metrics.time_operation("test_op");
            std::thread::sleep(Duration::from_millis(5));
        }

        let stats = metrics.operation_stats("test_op");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().count, 1);
    }
}
