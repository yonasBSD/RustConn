//! Memory usage tracking and optimization utilities.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::interner::StringInterner;
use super::{format_bytes, lock_mutex};

/// Memory usage tracker
///
/// Provides utilities for tracking and optimizing memory usage.
pub struct MemoryTracker {
    /// Tracked allocations by category
    allocations: Mutex<HashMap<String, usize>>,
    /// Peak memory usage by category
    peak_usage: Mutex<HashMap<String, usize>>,
}

impl MemoryTracker {
    /// Creates a new memory tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
            peak_usage: Mutex::new(HashMap::new()),
        }
    }

    /// Records an allocation
    pub fn record_allocation(&self, category: &str, size: usize) {
        let Some(mut allocations) = lock_mutex(&self.allocations, "allocations") else {
            return;
        };
        let current = allocations.entry(category.to_string()).or_insert(0);
        *current += size;

        let current_val = *current;
        drop(allocations);

        if let Some(mut peak) = lock_mutex(&self.peak_usage, "peak_usage") {
            let peak_val = peak.entry(category.to_string()).or_insert(0);
            if current_val > *peak_val {
                *peak_val = current_val;
            }
        }
    }

    /// Records a deallocation
    pub fn record_deallocation(&self, category: &str, size: usize) {
        if let Some(mut allocations) = lock_mutex(&self.allocations, "allocations")
            && let Some(current) = allocations.get_mut(category)
        {
            *current = current.saturating_sub(size);
        }
    }

    /// Gets current allocation for a category
    #[must_use]
    pub fn current_allocation(&self, category: &str) -> usize {
        lock_mutex(&self.allocations, "allocations")
            .and_then(|a| a.get(category).copied())
            .unwrap_or(0)
    }

    /// Gets peak allocation for a category
    #[must_use]
    pub fn peak_allocation(&self, category: &str) -> usize {
        lock_mutex(&self.peak_usage, "peak_usage")
            .and_then(|p| p.get(category).copied())
            .unwrap_or(0)
    }

    /// Gets total current allocation across all categories
    #[must_use]
    pub fn total_allocation(&self) -> usize {
        lock_mutex(&self.allocations, "allocations")
            .map(|a| a.values().sum())
            .unwrap_or(0)
    }

    /// Gets all allocation statistics
    #[must_use]
    pub fn all_stats(&self) -> HashMap<String, AllocationStats> {
        let Some(allocations) = lock_mutex(&self.allocations, "allocations") else {
            return HashMap::new();
        };
        let Some(peak) = lock_mutex(&self.peak_usage, "peak_usage") else {
            return HashMap::new();
        };

        allocations
            .iter()
            .map(|(k, &current)| {
                let peak_val = peak.get(k).copied().unwrap_or(current);
                (
                    k.clone(),
                    AllocationStats {
                        current,
                        peak: peak_val,
                    },
                )
            })
            .collect()
    }

    /// Clears all tracking data
    pub fn clear(&self) {
        if let Some(mut a) = lock_mutex(&self.allocations, "allocations") {
            a.clear();
        }
        if let Some(mut p) = lock_mutex(&self.peak_usage, "peak_usage") {
            p.clear();
        }
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Allocation statistics for a category
#[derive(Debug, Clone, Copy)]
pub struct AllocationStats {
    /// Current allocation in bytes
    pub current: usize,
    /// Peak allocation in bytes
    pub peak: usize,
}

/// Memory optimizer providing utilities for reducing memory usage
///
/// Provides methods for estimating memory usage, identifying optimization
/// opportunities, and implementing memory-efficient patterns.
pub struct MemoryOptimizer {
    /// String interner for deduplication
    interner: StringInterner,
    /// Memory usage snapshots
    snapshots: Mutex<Vec<MemorySnapshot>>,
    /// Maximum snapshots to retain
    max_snapshots: usize,
}

/// A snapshot of memory usage at a point in time
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    /// Timestamp of the snapshot
    pub timestamp: Instant,
    /// Label for the snapshot
    pub label: String,
    /// Estimated heap usage in bytes
    pub heap_estimate: usize,
    /// Number of connections
    pub connection_count: usize,
    /// Number of groups
    pub group_count: usize,
    /// Number of sessions
    pub session_count: usize,
}

