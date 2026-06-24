//! Progress reporting utilities.
//!
//! This module provides a flexible and efficient progress reporting system,
//! including throttling, composition, and async channel support.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::Mutex;

// ============================================================================
//  Progress State
// ============================================================================

/// Represents a progress update.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportState {
    /// Current progress value (e.g., bytes downloaded, files processed).
    pub current: u64,
    /// Total expected value, if known.
    pub total: Option<u64>,
    /// Optional human-readable message describing the current phase.
    pub message: Option<String>,
    /// Speed in units per second (e.g., bytes/s), if calculable.
    pub speed: Option<f64>,
    /// Estimated time remaining, if calculable.
    pub eta: Option<Duration>,
}

impl ReportState {
    /// Creates a new progress state with only current and total.
    pub fn new(current: u64, total: Option<u64>) -> Self {
        Self {
            current,
            total,
            message: None,
            speed: None,
            eta: None,
        }
    }

    /// Sets a message.
    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    /// Sets speed.
    pub fn with_speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// Sets ETA.
    pub fn with_eta(mut self, eta: Duration) -> Self {
        self.eta = Some(eta);
        self
    }
}

// ============================================================================
//  Reporter Trait
// ============================================================================

/// A trait for reporting progress.
///
/// Implementations should be cheap to call and not block significantly.
pub trait Reporter: Send + Sync {
    /// Reports a progress update.
    ///
    /// This method should not panic; if it fails, it should log the error
    /// and continue.
    fn report(&self, state: &ReportState);

    /// Called when the task is finished successfully.
    fn finish(&self) {
        // Default: do nothing.
    }

    /// Called when the task is aborted due to error or cancellation.
    fn abort(&self) {
        // Default: do nothing.
    }
}

// ============================================================================
//  No-Op Reporter (NR)
// ============================================================================

/// A reporter that does nothing.
///
/// This is useful as a placeholder when progress reporting is not needed.
pub struct NoopReporter;

impl Reporter for NoopReporter {
    fn report(&self, _state: &ReportState) {}
}

/// Constant instance of the no-op reporter.
pub const NR: NoopReporter = NoopReporter;

// ============================================================================
//  Throttled Reporter
// ============================================================================

/// A reporter that limits the frequency of updates.
///
/// It accumulates the most recent state and only forwards it at most once
/// per `min_interval` duration.
pub struct ThrottledReporter<R: Reporter> {
    inner: R,
    min_interval: Duration,
    last_report: Mutex<Option<Instant>>,
    pending: Mutex<Option<ReportState>>,
}

impl<R: Reporter> ThrottledReporter<R> {
    pub fn new(inner: R, min_interval: Duration) -> Self {
        Self {
            inner,
            min_interval,
            last_report: Mutex::new(None),
            pending: Mutex::new(None),
        }
    }

    /// Returns the inner reporter.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Reporter> Reporter for ThrottledReporter<R> {
    fn report(&self, state: &ReportState) {
        // Store the state in pending, then attempt to flush if enough time has passed.
        let now = Instant::now();
        let mut pending_lock = self.pending.blocking_lock();
        *pending_lock = Some(state.clone());
        drop(pending_lock);

        let mut last_lock = self.last_report.blocking_lock();
        let should_report = match *last_lock {
            Some(prev) => now - prev >= self.min_interval,
            None => true,
        };

        if should_report {
            // Take the pending state and send it.
            let mut pending_lock2 = self.pending.blocking_lock();
            if let Some(st) = pending_lock2.take() {
                self.inner.report(&st);
                *last_lock = Some(now);
            }
        }
    }

    fn finish(&self) {
        // Flush pending state one last time.
        let mut pending_lock = self.pending.blocking_lock();
        if let Some(st) = pending_lock.take() {
            self.inner.report(&st);
        }
        self.inner.finish();
    }

    fn abort(&self) {
        let mut pending_lock = self.pending.blocking_lock();
        *pending_lock = None; // discard pending
        self.inner.abort();
    }
}

// ============================================================================
//  Composed Reporter
// ============================================================================

/// A reporter that forwards updates to multiple inner reporters.
pub struct MultiReporter {
    reporters: Vec<Box<dyn Reporter>>,
}

