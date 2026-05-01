//! Fallback connection strategy chain.
//!
//! Provides [`ConnectionFallback`] — a generic mechanism for trying multiple
//! connection strategies in priority order. If the first strategy fails,
//! the next one is attempted, and so on until one succeeds or all fail.
//!
//! # Design
//!
//! Inspired by Field Monitor's `LibvirtDynamicAdapter<T>` which tries
//! File Descriptor → Unix Socket → Network connections in sequence.
//! Adapted for RustConn's async architecture with `tracing` integration.
//!
//! # Example
//!
//! ```rust,ignore
//! use rustconn_core::connection::fallback::{ConnectionFallback, ConnectionStrategy};
//!
//! let fallback = ConnectionFallback::new("rdp")
//!     .add(IronRdpStrategy::new(&conn))
//!     .add(FreeRdpStrategy::new(&conn, "wlfreerdp3"))
//!     .add(FreeRdpStrategy::new(&conn, "xfreerdp"));
//!
//! match fallback.connect().await {
//!     Ok(session) => { /* connected via first available strategy */ }
//!     Err(report) => { /* all strategies failed */ }
//! }
//! ```

use std::fmt;
use std::pin::Pin;
use thiserror::Error;

/// Trait for a single connection strategy.
///
/// Each strategy represents one way to establish a connection
/// (e.g., IronRDP native, FreeRDP wlfreerdp, FreeRDP xfreerdp).
///
/// Strategies are tried in the order they are added to
/// [`ConnectionFallback`]. A strategy can declare itself unavailable
/// (e.g., binary not found) via [`is_available`](ConnectionStrategy::is_available),
/// in which case it is skipped without counting as a failure.
pub trait ConnectionStrategy: Send + Sync {
    /// The successful connection result type.
    type Output: Send;

    /// Human-readable name for logging (e.g., "IronRDP", "wlfreerdp3").
    fn name(&self) -> &str;

    /// Whether this strategy can be attempted right now.
    ///
    /// Return `false` if a required binary is missing, a feature flag
    /// is disabled, or a precondition is not met. Unavailable strategies
    /// are skipped silently (logged at debug level).
    fn is_available(&self) -> bool;

    /// Attempt to establish a connection.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error description on failure.
    fn connect(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Self::Output, String>> + Send + '_>>;
}

/// Record of a single failed strategy attempt.
#[derive(Debug, Clone)]
pub struct StrategyAttempt {
    /// Name of the strategy that was tried.
    pub strategy: String,
    /// Error message from the failed attempt.
    pub error: String,
}

impl fmt::Display for StrategyAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.strategy, self.error)
    }
}

/// Report of all failed attempts when the entire fallback chain fails.
#[derive(Debug, Clone, Error)]
#[error("{}", if self.attempts.is_empty() {
    format!("{}: no strategies available", self.context)
} else {
    let mut msg = format!("{}: all {} strategies failed", self.context, self.attempts.len());
    for (i, attempt) in self.attempts.iter().enumerate() {
        use std::fmt::Write;
        let _ = write!(msg, "\n  {}. {attempt}", i + 1);
    }
    msg
})]
pub struct FallbackError {
    /// Protocol or context name (e.g., "rdp", "vnc").
    pub context: String,
    /// All attempted strategies and their errors.
    pub attempts: Vec<StrategyAttempt>,
}

/// Ordered chain of connection strategies with automatic fallback.
///
/// Strategies are tried in insertion order. The first successful
/// connection is returned. If all strategies fail, a [`FallbackError`]
/// with all attempt details is returned.
pub struct ConnectionFallback<T: Send> {
    context: String,
    strategies: Vec<Box<dyn ConnectionStrategy<Output = T>>>,
}

