//! Configurable retry strategy for transient AI provider failures.
//!
//! [`RetryConfig`] describes how many retries to attempt, the initial and
//! maximum back-off delays, and which error messages are considered retryable.
//! The standalone [`execute_with_retry`] function wraps any fallible closure
//! with this policy.

use std::thread;
use std::time::Duration;

use anyhow::Result;
use tracing::{debug, warn};

// ── RetryConfig ───────────────────────────────────────────────────────────────

/// Configurable retry policy for AI provider calls.
///
/// # Defaults
///
/// | Field               | Value  | Meaning                                    |
/// |---------------------|--------|--------------------------------------------|
/// | `max_attempts`      | `3`    | Up to 3 total attempts (2 retries)         |
/// | `initial_delay_ms`  | `500`  | First retry waits 500 ms                   |
/// | `max_delay_ms`      | `5000` | Back-off capped at 5 s                     |
/// | `backoff_factor`    | `2.0`  | Delay doubles after each failed attempt    |
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Total number of attempts (including the first).  Setting this to `1`
    /// disables retries.
    pub max_attempts: u32,
    /// Milliseconds to wait before the first retry.
    pub initial_delay_ms: u64,
    /// Maximum milliseconds to wait between retries (back-off ceiling).
    pub max_delay_ms: u64,
    /// Multiplicative factor applied to the delay after each failed attempt.
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 500,
            max_delay_ms: 5_000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    /// Disable retries (attempt exactly once).
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Return the delay (in milliseconds) to apply before attempt `n`
    /// (zero-indexed; attempt 0 never waits).
    ///
    /// The returned value is `min(initial_delay_ms × backoff_factor^(n-1), max_delay_ms)`.
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return 0;
        }
        let delay = self.initial_delay_ms as f64
            * self.backoff_factor.powi(attempt as i32 - 1);
        (delay as u64).min(self.max_delay_ms)
    }
}

// ── Retryable error classification ────────────────────────────────────────────

/// Return `true` when the error message suggests a transient failure that
/// may succeed on retry.
///
/// Recognised patterns:
/// * `"connection refused"` – provider not running yet
/// * `"timeout"` / `"timed out"` – provider overloaded
/// * `"rate limit"` / `"429"` – rate-limited
/// * `"503"` / `"service unavailable"` – provider temporarily unavailable
/// * `"502"` / `"bad gateway"` – upstream issue
/// * `"500"` / `"internal server error"` – transient backend error
pub fn is_retryable(error: &anyhow::Error) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("connection refused")
        || msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("rate limit")
        || msg.contains("429")
        || msg.contains("503")
        || msg.contains("service unavailable")
        || msg.contains("502")
        || msg.contains("bad gateway")
        || msg.contains("500")
        || msg.contains("internal server error")
}

// ── execute_with_retry ────────────────────────────────────────────────────────

/// Execute `f` with the retry policy described by `config`.
///
/// On a transient failure (as determined by [`is_retryable`]), waits for the
/// back-off interval and tries again.  Non-retryable errors are returned
/// immediately without further attempts.
///
/// # Returns
///
/// * `Ok(T)` – the closure succeeded.
/// * `Err(e)` – all attempts exhausted or a non-retryable error occurred.
pub fn execute_with_retry<T, F>(config: &RetryConfig, context: &str, mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        let delay_ms = config.delay_for_attempt(attempt);
        if delay_ms > 0 {
            debug!(
                context = %context,
                attempt = %attempt,
                delay_ms = %delay_ms,
                "Retrying AI provider call after back-off"
            );
            thread::sleep(Duration::from_millis(delay_ms));
        }

        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                if is_retryable(&e) && attempt + 1 < config.max_attempts {
                    warn!(
                        context = %context,
                        attempt = %attempt,
                        error = %e,
                        "AI provider call failed (retryable); will retry"
                    );
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // All attempts exhausted.
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("all retry attempts exhausted")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // ── RetryConfig ───────────────────────────────────────────────────────────

    #[test]
    fn test_default_config() {
        let c = RetryConfig::default();
        assert_eq!(c.max_attempts, 3);
        assert_eq!(c.initial_delay_ms, 500);
        assert_eq!(c.max_delay_ms, 5_000);
    }

    #[test]
    fn test_no_retry_config() {
        let c = RetryConfig::no_retry();
        assert_eq!(c.max_attempts, 1);
    }

    #[test]
    fn test_delay_for_attempt_zero() {
        let c = RetryConfig::default();
        assert_eq!(c.delay_for_attempt(0), 0);
    }

    #[test]
    fn test_delay_for_attempt_backoff() {
        let c = RetryConfig {
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            backoff_factor: 2.0,
            max_attempts: 5,
        };
        assert_eq!(c.delay_for_attempt(1), 100);
        assert_eq!(c.delay_for_attempt(2), 200);
        assert_eq!(c.delay_for_attempt(3), 400);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let c = RetryConfig {
            initial_delay_ms: 1_000,
            max_delay_ms: 1_500,
            backoff_factor: 2.0,
            max_attempts: 5,
        };
        // 1000 * 2^1 = 2000 → capped at 1500
        assert_eq!(c.delay_for_attempt(2), 1_500);
    }

    // ── is_retryable ──────────────────────────────────────────────────────────

    #[test]
    fn test_retryable_connection_refused() {
        let e = anyhow::anyhow!("connection refused");
        assert!(is_retryable(&e));
    }

    #[test]
    fn test_retryable_timeout() {
        let e = anyhow::anyhow!("request timed out");
        assert!(is_retryable(&e));
    }

    #[test]
    fn test_retryable_rate_limit() {
        let e = anyhow::anyhow!("rate limit exceeded");
        assert!(is_retryable(&e));
    }

    #[test]
    fn test_not_retryable_for_auth_error() {
        let e = anyhow::anyhow!("401 unauthorized");
        assert!(!is_retryable(&e));
    }

    #[test]
    fn test_not_retryable_for_bad_input() {
        let e = anyhow::anyhow!("missing required field 'model'");
        assert!(!is_retryable(&e));
    }

    // ── execute_with_retry ────────────────────────────────────────────────────

    #[test]
    fn test_succeeds_on_first_attempt() {
        let config = RetryConfig::no_retry();
        let result = execute_with_retry(&config, "test", || Ok::<i32, anyhow::Error>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_succeeds_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 0, // no sleep in tests
            max_delay_ms: 0,
            backoff_factor: 1.0,
        };

        let result = execute_with_retry(&config, "test", move || {
            let n = attempts_clone.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Err(anyhow::anyhow!("connection refused"))
            } else {
                Ok::<&str, anyhow::Error>("success")
            }
        });

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_fails_after_max_attempts() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            backoff_factor: 1.0,
        };

        let result = execute_with_retry(&config, "test", move || {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(anyhow::anyhow!("connection refused"))
        });

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_non_retryable_error_fails_immediately() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let config = RetryConfig {
            max_attempts: 5,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            backoff_factor: 1.0,
        };

        let result = execute_with_retry(&config, "test", move || {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(anyhow::anyhow!("401 unauthorized: bad API key"))
        });

        assert!(result.is_err());
        // Only 1 attempt because the error is not retryable
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