/// Memory usage estimate for a data structure
#[derive(Debug, Clone, Default)]
pub struct MemoryEstimate {
    /// Stack size in bytes
    pub stack_size: usize,
    /// Heap size in bytes (estimated)
    pub heap_size: usize,
    /// Total size in bytes
    pub total_size: usize,
    /// Number of heap allocations (estimated)
    pub allocation_count: usize,
}

impl MemoryEstimate {
    /// Creates a new memory estimate
    #[must_use]
    pub const fn new(stack_size: usize, heap_size: usize, allocation_count: usize) -> Self {
        Self {
            stack_size,
            heap_size,
            total_size: stack_size + heap_size,
            allocation_count,
        }
    }

    /// Adds another estimate to this one
    #[must_use]
    pub const fn add(&self, other: &Self) -> Self {
        Self {
            stack_size: self.stack_size + other.stack_size,
            heap_size: self.heap_size + other.heap_size,
            total_size: self.total_size + other.total_size,
            allocation_count: self.allocation_count + other.allocation_count,
        }
    }

    /// Multiplies the estimate by a count
    #[must_use]
    pub const fn multiply(&self, count: usize) -> Self {
        Self {
            stack_size: self.stack_size * count,
            heap_size: self.heap_size * count,
            total_size: self.total_size * count,
            allocation_count: self.allocation_count * count,
        }
    }

    /// Formats the estimate as a human-readable string
    #[must_use]
    pub fn format(&self) -> String {
        format!(
            "stack: {}, heap: {}, total: {}, allocations: {}",
            format_bytes(self.stack_size),
            format_bytes(self.heap_size),
            format_bytes(self.total_size),
            self.allocation_count
        )
    }
}

/// Memory optimization recommendation
#[derive(Debug, Clone)]
pub struct OptimizationRecommendation {
    /// Category of the recommendation
    pub category: OptimizationCategory,
    /// Description of the issue
    pub description: String,
    /// Estimated memory savings in bytes
    pub estimated_savings: usize,
    /// Priority level (1-5, 5 being highest)
    pub priority: u8,
}

/// Categories of memory optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationCategory {
    /// String deduplication opportunities
    StringDeduplication,
    /// Collection capacity optimization
    CollectionCapacity,
    /// Unused data cleanup
    UnusedData,
    /// Data structure choice
    DataStructure,
    /// Cache management
    CacheManagement,
}

impl MemoryOptimizer {
    /// Creates a new memory optimizer
    #[must_use]
    pub fn new() -> Self {
        Self {
            interner: StringInterner::new(),
            snapshots: Mutex::new(Vec::new()),
            max_snapshots: 100,
        }
    }

    /// Gets the string interner
    #[must_use]
    pub const fn interner(&self) -> &StringInterner {
        &self.interner
    }

    /// Takes a memory snapshot
    pub fn take_snapshot(
        &self,
        label: &str,
        connection_count: usize,
        group_count: usize,
        session_count: usize,
    ) {
        let snapshot = MemorySnapshot {
            timestamp: Instant::now(),
            label: label.to_string(),
            heap_estimate: self.estimate_current_heap(connection_count, group_count, session_count),
            connection_count,
            group_count,
            session_count,
        };

        let Some(mut snapshots) = lock_mutex(&self.snapshots, "snapshots") else {
            return;
        };
        if snapshots.len() >= self.max_snapshots {
            snapshots.remove(0);
        }
        snapshots.push(snapshot);
    }

