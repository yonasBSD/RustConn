//! Lazy initializer for deferred loading.

use std::sync::OnceLock;

/// Lazy initializer for deferred loading
///
/// Wraps a value that is initialized on first access, useful for
/// deferring expensive initialization until actually needed.
pub struct LazyInit<T, F = fn() -> T> {
    /// The initialized value
    value: OnceLock<T>,
    /// The initialization function
    init: F,
}

impl<T, F: Fn() -> T> LazyInit<T, F> {
    /// Creates a new lazy initializer with the given initialization function
    pub const fn new(init: F) -> Self {
        Self {
            value: OnceLock::new(),
            init,
        }
    }

    /// Gets the value, initializing it if necessary
    pub fn get(&self) -> &T {
        self.value.get_or_init(&self.init)
    }

    /// Checks if the value has been initialized
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.value.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_lazy_init() {
        let counter = AtomicUsize::new(0);
        let lazy = LazyInit::new(|| {
            counter.fetch_add(1, Ordering::SeqCst);
            42
        });

        assert!(!lazy.is_initialized());
        assert_eq!(*lazy.get(), 42);
        assert!(lazy.is_initialized());
        assert_eq!(*lazy.get(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
