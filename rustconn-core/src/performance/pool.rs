//! Memory-efficient pool for reusing allocations.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::lock_mutex;

/// Memory-efficient pool for reusing allocations
///
/// Reduces allocation overhead by reusing previously allocated objects.
pub struct ObjectPool<T> {
    /// Pool of available objects
    pool: Mutex<Vec<T>>,
    /// Maximum pool size
    max_size: usize,
    /// Statistics
    stats: PoolStats,
}

/// Statistics for the object pool
#[derive(Debug, Default)]
pub struct PoolStats {
    /// Number of objects acquired
    pub acquired: AtomicUsize,
    /// Number of objects returned
    pub returned: AtomicUsize,
    /// Number of objects created (not from pool)
    pub created: AtomicUsize,
    /// Number of objects dropped (pool full)
    pub dropped: AtomicUsize,
}

impl<T: Default> ObjectPool<T> {
    /// Creates a new object pool with the specified maximum size
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Mutex::new(Vec::with_capacity(max_size)),
            max_size,
            stats: PoolStats::default(),
        }
    }

    /// Acquires an object from the pool, or creates a new one if empty
    pub fn acquire(&self) -> T {
        self.stats.acquired.fetch_add(1, Ordering::Relaxed);

        if let Some(mut pool) = lock_mutex(&self.pool, "object_pool")
            && let Some(obj) = pool.pop()
        {
            return obj;
        }
        self.stats.created.fetch_add(1, Ordering::Relaxed);
        T::default()
    }

    /// Returns an object to the pool for reuse
    pub fn release(&self, obj: T) {
        self.stats.returned.fetch_add(1, Ordering::Relaxed);

        if let Some(mut pool) = lock_mutex(&self.pool, "object_pool") {
            if pool.len() < self.max_size {
                pool.push(obj);
            } else {
                self.stats.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Returns the current pool size
    #[must_use]
    pub fn size(&self) -> usize {
        lock_mutex(&self.pool, "object_pool")
            .map(|p| p.len())
            .unwrap_or(0)
    }

    /// Returns pool statistics
    #[must_use]
    pub const fn stats(&self) -> &PoolStats {
        &self.stats
    }

    /// Clears the pool
    pub fn clear(&self) {
        if let Some(mut p) = lock_mutex(&self.pool, "object_pool") {
            p.clear();
        }
    }

    /// Pre-populates the pool with objects
    pub fn warm(&self, count: usize) {
        if let Some(mut pool) = lock_mutex(&self.pool, "object_pool") {
            let to_add = count.min(self.max_size).saturating_sub(pool.len());
            for _ in 0..to_add {
                pool.push(T::default());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool_basic() {
        let pool: ObjectPool<Vec<u8>> = ObjectPool::new(10);

        // Acquire creates new object when pool is empty
        let obj1 = pool.acquire();
        assert!(obj1.is_empty());
        assert_eq!(pool.stats().created.load(Ordering::Relaxed), 1);

        // Release returns object to pool
        pool.release(obj1);
        assert_eq!(pool.size(), 1);

        // Acquire reuses pooled object
        let _obj2 = pool.acquire();
        assert_eq!(pool.size(), 0);
        assert_eq!(pool.stats().acquired.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_object_pool_max_size() {
        let pool: ObjectPool<Vec<u8>> = ObjectPool::new(2);

        pool.release(Vec::new());
        pool.release(Vec::new());
        pool.release(Vec::new()); // Should be dropped

        assert_eq!(pool.size(), 2);
        assert_eq!(pool.stats().dropped.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_object_pool_warm() {
        let pool: ObjectPool<Vec<u8>> = ObjectPool::new(10);
        pool.warm(5);
        assert_eq!(pool.size(), 5);
    }
}