impl<T: Send> ConnectionFallback<T> {
    /// Creates a new fallback chain with the given context name.
    ///
    /// The context is used in log messages and error reports
    /// (e.g., "rdp", "vnc", "spice").
    pub fn new(context: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            strategies: Vec::new(),
        }
    }

    /// Adds a strategy to the end of the chain (lowest priority).
    ///
    /// Strategies are tried in the order they are added.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, strategy: impl ConnectionStrategy<Output = T> + 'static) -> Self {
        self.strategies.push(Box::new(strategy));
        self
    }

    /// Returns the number of registered strategies.
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    /// Returns the number of currently available strategies.
    pub fn available_count(&self) -> usize {
        self.strategies.iter().filter(|s| s.is_available()).count()
    }

    /// Returns the names of all registered strategies.
    pub fn strategy_names(&self) -> Vec<&str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }

    /// Tries each strategy in order until one succeeds.
    ///
    /// Unavailable strategies are skipped. Failed strategies are logged
    /// and recorded in the error report.
    ///
    /// # Errors
    ///
    /// Returns [`FallbackError`] if all strategies fail or none are available.
    pub async fn connect(&self) -> Result<T, FallbackError> {
        let mut attempts = Vec::new();

        for strategy in &self.strategies {
            if !strategy.is_available() {
                tracing::debug!(
                    context = %self.context,
                    strategy = strategy.name(),
                    "Strategy unavailable, skipping"
                );
                continue;
            }

            tracing::info!(
                context = %self.context,
                strategy = strategy.name(),
                "Attempting connection"
            );

            match strategy.connect().await {
                Ok(result) => {
                    tracing::info!(
                        context = %self.context,
                        strategy = strategy.name(),
                        "Connection successful"
                    );
                    return Ok(result);
                }
                Err(error) => {
                    tracing::warn!(
                        context = %self.context,
                        strategy = strategy.name(),
                        %error,
                        "Strategy failed, trying next"
                    );
                    attempts.push(StrategyAttempt {
                        strategy: strategy.name().to_string(),
                        error,
                    });
                }
            }
        }

        Err(FallbackError {
            context: self.context.clone(),
            attempts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysSucceeds {
        name: String,
        value: u32,
    }

    impl ConnectionStrategy for AlwaysSucceeds {
        type Output = u32;

        fn name(&self) -> &str {
            &self.name
        }

        fn is_available(&self) -> bool {
            true
        }

        fn connect(
            &self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<u32, String>> + Send + '_>> {
            let value = self.value;
            Box::pin(async move { Ok(value) })
        }
    }

    struct AlwaysFails {
        name: String,
    }

    impl ConnectionStrategy for AlwaysFails {
        type Output = u32;

        fn name(&self) -> &str {
            &self.name
        }

        fn is_available(&self) -> bool {
            true
        }

        fn connect(
            &self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<u32, String>> + Send + '_>> {
            let name = self.name.clone();
            Box::pin(async move { Err(format!("{name} failed")) })
        }
    }

    struct Unavailable;

    impl ConnectionStrategy for Unavailable {
        type Output = u32;

        fn name(&self) -> &str {
            "unavailable"
        }

        fn is_available(&self) -> bool {
            false
        }

        fn connect(
            &self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<u32, String>> + Send + '_>> {
            Box::pin(async { unreachable!("should not be called") })
        }
    }

    #[tokio::test]
    async fn test_first_strategy_succeeds() {
        let fallback = ConnectionFallback::new("test")
            .add(AlwaysSucceeds {
                name: "first".into(),
                value: 42,
            })
            .add(AlwaysSucceeds {
                name: "second".into(),
                value: 99,
            });

        let result = fallback.connect().await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_fallback_to_second() {
        let fallback = ConnectionFallback::new("test")
            .add(AlwaysFails {
                name: "first".into(),
            })
            .add(AlwaysSucceeds {
                name: "second".into(),
                value: 99,
            });

        let result = fallback.connect().await;
        assert_eq!(result.unwrap(), 99);
    }

    #[tokio::test]
    async fn test_all_fail() {
        let fallback: ConnectionFallback<u32> = ConnectionFallback::new("test")
            .add(AlwaysFails {
                name: "first".into(),
            })
            .add(AlwaysFails {
                name: "second".into(),
            });

        let err = fallback.connect().await.unwrap_err();
        assert_eq!(err.context, "test");
        assert_eq!(err.attempts.len(), 2);
        assert_eq!(err.attempts[0].strategy, "first");
        assert_eq!(err.attempts[1].strategy, "second");
    }

    #[tokio::test]
    async fn test_skip_unavailable() {
        let fallback = ConnectionFallback::new("test")
            .add(Unavailable)
            .add(AlwaysSucceeds {
                name: "available".into(),
                value: 7,
            });

        let result = fallback.connect().await;
        assert_eq!(result.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_no_strategies() {
        let fallback: ConnectionFallback<u32> = ConnectionFallback::new("empty");

        let err = fallback.connect().await.unwrap_err();
        assert!(err.attempts.is_empty());
        assert!(err.to_string().contains("no strategies available"));
    }

    #[tokio::test]
    async fn test_all_unavailable() {
        let fallback: ConnectionFallback<u32> = ConnectionFallback::new("test").add(Unavailable);

        let err = fallback.connect().await.unwrap_err();
        assert!(err.attempts.is_empty());
    }

    #[test]
    fn test_strategy_count() {
        let fallback: ConnectionFallback<u32> = ConnectionFallback::new("test")
            .add(AlwaysSucceeds {
                name: "a".into(),
                value: 1,
            })
            .add(AlwaysFails { name: "b".into() })
            .add(Unavailable);

        assert_eq!(fallback.strategy_count(), 3);
        assert_eq!(fallback.available_count(), 2);
        assert_eq!(fallback.strategy_names(), vec!["a", "b", "unavailable"]);
    }

    #[test]
    fn test_fallback_error_display() {
        let err = FallbackError {
            context: "rdp".into(),
            attempts: vec![
                StrategyAttempt {
                    strategy: "IronRDP".into(),
                    error: "TLS handshake failed".into(),
                },
                StrategyAttempt {
                    strategy: "wlfreerdp3".into(),
                    error: "command not found".into(),
                },
            ],
        };

        let display = err.to_string();
        assert!(display.contains("rdp: all 2 strategies failed"));
        assert!(display.contains("IronRDP: TLS handshake failed"));
        assert!(display.contains("wlfreerdp3: command not found"));
    }
}