impl MultiReporter {
    pub fn new(reporters: Vec<Box<dyn Reporter>>) -> Self {
        Self { reporters }
    }
}

impl Reporter for MultiReporter {
    fn report(&self, state: &ReportState) {
        for r in &self.reporters {
            r.report(state);
        }
    }

    fn finish(&self) {
        for r in &self.reporters {
            r.finish();
        }
    }

    fn abort(&self) {
        for r in &self.reporters {
            r.abort();
        }
    }
}

// ============================================================================
//  Async Channel Reporter
// ============================================================================

/// A reporter that sends progress updates through a tokio unbounded channel.
///
/// This allows progress to be processed asynchronously by a separate task,
/// preventing blocking of the worker thread.
pub struct ChannelReporter {
    sender: UnboundedSender<ReportState>,
}

impl ChannelReporter {
    pub fn new(sender: UnboundedSender<ReportState>) -> Self {
        Self { sender }
    }
}

impl Reporter for ChannelReporter {
    fn report(&self, state: &ReportState) {
        let _ = self.sender.send(state.clone()); // ignore send errors
    }
}

// ============================================================================
//  Logging Reporter
// ============================================================================

/// A reporter that logs progress at the debug level.
pub struct LogReporter {
    target: String,
}

impl LogReporter {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }
}

impl Reporter for LogReporter {
    fn report(&self, state: &ReportState) {
        log::debug!(
            target: &self.target,
            "Progress: {} / {:?} - {:?} - speed: {:?} - ETA: {:?}",
            state.current,
            state.total,
            state.message,
            state.speed,
            state.eta
        );
    }
}

// ============================================================================
//  Convenience Functions
// ============================================================================

/// Creates a reporter that prints progress to stdout.
pub fn console_reporter() -> impl Reporter {
    struct Console;
    impl Reporter for Console {
        fn report(&self, state: &ReportState) {
            let total = state.total.map(|t| format!("/{}", t)).unwrap_or_default();
            let msg = state.message.as_deref().unwrap_or("");
            println!("\r{}: {}{}   ", msg, state.current, total);
        }
        fn finish(&self) {
            println!("\nDone.");
        }
    }
    Console
}

/// Creates a throttled version of any reporter.
pub fn throttle<R: Reporter>(reporter: R, min_interval: Duration) -> ThrottledReporter<R> {
    ThrottledReporter::new(reporter, min_interval)
}

/// Creates a multi-reporter from a list of reporters.
pub fn combine(reporters: Vec<Box<dyn Reporter>>) -> MultiReporter {
    MultiReporter::new(reporters)
}

// ============================================================================
//  Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct CountingReporter {
        count: Arc<AtomicU64>,
    }

    impl CountingReporter {
        fn new() -> Self {
            Self {
                count: Arc::new(AtomicU64::new(0)),
            }
        }
        fn get_count(&self) -> u64 {
            self.count.load(Ordering::SeqCst)
        }
    }

    impl Reporter for CountingReporter {
        fn report(&self, _state: &ReportState) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_throttle() {
        let inner = CountingReporter::new();
        let throttled = ThrottledReporter::new(inner, Duration::from_millis(100));

        for _ in 0..10 {
            throttled.report(&ReportState::new(1, None));
        }
        std::thread::sleep(Duration::from_millis(150));
        throttled.report(&ReportState::new(2, None));
        throttled.finish();

        // Should have reported at most 2 times (one after 100ms, one at finish)
        let count = throttled.into_inner().get_count();
        assert!(count <= 2);
    }

    #[test]
    fn test_multi_reporter() {
        let r1 = CountingReporter::new();
        let r2 = CountingReporter::new();
        let multi = MultiReporter::new(vec![Box::new(r1), Box::new(r2)]);

        multi.report(&ReportState::new(1, None));
        assert_eq!(multi.reporters[0].get_count(), 1);
        assert_eq!(multi.reporters[1].get_count(), 1);
    }

    #[tokio::test]
    async fn test_channel_reporter() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let reporter = ChannelReporter::new(tx);

        reporter.report(&ReportState::new(42, Some(100)));
        let received = rx.recv().await.unwrap();
        assert_eq!(received.current, 42);
        assert_eq!(received.total, Some(100));
    }
}