    /// Gets all memory snapshots
    #[must_use]
    pub fn snapshots(&self) -> Vec<MemorySnapshot> {
        lock_mutex(&self.snapshots, "snapshots")
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Clears all snapshots
    pub fn clear_snapshots(&self) {
        if let Some(mut s) = lock_mutex(&self.snapshots, "snapshots") {
            s.clear();
        }
    }

    /// Estimates current heap usage based on data counts
    #[must_use]
    pub const fn estimate_current_heap(
        &self,
        connection_count: usize,
        group_count: usize,
        session_count: usize,
    ) -> usize {
        // Rough estimates based on typical data structure sizes
        const CONNECTION_SIZE: usize = 2048; // ~2KB per connection with all fields
        const GROUP_SIZE: usize = 256; // ~256 bytes per group
        const SESSION_SIZE: usize = 512; // ~512 bytes per session

        connection_count * CONNECTION_SIZE + group_count * GROUP_SIZE + session_count * SESSION_SIZE
    }

    /// Estimates memory usage for a string
    #[must_use]
    pub const fn estimate_string(s: &str) -> MemoryEstimate {
        let stack_size = std::mem::size_of::<String>();
        let heap_size = s.len() + std::mem::size_of::<usize>(); // capacity overhead
        MemoryEstimate::new(stack_size, heap_size, 1)
    }

    /// Estimates memory usage for a Vec
    #[must_use]
    pub const fn estimate_vec<T>(vec: &[T]) -> MemoryEstimate {
        let stack_size = std::mem::size_of::<Vec<T>>();
        let heap_size = std::mem::size_of_val(vec);
        MemoryEstimate::new(stack_size, heap_size, 1)
    }

    /// Estimates memory usage for a `HashMap`
    #[must_use]
    pub fn estimate_hashmap<K, V>(map: &HashMap<K, V>) -> MemoryEstimate {
        let stack_size = std::mem::size_of::<HashMap<K, V>>();
        // HashMap has ~1.5x overhead for hash table
        let entry_size = std::mem::size_of::<K>() + std::mem::size_of::<V>() + 8; // 8 bytes for hash
        let heap_size = (map.len() as f64 * entry_size as f64 * 1.5) as usize;
        MemoryEstimate::new(stack_size, heap_size, 1)
    }

    /// Analyzes memory usage and provides optimization recommendations
    #[must_use]
    pub fn analyze(
        &self,
        connection_count: usize,
        _group_count: usize,
        session_count: usize,
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // Check string interner effectiveness
        let interner_stats = self.interner.stats();
        let hit_rate = if interner_stats.intern_count.load(Ordering::Relaxed) > 0 {
            interner_stats.hit_count.load(Ordering::Relaxed) as f64
                / interner_stats.intern_count.load(Ordering::Relaxed) as f64
        } else {
            0.0
        };

        if hit_rate < 0.3 && interner_stats.intern_count.load(Ordering::Relaxed) > 100 {
            recommendations.push(OptimizationRecommendation {
                category: OptimizationCategory::StringDeduplication,
                description: format!(
                    "Low string interner hit rate ({:.1}%). Consider interning more repeated strings.",
                    hit_rate * 100.0
                ),
                estimated_savings: interner_stats.bytes_saved.load(Ordering::Relaxed) * 2,
                priority: 3,
            });
        }

        // Check for large connection counts
        if connection_count > 500 {
            recommendations.push(OptimizationRecommendation {
                category: OptimizationCategory::DataStructure,
                description: format!(
                    "Large connection count ({connection_count}). Consider using virtual scrolling for UI."
                ),
                estimated_savings: connection_count * 100, // UI widget overhead
                priority: 4,
            });
        }

        // Check for many active sessions
        if session_count > 20 {
            recommendations.push(OptimizationRecommendation {
                category: OptimizationCategory::CacheManagement,
                description: format!(
                    "Many active sessions ({session_count}). Consider session cleanup for idle connections."
                ),
                estimated_savings: session_count * 512,
                priority: 3,
            });
        }

        // Check snapshots for memory growth
        if let Some(snapshots) = lock_mutex(&self.snapshots, "snapshots")
            && snapshots.len() >= 2
        {
            let first = &snapshots[0];
            let last = &snapshots[snapshots.len() - 1];
            let growth = last.heap_estimate.saturating_sub(first.heap_estimate);
            let growth_rate = if first.heap_estimate > 0 {
                growth as f64 / first.heap_estimate as f64
            } else {
                0.0
            };

            if growth_rate > 0.5 && growth > 1024 * 1024 {
                recommendations.push(OptimizationRecommendation {
                    category: OptimizationCategory::UnusedData,
                    description: format!(
                        "Memory grew by {:.1}% ({}) since first snapshot. Check for memory leaks.",
                        growth_rate * 100.0,
                        format_bytes(growth)
                    ),
                    estimated_savings: growth / 2,
                    priority: 5,
                });
            }
        }

        // Sort by priority (highest first)
        recommendations.sort_by_key(|a| std::cmp::Reverse(a.priority));
        recommendations
    }

    /// Generates a memory usage report
    #[must_use]
    pub fn generate_report(
        &self,
        connection_count: usize,
        group_count: usize,
        session_count: usize,
    ) -> String {
        let mut report = String::new();
        report.push_str("=== Memory Usage Report ===\n\n");

        // Current estimates
        let heap_estimate =
            self.estimate_current_heap(connection_count, group_count, session_count);
        report.push_str(&format!(
            "Estimated heap usage: {}\n",
            format_bytes(heap_estimate)
        ));
        report.push_str(&format!(
            "  Connections: {} ({} each)\n",
            connection_count,
            format_bytes(2048)
        ));
        report.push_str(&format!(
            "  Groups: {} ({} each)\n",
            group_count,
            format_bytes(256)
        ));
        report.push_str(&format!(
            "  Sessions: {} ({} each)\n\n",
            session_count,
            format_bytes(512)
        ));

        // String interner stats
        let stats = self.interner.stats();
        report.push_str("String Interner:\n");
        report.push_str(&format!(
            "  Unique strings: {}\n",
            stats.unique_count.load(Ordering::Relaxed)
        ));
        report.push_str(&format!(
            "  Intern requests: {}\n",
            stats.intern_count.load(Ordering::Relaxed)
        ));
        report.push_str(&format!(
            "  Cache hits: {}\n",
            stats.hit_count.load(Ordering::Relaxed)
        ));
        report.push_str(&format!(
            "  Bytes saved: {}\n\n",
            format_bytes(stats.bytes_saved.load(Ordering::Relaxed))
        ));

        // Recommendations
        let recommendations = self.analyze(connection_count, group_count, session_count);
        if recommendations.is_empty() {
            report.push_str("No optimization recommendations at this time.\n");
        } else {
            report.push_str("Optimization Recommendations:\n");
            for rec in &recommendations {
                report.push_str(&format!(
                    "  [P{}] {:?}: {}\n       Estimated savings: {}\n",
                    rec.priority,
                    rec.category,
                    rec.description,
                    format_bytes(rec.estimated_savings)
                ));
            }
        }

        report
    }
}

impl Default for MemoryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory pressure levels for adaptive behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPressure {
    /// Low memory usage, normal operation
    Low,
    /// Moderate memory usage, consider optimization
    Moderate,
    /// High memory usage, aggressive optimization needed
    High,
    /// Critical memory usage, emergency measures
    Critical,
}

impl MemoryPressure {
    /// Determines memory pressure based on estimated usage and threshold
    #[must_use]
    pub fn from_usage(current_bytes: usize, threshold_bytes: usize) -> Self {
        let ratio = current_bytes as f64 / threshold_bytes as f64;

        if ratio < 0.5 {
            Self::Low
        } else if ratio < 0.75 {
            Self::Moderate
        } else if ratio < 0.9 {
            Self::High
        } else {
            Self::Critical
        }
    }

