//! Batch processor for optimizing bulk operations.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::lock_mutex;

/// Batch processor for optimizing bulk operations
///
/// Collects items and processes them in batches to reduce overhead.
pub struct BatchProcessor<T> {
    /// Items waiting to be processed
    items: Arc<Mutex<Vec<T>>>,
    /// Maximum batch size
    max_batch_size: usize,
    /// Maximum wait time before processing
    max_wait: Duration,
    /// Last flush time
    last_flush: Arc<Mutex<Instant>>,
}

impl<T> BatchProcessor<T> {
    /// Creates a new batch processor
    #[must_use]
    pub fn new(max_batch_size: usize, max_wait: Duration) -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::with_capacity(max_batch_size))),
            max_batch_size,
            max_wait,
            last_flush: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Adds an item to the batch
    ///
    /// Returns `Some(Vec<T>)` if the batch should be processed now.
    pub fn add(&self, item: T) -> Option<Vec<T>> {
        let mut items = lock_mutex(&self.items, "batch_items")?;
        items.push(item);

        let time_exceeded = lock_mutex(&self.last_flush, "batch_flush")
            .is_some_and(|lf| lf.elapsed() >= self.max_wait);
        let should_flush = items.len() >= self.max_batch_size || time_exceeded;

        if should_flush {
            let batch = std::mem::take(&mut *items);
            drop(items);
            if let Some(mut lf) = lock_mutex(&self.last_flush, "batch_flush") {
                *lf = Instant::now();
            }
            Some(batch)
        } else {
            None
        }
    }

    /// Forces a flush of all pending items
    #[must_use]
    pub fn flush(&self) -> Vec<T> {
        let batch = lock_mutex(&self.items, "batch_items")
            .map(|mut items| std::mem::take(&mut *items))
            .unwrap_or_default();
        if let Some(mut lf) = lock_mutex(&self.last_flush, "batch_flush") {
            *lf = Instant::now();
        }
        batch
    }

    /// Returns the number of pending items
    #[must_use]
    pub fn pending_count(&self) -> usize {
        lock_mutex(&self.items, "batch_items")
            .map(|i| i.len())
            .unwrap_or(0)
    }

    /// Checks if there are pending items
    #[must_use]
    pub fn has_pending(&self) -> bool {
        lock_mutex(&self.items, "batch_items").is_some_and(|i| !i.is_empty())
    }
}

impl<T> Default for BatchProcessor<T> {
    fn default() -> Self {
        Self::new(100, Duration::from_millis(50))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_processor() {
        let processor: BatchProcessor<i32> = BatchProcessor::new(3, Duration::from_secs(10));

        assert!(processor.add(1).is_none());
        assert!(processor.add(2).is_none());

        let batch = processor.add(3);
        assert!(batch.is_some());
        assert_eq!(batch.unwrap(), vec![1, 2, 3]);

        processor.add(4);
        let remaining = processor.flush();
        assert_eq!(remaining, vec![4]);
    }
}
