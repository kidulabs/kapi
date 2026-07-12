//! Work queue with deduplication and exponential-backoff retry.
//!
//! [`WorkQueue`] provides a FIFO queue where each item is identified by a
//! [`QueueKey`].  Duplicate adds are silently ignored while the key is
//! already pending.  Failed items are re-queued after an exponential backoff
//! (1 s, 2 s, 4 s, … capped at 5 min).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use kapi_core::ResourceKey;
use tokio::sync::{Mutex, Notify};
use tracing::warn;

/// Unique identifier for a work-queue item.
///
/// This is essentially a fully-qualified object reference (kind + name +
/// optional namespace), sufficient to fetch the object from the API server.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct QueueKey {
    pub key: ResourceKey,
    pub name: String,
    pub namespace: Option<String>,
}

impl QueueKey {
    pub fn new(key: ResourceKey, name: impl Into<String>, namespace: Option<String>) -> Self {
        QueueKey { key, name: name.into(), namespace }
    }
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct QueueState {
    /// Set of keys currently in the queue (for O(1) dedup).
    pending: HashSet<QueueKey>,
    /// FIFO ordering of pending keys.
    queue: VecDeque<QueueKey>,
    /// Number of consecutive failures per key (for backoff).
    retry_count: HashMap<QueueKey, u32>,
}

impl QueueState {
    fn new() -> Self {
        QueueState { pending: HashSet::new(), queue: VecDeque::new(), retry_count: HashMap::new() }
    }
}

// ---------------------------------------------------------------------------
// WorkQueue
// ---------------------------------------------------------------------------

/// A FIFO work queue with deduplication and exponential backoff.
///
/// # Usage
///
/// ```ignore
/// let wq = WorkQueue::new();
/// wq.add(some_key).await;
/// let key = wq.get().await;   // blocks until an item is available
/// // … process key …
/// wq.done(key, true).await;   // success → no retry
/// ```
pub struct WorkQueue {
    state: Arc<Mutex<QueueState>>,
    notify: Arc<Notify>,
}

impl WorkQueue {
    /// Creates an empty work queue.
    pub fn new() -> Self {
        WorkQueue {
            state: Arc::new(Mutex::new(QueueState::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Adds a key to the queue.
    ///
    /// If the key is already pending (has been added but not yet processed
    /// via [`get`](Self::get)), this call is a no-op.
    pub async fn add(&self, key: QueueKey) {
        let mut state = self.state.lock().await;
        if state.pending.insert(key.clone()) {
            state.queue.push_back(key);
            self.notify.notify_one();
        }
    }

    /// Retrieves the next key, blocking until one is available.
    ///
    /// The returned key is removed from the pending set so it can be
    /// re-queued later if needed.
    pub async fn get(&self) -> QueueKey {
        loop {
            let mut state = self.state.lock().await;
            if let Some(key) = state.queue.pop_front() {
                state.pending.remove(&key);
                return key;
            }
            // Release the lock and wait for a notification.
            drop(state);
            self.notify.notified().await;
        }
    }

    /// Marks a key as processed.
    ///
    /// * `success = true`  — Reset retry tracking.  The key is ready for a
    ///   fresh add cycle.
    /// * `success = false` — Log a warning, increment the retry counter, and
    ///   re-queue the key after the next backoff duration.
    pub async fn done(&self, key: QueueKey, success: bool) {
        let mut state = self.state.lock().await;
        state.pending.remove(&key);

        if success {
            // Reset retry tracking.
            state.retry_count.remove(&key);
        } else {
            let count = state.retry_count.get(&key).copied().unwrap_or(0);
            let delay = Self::next_backoff(count);

            state.retry_count.insert(key.clone(), count);

            warn!(
                kind = %key.key.kind,
                name = %key.name,
                namespace = ?key.namespace,
                retry_count = count,
                backoff_secs = delay.as_secs(),
                "reconciliation failed, will retry after backoff",
            );

            // Schedule re-queue (release the lock first).
            drop(state);
            self.requeue_after_inner(key, delay).await;
        }
    }

    /// Re-queues a key after the specified duration.
    ///
    /// This is typically used when a reconciler returns
    /// [`ReconcileResult`](crate::reconciler::ReconcileResult) with a
    /// non-`None` `requeue_after` field.
    pub async fn requeue_after(&self, key: QueueKey, duration: Duration) {
        self.requeue_after_inner(key, duration).await;
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Spawns a background task that sleeps for `duration` and then adds
    /// the key back to the queue.
    async fn requeue_after_inner(&self, key: QueueKey, duration: Duration) {
        let state = self.state.clone();
        let notify = self.notify.clone();
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let mut s = state.lock().await;
            if s.pending.insert(key.clone()) {
                s.queue.push_back(key);
                notify.notify_one();
            }
        });
    }

    /// Returns the backoff duration for a given retry count.
    ///
    /// Uses exponential backoff: 2^retry seconds, capped at 300.
    fn next_backoff(retry_count: u32) -> Duration {
        let secs = 1u64 << retry_count.min(9); // 2^retry  (capped at 2⁹ = 512)
        Duration::from_secs(secs.min(300))
    }
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kapi_core::ResourceKey;
    use std::time::Instant;
    use tokio::time::timeout;

    /// Helper: build a [`QueueKey`] for a test resource kind.
    fn test_key(name: &str) -> QueueKey {
        QueueKey::new(
            ResourceKey { group: "test.io".into(), version: "v1".into(), kind: "Widget".into() },
            name,
            None,
        )
    }

    #[tokio::test]
    async fn test_add_deduplication() {
        let wq = WorkQueue::new();
        let k = test_key("dup-test");

        wq.add(k.clone()).await;
        wq.add(k.clone()).await; // duplicate — should be no-op

        let got = wq.get().await;
        assert_eq!(got, k);

        // Queue should be empty now (only one item was added).
        let result = timeout(Duration::from_millis(50), wq.get()).await;
        assert!(result.is_err(), "expected timeout (queue should be empty)");
    }

    #[tokio::test]
    async fn test_get_blocks_when_empty() {
        let wq = WorkQueue::new();
        let k = test_key("block-test");

        // Spawn: after 100 ms add the key.
        let wq_clone = wq.state.clone();
        let notify = wq.notify.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut s = wq_clone.lock().await;
            s.pending.insert(k.clone());
            s.queue.push_back(k);
            notify.notify_one();
        });

        // get() should block and eventually return the key.
        let start = Instant::now();
        let got = wq.get().await;
        let elapsed = start.elapsed();

        assert_eq!(got.name, "block-test");
        // Should have waited at least ~100ms.
        assert!(elapsed >= Duration::from_millis(80), "get returned too fast: {elapsed:?}");
    }

    #[tokio::test]
    async fn test_done_success_resets_backoff() {
        let wq = WorkQueue::new();
        let k = test_key("reset-test");

        // First failure cycle.
        wq.add(k.clone()).await;
        let got = wq.get().await;
        wq.done(got, false).await; // failure → requeued with backoff

        // Let the background requeue fire.
        tokio::time::sleep(Duration::from_millis(50)).await;
        // The key should NOT be available yet (backoff is 1 s for count=0).
        let result = timeout(Duration::from_millis(50), wq.get()).await;
        assert!(result.is_err(), "expected timeout — key still in backoff");

        // Wait for the backoff to elapse.
        tokio::time::sleep(Duration::from_secs(3)).await; // > 2 s backoff
        let got = wq.get().await;
        assert_eq!(got, k);

        // Now succeed — retry tracking should reset.
        wq.done(got, true).await;

        // Re-add immediately — should be available without delay.
        wq.add(k.clone()).await;
        let got2 = timeout(Duration::from_millis(50), wq.get()).await;
        assert!(got2.is_ok(), "expected immediate availability after success reset");
        assert_eq!(got2.unwrap(), k);
    }

    #[tokio::test]
    async fn test_done_failure_applies_backoff() {
        let wq = WorkQueue::new();
        let k = test_key("backoff-test");

        wq.add(k.clone()).await;
        let got = wq.get().await;
        wq.done(got, false).await; // failure → requeued with 2 s backoff

        // Should NOT be available immediately.
        let result = timeout(Duration::from_millis(100), wq.get()).await;
        assert!(result.is_err(), "expected timeout — key should be in backoff");
    }

    #[tokio::test]
    async fn test_requeue_after() {
        let wq = WorkQueue::new();
        let k = test_key("requeue-after-test");

        // Requeue after a short delay.
        wq.requeue_after(k.clone(), Duration::from_millis(150)).await;

        // Should NOT be available before the delay.
        let result = timeout(Duration::from_millis(50), wq.get()).await;
        assert!(result.is_err(), "expected timeout — delay not yet elapsed");

        // Should become available after the delay.
        let got = timeout(Duration::from_millis(200), wq.get()).await;
        assert!(got.is_ok(), "expected key after requeue_after delay");
        assert_eq!(got.unwrap(), k);
    }

    #[tokio::test]
    async fn test_next_backoff_values() {
        // next_backoff is a private fn; test it via the module path.
        assert_eq!(WorkQueue::next_backoff(0), Duration::from_secs(1));
        assert_eq!(WorkQueue::next_backoff(1), Duration::from_secs(2));
        assert_eq!(WorkQueue::next_backoff(2), Duration::from_secs(4));
        assert_eq!(WorkQueue::next_backoff(3), Duration::from_secs(8));
        assert_eq!(WorkQueue::next_backoff(8), Duration::from_secs(256));
        assert_eq!(WorkQueue::next_backoff(9), Duration::from_secs(300)); // capped
        assert_eq!(WorkQueue::next_backoff(10), Duration::from_secs(300)); // capped
    }
}