    /// Returns recommended actions for this pressure level
    #[must_use]
    pub const fn recommended_actions(&self) -> &'static [&'static str] {
        match self {
            Self::Low => &[],
            Self::Moderate => &[
                "Consider clearing unused caches",
                "Shrink over-allocated collections",
            ],
            Self::High => &[
                "Clear all caches",
                "Shrink all collections",
                "Disable non-essential features",
            ],
            Self::Critical => &[
                "Emergency cache clear",
                "Close idle sessions",
                "Reduce connection list to visible items only",
            ],
        }
    }
}

/// Detailed memory breakdown by category
#[derive(Debug, Clone, Default)]
pub struct MemoryBreakdown {
    /// Memory used by connections
    pub connections: usize,
    /// Memory used by groups
    pub groups: usize,
    /// Memory used by sessions
    pub sessions: usize,
    /// Memory used by snippets
    pub snippets: usize,
    /// Memory used by templates
    pub templates: usize,
    /// Memory used by clusters
    pub clusters: usize,
    /// Memory used by caches
    pub caches: usize,
    /// Memory used by UI state
    pub ui_state: usize,
    /// Other/overhead memory
    pub overhead: usize,
}

impl MemoryBreakdown {
    /// Creates a new memory breakdown with estimated sizes
    #[must_use]
    pub const fn estimate(
        connection_count: usize,
        group_count: usize,
        session_count: usize,
        snippet_count: usize,
        template_count: usize,
        cluster_count: usize,
    ) -> Self {
        // Size estimates based on typical struct sizes
        const CONNECTION_SIZE: usize = 2048;
        const GROUP_SIZE: usize = 256;
        const SESSION_SIZE: usize = 512;
        const SNIPPET_SIZE: usize = 512;
        const TEMPLATE_SIZE: usize = 1024;
        const CLUSTER_SIZE: usize = 384;
        const CACHE_OVERHEAD: usize = 4096;
        const UI_STATE_SIZE: usize = 8192;
        const BASE_OVERHEAD: usize = 16384;

        Self {
            connections: connection_count * CONNECTION_SIZE,
            groups: group_count * GROUP_SIZE,
            sessions: session_count * SESSION_SIZE,
            snippets: snippet_count * SNIPPET_SIZE,
            templates: template_count * TEMPLATE_SIZE,
            clusters: cluster_count * CLUSTER_SIZE,
            caches: CACHE_OVERHEAD,
            ui_state: UI_STATE_SIZE,
            overhead: BASE_OVERHEAD,
        }
    }

