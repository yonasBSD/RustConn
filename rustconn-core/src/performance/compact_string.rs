//! Compact string storage for frequently repeated short strings.

use std::hash::{Hash, Hasher};

/// Compact string storage for frequently repeated short strings
///
/// Uses a small inline buffer for short strings to avoid heap allocation.
/// Strings longer than the inline capacity fall back to heap allocation.
#[derive(Clone)]
pub struct CompactString {
    /// Storage: either inline bytes or heap-allocated String
    storage: CompactStringStorage,
}

#[derive(Clone)]
enum CompactStringStorage {
    /// Inline storage for short strings (up to 23 bytes on 64-bit)
    Inline {
        /// Length of the string
        len: u8,
        /// Inline buffer storing valid UTF-8
        buf: [u8; 23],
    },
    /// Heap-allocated string for longer strings
    Heap(String),
}

impl CompactString {
    /// Maximum length for inline storage
    pub const INLINE_CAPACITY: usize = 23;

    /// Creates a new compact string
    #[must_use]
    pub fn new(s: &str) -> Self {
        if s.len() <= Self::INLINE_CAPACITY {
            let mut buf = [0u8; 23];
            buf[..s.len()].copy_from_slice(s.as_bytes());
            Self {
                storage: CompactStringStorage::Inline {
                    len: s.len() as u8,
                    buf,
                },
            }
        } else {
            Self {
                storage: CompactStringStorage::Heap(s.to_string()),
            }
        }
    }

    /// Returns the string as a slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.storage {
            CompactStringStorage::Inline { len, buf } => {
                // We only store valid UTF-8 in the buffer (from str input)
                // This unwrap is safe because we only copy from valid UTF-8 strings
                std::str::from_utf8(&buf[..*len as usize])
                    .expect("CompactString buffer should contain valid UTF-8")
            }
            CompactStringStorage::Heap(s) => s,
        }
    }

    /// Returns true if the string is stored inline
    #[must_use]
    pub const fn is_inline(&self) -> bool {
        matches!(self.storage, CompactStringStorage::Inline { .. })
    }

    /// Returns the length of the string
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.storage {
            CompactStringStorage::Inline { len, .. } => *len as usize,
            CompactStringStorage::Heap(s) => s.len(),
        }
    }

    /// Returns true if the string is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::fmt::Debug for CompactString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactString")
            .field("value", &self.as_str())
            .field("inline", &self.is_inline())
            .finish()
    }
}

impl std::fmt::Display for CompactString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PartialEq for CompactString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for CompactString {}

impl Hash for CompactString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl From<&str> for CompactString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for CompactString {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

impl AsRef<str> for CompactString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_string_inline() {
        let short = CompactString::new("hello");
        assert!(short.is_inline());
        assert_eq!(short.as_str(), "hello");
        assert_eq!(short.len(), 5);
    }

    #[test]
    fn test_compact_string_heap() {
        let long = CompactString::new("this is a very long string that exceeds inline capacity");
        assert!(!long.is_inline());
        assert_eq!(
            long.as_str(),
            "this is a very long string that exceeds inline capacity"
        );
    }

    #[test]
    fn test_compact_string_equality() {
        let s1 = CompactString::new("test");
        let s2 = CompactString::new("test");
        let s3 = CompactString::new("other");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_compact_string_empty() {
        let empty = CompactString::new("");
        assert!(empty.is_empty());
        assert!(empty.is_inline());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_compact_string_max_inline() {
        // Test string at exactly inline capacity
        let max_inline = "a".repeat(CompactString::INLINE_CAPACITY);
        let s = CompactString::new(&max_inline);
        assert!(s.is_inline());
        assert_eq!(s.len(), CompactString::INLINE_CAPACITY);

        // Test string just over inline capacity
        let over_inline = "a".repeat(CompactString::INLINE_CAPACITY + 1);
        let s2 = CompactString::new(&over_inline);
        assert!(!s2.is_inline());
    }
}
