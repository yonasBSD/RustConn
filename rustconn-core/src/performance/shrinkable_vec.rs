//! Shrinkable wrapper for collections that can release excess capacity.

/// Shrinkable wrapper for collections that can release excess capacity
///
/// Wraps a Vec and periodically shrinks it to reduce memory usage.
pub struct ShrinkableVec<T> {
    /// Inner vector (pub for testing)
    pub inner: Vec<T>,
    /// Shrink threshold (shrink when capacity > len * threshold)
    shrink_threshold: f64,
    /// Minimum capacity to maintain
    min_capacity: usize,
}

impl<T> ShrinkableVec<T> {
    /// Creates a new shrinkable vector
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: Vec::new(),
            shrink_threshold: 2.0,
            min_capacity: 16,
        }
    }

    /// Creates with custom shrink settings
    #[must_use]
    pub const fn with_settings(shrink_threshold: f64, min_capacity: usize) -> Self {
        Self {
            inner: Vec::new(),
            shrink_threshold,
            min_capacity,
        }
    }

    /// Pushes an item
    pub fn push(&mut self, item: T) {
        self.inner.push(item);
    }

    /// Removes and returns the last item
    pub fn pop(&mut self) -> Option<T> {
        let result = self.inner.pop();
        self.maybe_shrink();
        result
    }

    /// Removes an item at index
    pub fn remove(&mut self, index: usize) -> T {
        let result = self.inner.remove(index);
        self.maybe_shrink();
        result
    }

    /// Clears the vector
    pub fn clear(&mut self) {
        self.inner.clear();
        self.maybe_shrink();
    }

    /// Returns the length
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the capacity
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns a slice
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    /// Returns a mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.inner
    }

    /// Forces a shrink operation
    pub fn shrink(&mut self) {
        let target = self.inner.len().max(self.min_capacity);
        self.inner.shrink_to(target);
    }

    /// Shrinks if capacity exceeds threshold
    fn maybe_shrink(&mut self) {
        let len = self.inner.len();
        let capacity = self.inner.capacity();

        if capacity > self.min_capacity && capacity as f64 > len as f64 * self.shrink_threshold {
            self.shrink();
        }
    }

    /// Returns the inner vector
    #[must_use]
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }

    /// Returns estimated memory savings from shrinking
    #[must_use]
    pub fn potential_savings(&self) -> usize {
        let excess = self
            .inner
            .capacity()
            .saturating_sub(self.inner.len().max(self.min_capacity));
        excess * std::mem::size_of::<T>()
    }
}

impl<T> Default for ShrinkableVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::ops::Deref for ShrinkableVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for ShrinkableVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shrinkable_vec_basic() {
        let mut vec: ShrinkableVec<i32> = ShrinkableVec::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);

        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);

        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_shrinkable_vec_shrink() {
        let mut vec: ShrinkableVec<i32> = ShrinkableVec::with_settings(1.5, 4);

        // Add many items
        for i in 0..100 {
            vec.push(i);
        }

        let initial_capacity = vec.capacity();

        // Clear and check shrinking
        vec.clear();
        vec.shrink();

        assert!(vec.capacity() < initial_capacity);
    }

    #[test]
    fn test_shrinkable_vec_potential_savings() {
        // Use settings that won't auto-shrink on clear
        let mut vec: ShrinkableVec<i32> = ShrinkableVec::with_settings(10.0, 4);

        for i in 0..100 {
            vec.push(i);
        }

        // Don't clear - just check potential savings with excess capacity
        // After pushing 100 items, capacity is likely > 100
        // Potential savings = (capacity - max(len, min_capacity)) * size_of::<i32>()
        // With 100 items and min_capacity=4, savings should be based on capacity - 100

        // Instead, let's test with a partially filled vec
        let mut vec2: ShrinkableVec<i32> = ShrinkableVec::with_settings(10.0, 4);
        vec2.inner.reserve(200); // Force large capacity
        vec2.push(1);
        vec2.push(2);

        let savings = vec2.potential_savings();
        // capacity is ~200, len is 2, min_capacity is 4
        // savings = (200 - 4) * 4 bytes = 784 bytes
        assert!(savings > 0, "Expected savings > 0, got {savings}");
    }
}