    /// Returns total estimated memory usage
    #[must_use]
    pub const fn total(&self) -> usize {
        self.connections
            + self.groups
            + self.sessions
            + self.snippets
            + self.templates
            + self.clusters
            + self.caches
            + self.ui_state
            + self.overhead
    }

    /// Returns the largest memory consumer
    #[must_use]
    pub fn largest_consumer(&self) -> (&'static str, usize) {
        let categories = [
            ("connections", self.connections),
            ("groups", self.groups),
            ("sessions", self.sessions),
            ("snippets", self.snippets),
            ("templates", self.templates),
            ("clusters", self.clusters),
            ("caches", self.caches),
            ("ui_state", self.ui_state),
            ("overhead", self.overhead),
        ];

        categories
            .into_iter()
            .max_by_key(|(_, size)| *size)
            .unwrap_or(("unknown", 0))
    }

    /// Formats the breakdown as a report
    #[must_use]
    pub fn format_report(&self) -> String {
        let mut report = String::new();
        report.push_str("Memory Breakdown:\n");
        report.push_str(&format!(
            "  Connections: {}\n",
            format_bytes(self.connections)
        ));
        report.push_str(&format!("  Groups: {}\n", format_bytes(self.groups)));
        report.push_str(&format!("  Sessions: {}\n", format_bytes(self.sessions)));
        report.push_str(&format!("  Snippets: {}\n", format_bytes(self.snippets)));
        report.push_str(&format!("  Templates: {}\n", format_bytes(self.templates)));
        report.push_str(&format!("  Clusters: {}\n", format_bytes(self.clusters)));
        report.push_str(&format!("  Caches: {}\n", format_bytes(self.caches)));
        report.push_str(&format!("  UI State: {}\n", format_bytes(self.ui_state)));
        report.push_str(&format!("  Overhead: {}\n", format_bytes(self.overhead)));
        report.push_str("  ─────────────────\n");
        report.push_str(&format!("  Total: {}\n", format_bytes(self.total())));
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();

        tracker.record_allocation("connections", 1000);
        tracker.record_allocation("connections", 500);
        assert_eq!(tracker.current_allocation("connections"), 1500);

        tracker.record_deallocation("connections", 300);
        assert_eq!(tracker.current_allocation("connections"), 1200);
        assert_eq!(tracker.peak_allocation("connections"), 1500);
    }

