//! Debouncer for rate-limiting operations.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::lock_mutex;

/// Debouncer for rate-limiting operations
///
/// Useful for search input and other high-frequency events where
/// we want to wait for user input to settle before processing.
pub struct Debouncer {
    /// Minimum delay between operations
    delay: Duration,
    /// Last operation instant
    last_operation: Mutex<Option<Instant>>,
    /// Pending operation flag
    pending: AtomicBool,
}

impl Debouncer {
    /// Creates a new debouncer with the specified delay
    #[must_use]
    pub const fn new(delay: Duration) -> Self {
        Self {
            delay,
            last_operation: Mutex::new(None),
            pending: AtomicBool::new(false),
        }
    }

    /// Creates a debouncer with a 100ms delay (good for search input)
    #[must_use]
    pub const fn for_search() -> Self {
        Self::new(Duration::from_millis(100))
    }

    /// Creates a debouncer with a 16ms delay (good for rendering at 60fps)
    #[must_use]
    pub const fn for_render() -> Self {
        Self::new(Duration::from_millis(16))
    }

    /// Checks if enough time has passed since the last operation
    ///
    /// Returns `true` if the operation should proceed, `false` if it should be skipped.
    #[must_use]
    pub fn should_proceed(&self) -> bool {
        let now = Instant::now();
        let Some(mut last) = lock_mutex(&self.last_operation, "debouncer_last_op") else {
            return true;
        };

        match *last {
            None => {
                *last = Some(now);
                self.pending.store(false, Ordering::SeqCst);
                true
            }
            Some(last_time) if now.duration_since(last_time) >= self.delay => {
                *last = Some(now);
                self.pending.store(false, Ordering::SeqCst);
                true
            }
            _ => {
                self.pending.store(true, Ordering::SeqCst);
                false
            }
        }
    }

    /// Marks that there's a pending operation
    pub fn mark_pending(&self) {
        self.pending.store(true, Ordering::SeqCst);
    }

    /// Checks if there's a pending operation
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.pending.load(Ordering::SeqCst)
    }

    /// Resets the debouncer state
    pub fn reset(&self) {
        if let Some(mut last) = lock_mutex(&self.last_operation, "debouncer_last_op") {
            *last = None;
        }
        self.pending.store(false, Ordering::SeqCst);
    }

    /// Gets the delay duration
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer() {
        let debouncer = Debouncer::new(Duration::from_millis(50));

        // First call should proceed
        assert!(debouncer.should_proceed());

        // Immediate second call should not proceed
        assert!(!debouncer.should_proceed());
        assert!(debouncer.has_pending());

        // After delay, should proceed again
        std::thread::sleep(Duration::from_millis(60));
        assert!(debouncer.should_proceed());
    }
}
