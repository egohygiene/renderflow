//! Execution metrics collected during AI transform execution.
//!
//! [`AiExecutionMetrics`] accumulates statistics across all AI calls made
//! within a single build run.  The metrics can be printed in execution
//! summaries to give users visibility into cost, performance, and cache
//! efficiency.

use std::fmt;
use std::sync::{Arc, Mutex};

// ── AiExecutionMetrics ────────────────────────────────────────────────────────

/// Per-run statistics accumulated during AI transform execution.
///
/// Construct one with [`AiExecutionMetrics::new`] (or `default()`) and share
/// it across transforms via an `Arc<Mutex<AiExecutionMetrics>>`.  After the
/// build completes, call [`AiExecutionMetrics::summary`] to produce a
/// human-readable report.
#[derive(Debug, Default, Clone)]
pub struct AiExecutionMetrics {
    /// Total number of AI provider calls made (cache misses + bypasses).
    pub request_count: u64,
    /// Number of AI cache hits (calls avoided by the cache).
    pub cache_hits: u64,
    /// Number of AI cache misses (calls that reached the provider).
    pub cache_misses: u64,
    /// Estimated total input tokens consumed across all calls.
    pub total_input_tokens: u64,
    /// Estimated total output tokens produced across all calls.
    pub total_output_tokens: u64,
    /// Total wall-clock milliseconds spent waiting for provider responses.
    pub total_duration_ms: u64,
}

impl AiExecutionMetrics {
    /// Create a new empty metrics instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single provider call.
    ///
    /// `input_tokens` and `output_tokens` are `None` when the backend does not
    /// report token counts.  `duration_ms` is the wall-clock time for the call.
    pub fn record_call(
        &mut self,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        duration_ms: u64,
    ) {
        self.request_count += 1;
        self.cache_misses += 1;
        if let Some(t) = input_tokens {
            self.total_input_tokens += u64::from(t);
        }
        if let Some(t) = output_tokens {
            self.total_output_tokens += u64::from(t);
        }
        self.total_duration_ms += duration_ms;
    }

    /// Record a cache hit (no provider call was made).
    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    /// Return a human-readable summary string suitable for printing at the end
    /// of a build run.
    ///
    /// # Example output
    ///
    /// ```text
    /// AI Execution Summary:
    ///   Requests:       3
    ///   Cache hits:     1
    ///   Cache misses:   2
    ///   Input tokens:   512
    ///   Output tokens:  128
    ///   Total time:     1450 ms
    /// ```
    pub fn summary(&self) -> String {
        let mut lines = vec![String::from("AI Execution Summary:")];
        lines.push(format!("  Requests:       {}", self.request_count));
        lines.push(format!("  Cache hits:     {}", self.cache_hits));
        lines.push(format!("  Cache misses:   {}", self.cache_misses));
        if self.total_input_tokens > 0 || self.total_output_tokens > 0 {
            lines.push(format!("  Input tokens:   {}", self.total_input_tokens));
            lines.push(format!("  Output tokens:  {}", self.total_output_tokens));
        }
        lines.push(format!("  Total time:     {} ms", self.total_duration_ms));
        lines.join("\n")
    }

    /// Return `true` when any AI calls were made or any cache hits recorded.
    pub fn has_activity(&self) -> bool {
        self.request_count > 0 || self.cache_hits > 0
    }
}

impl fmt::Display for AiExecutionMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ── SharedMetrics ─────────────────────────────────────────────────────────────

/// A thread-safe, clonable handle to a shared [`AiExecutionMetrics`] instance.
///
/// Pass this to each [`AiTransform`](crate::transforms::ai::AiTransform) so
/// that metrics are aggregated across all parallel transform executions.
///
/// [`AiTransform`]: crate::transforms::ai::AiTransform
#[derive(Debug, Clone, Default)]
pub struct SharedMetrics(Arc<Mutex<AiExecutionMetrics>>);

impl SharedMetrics {
    /// Create a new, empty shared metrics handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a provider call.
    pub fn record_call(&self, input_tokens: Option<u32>, output_tokens: Option<u32>, duration_ms: u64) {
        if let Ok(mut guard) = self.0.lock() {
            guard.record_call(input_tokens, output_tokens, duration_ms);
        }
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        if let Ok(mut guard) = self.0.lock() {
            guard.record_cache_hit();
        }
    }

    /// Retrieve a snapshot of the current metrics.
    pub fn snapshot(&self) -> AiExecutionMetrics {
        self.0
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_default_is_zero() {
        let m = AiExecutionMetrics::new();
        assert_eq!(m.request_count, 0);
        assert_eq!(m.cache_hits, 0);
        assert_eq!(m.cache_misses, 0);
        assert!(!m.has_activity());
    }

    #[test]
    fn test_record_call_increments_counters() {
        let mut m = AiExecutionMetrics::new();
        m.record_call(Some(100), Some(50), 200);
        assert_eq!(m.request_count, 1);
        assert_eq!(m.cache_misses, 1);
        assert_eq!(m.total_input_tokens, 100);
        assert_eq!(m.total_output_tokens, 50);
        assert_eq!(m.total_duration_ms, 200);
        assert!(m.has_activity());
    }

    #[test]
    fn test_record_call_accumulates() {
        let mut m = AiExecutionMetrics::new();
        m.record_call(Some(10), Some(5), 100);
        m.record_call(Some(20), Some(10), 200);
        assert_eq!(m.request_count, 2);
        assert_eq!(m.total_input_tokens, 30);
        assert_eq!(m.total_output_tokens, 15);
        assert_eq!(m.total_duration_ms, 300);
    }

    #[test]
    fn test_record_call_handles_none_tokens() {
        let mut m = AiExecutionMetrics::new();
        m.record_call(None, None, 50);
        assert_eq!(m.request_count, 1);
        assert_eq!(m.total_input_tokens, 0);
        assert_eq!(m.total_output_tokens, 0);
    }

    #[test]
    fn test_record_cache_hit() {
        let mut m = AiExecutionMetrics::new();
        m.record_cache_hit();
        assert_eq!(m.cache_hits, 1);
        assert_eq!(m.request_count, 0);
        assert!(m.has_activity());
    }

    #[test]
    fn test_summary_contains_fields() {
        let mut m = AiExecutionMetrics::new();
        m.record_call(Some(10), Some(5), 100);
        m.record_cache_hit();
        let s = m.summary();
        assert!(s.contains("Requests:"));
        assert!(s.contains("Cache hits:     1"));
        assert!(s.contains("Cache misses:   1"));
        assert!(s.contains("Total time:     100 ms"));
    }

    #[test]
    fn test_shared_metrics_thread_safe() {
        let shared = SharedMetrics::new();
        shared.record_call(Some(5), Some(3), 10);
        shared.record_cache_hit();
        let snap = shared.snapshot();
        assert_eq!(snap.request_count, 1);
        assert_eq!(snap.cache_hits, 1);
    }
}