    #[test]
    fn test_memory_estimate() {
        let est1 = MemoryEstimate::new(8, 100, 1);
        let est2 = MemoryEstimate::new(8, 50, 1);

        let combined = est1.add(&est2);
        assert_eq!(combined.stack_size, 16);
        assert_eq!(combined.heap_size, 150);
        assert_eq!(combined.total_size, 166);
        assert_eq!(combined.allocation_count, 2);

        let multiplied = est1.multiply(3);
        assert_eq!(multiplied.stack_size, 24);
        assert_eq!(multiplied.heap_size, 300);
    }

    #[test]
    fn test_memory_optimizer_snapshot() {
        let optimizer = MemoryOptimizer::new();

        optimizer.take_snapshot("initial", 10, 2, 1);
        optimizer.take_snapshot("after_load", 50, 5, 3);

        let snapshots = optimizer.snapshots();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].label, "initial");
        assert_eq!(snapshots[1].label, "after_load");
        assert_eq!(snapshots[0].connection_count, 10);
        assert_eq!(snapshots[1].connection_count, 50);
    }

    #[test]
    fn test_memory_optimizer_estimate() {
        let optimizer = MemoryOptimizer::new();

        let estimate = optimizer.estimate_current_heap(10, 2, 1);
        // 10 * 2048 + 2 * 256 + 1 * 512 = 20480 + 512 + 512 = 21504
        assert_eq!(estimate, 21504);
    }

    #[test]
    fn test_memory_optimizer_report() {
        let optimizer = MemoryOptimizer::new();
        optimizer.interner().intern("test");
        optimizer.interner().intern("test");

        let report = optimizer.generate_report(10, 2, 1);
        assert!(report.contains("Memory Usage Report"));
        assert!(report.contains("Connections: 10"));
        assert!(report.contains("String Interner"));
    }

    #[test]
    fn test_optimization_recommendations() {
        let optimizer = MemoryOptimizer::new();

        // Test with large connection count
        let recommendations = optimizer.analyze(600, 10, 5);
        assert!(
            recommendations
                .iter()
                .any(|r| r.category == OptimizationCategory::DataStructure)
        );

        // Test with many sessions
        let recommendations = optimizer.analyze(10, 5, 25);
        assert!(
            recommendations
                .iter()
                .any(|r| r.category == OptimizationCategory::CacheManagement)
        );
    }

    #[test]
    fn test_memory_pressure_levels() {
        assert_eq!(MemoryPressure::from_usage(40, 100), MemoryPressure::Low);
        assert_eq!(
            MemoryPressure::from_usage(60, 100),
            MemoryPressure::Moderate
        );
        assert_eq!(MemoryPressure::from_usage(80, 100), MemoryPressure::High);
        assert_eq!(
            MemoryPressure::from_usage(95, 100),
            MemoryPressure::Critical
        );
    }

    #[test]
    fn test_memory_pressure_recommendations() {
        assert!(MemoryPressure::Low.recommended_actions().is_empty());
        assert!(!MemoryPressure::Moderate.recommended_actions().is_empty());
        assert!(!MemoryPressure::High.recommended_actions().is_empty());
        assert!(!MemoryPressure::Critical.recommended_actions().is_empty());
    }

    #[test]
    fn test_memory_breakdown_estimate() {
        let breakdown = MemoryBreakdown::estimate(10, 5, 2, 3, 1, 2);

        assert!(breakdown.connections > 0);
        assert!(breakdown.groups > 0);
        assert!(breakdown.sessions > 0);
        assert!(breakdown.total() > 0);
    }

    #[test]
    fn test_memory_breakdown_largest_consumer() {
        let breakdown = MemoryBreakdown::estimate(100, 5, 2, 3, 1, 2);
        let (name, _size) = breakdown.largest_consumer();
        assert_eq!(name, "connections");
    }

    #[test]
    fn test_memory_breakdown_report() {
        let breakdown = MemoryBreakdown::estimate(10, 5, 2, 3, 1, 2);
        let report = breakdown.format_report();

        assert!(report.contains("Memory Breakdown"));
        assert!(report.contains("Connections"));
        assert!(report.contains("Total"));
    }
}
