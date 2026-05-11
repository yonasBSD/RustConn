//! String interner for deduplicating repeated strings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use super::{read_rwlock, write_rwlock};

/// String interner for deduplicating repeated strings
///
/// Reduces memory usage when the same strings appear multiple times
/// (e.g., protocol names, common hostnames, usernames).
pub struct StringInterner {
    /// Interned strings storage — keyed by the actual string for collision-free dedup
    strings: RwLock<HashMap<Arc<str>, Arc<str>>>,
    /// Statistics
    stats: InternerStats,
}

/// Statistics for the string interner
#[derive(Debug, Default)]
pub struct InternerStats {
    /// Number of intern requests
    pub intern_count: AtomicUsize,
    /// Number of cache hits
    pub hit_count: AtomicUsize,
    /// Number of unique strings stored
    pub unique_count: AtomicUsize,
    /// Estimated bytes saved through deduplication
    pub bytes_saved: AtomicUsize,
}

impl StringInterner {
    /// Creates a new string interner
    #[must_use]
    pub fn new() -> Self {
        Self {
            strings: RwLock::new(HashMap::new()),
            stats: InternerStats::default(),
        }
    }

    /// Interns a string, returning a reference-counted pointer
    ///
    /// If the string was already interned, returns the existing Arc.
    /// Otherwise, creates a new Arc and stores it.
    pub fn intern(&self, s: &str) -> Arc<str> {
        self.stats.intern_count.fetch_add(1, Ordering::Relaxed);

        let key: Arc<str> = Arc::from(s);

        // Try read lock first for cache hit
        {
            if let Some(strings) = read_rwlock(&self.strings, "interner_strings")
                && let Some(existing) = strings.get(&key)
            {
                self.stats.hit_count.fetch_add(1, Ordering::Relaxed);
                self.stats.bytes_saved.fetch_add(s.len(), Ordering::Relaxed);
                return Arc::clone(existing);
            }
        }

        // Need write lock to insert
        let Some(mut strings) = write_rwlock(&self.strings, "interner_strings") else {
            // Fallback: return a fresh Arc without caching
            return key;
        };

        // Double-check after acquiring write lock
        if let Some(existing) = strings.get(&key) {
            self.stats.hit_count.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes_saved.fetch_add(s.len(), Ordering::Relaxed);
            return Arc::clone(existing);
        }

        // Insert new string
        strings.insert(Arc::clone(&key), Arc::clone(&key));
        self.stats.unique_count.fetch_add(1, Ordering::Relaxed);
        key
    }

    /// Gets the interner statistics
    #[must_use]
    pub const fn stats(&self) -> &InternerStats {
        &self.stats
    }

    /// Returns the number of unique strings stored
    #[must_use]
    pub fn len(&self) -> usize {
        read_rwlock(&self.strings, "interner_strings")
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Returns true if no strings are interned
    #[must_use]
    pub fn is_empty(&self) -> bool {
        read_rwlock(&self.strings, "interner_strings")
            .map(|s| s.is_empty())
            .unwrap_or(true)
    }

    /// Clears all interned strings
    pub fn clear(&self) {
        if let Some(mut s) = write_rwlock(&self.strings, "interner_strings") {
            s.clear();
        }
        self.stats.unique_count.store(0, Ordering::Relaxed);
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interner_basic() {
        let interner = StringInterner::new();

        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        let s3 = interner.intern("world");

        // Same string should return same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        // Different strings should be different
        assert!(!Arc::ptr_eq(&s1, &s3));

        assert_eq!(interner.len(), 2);
        assert_eq!(interner.stats().hit_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_string_interner_stats() {
        let interner = StringInterner::new();

        interner.intern("test");
        interner.intern("test");
        interner.intern("test");
        interner.intern("other");

        let stats = interner.stats();
        assert_eq!(stats.intern_count.load(Ordering::Relaxed), 4);
        assert_eq!(stats.hit_count.load(Ordering::Relaxed), 2);
        assert_eq!(stats.unique_count.load(Ordering::Relaxed), 2);
    }
}
