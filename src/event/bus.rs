use std::pin::Pin;
use std::task::{Context, Poll};

use dashmap::DashMap;
use futures_util::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::object::types::WatchEvent;
use crate::store::ResourceKey;

/// Trait abstracting event distribution for SSE watch endpoints.
///
/// This trait isolates `ObjectService` from the concrete `EventBus`
/// implementation, enabling mock-based testing and future event bus
/// backends without touching the service layer.
pub trait EventPublisher: Send + Sync {
    /// Publish a watch event for the given resource key.
    fn publish(&self, key: &ResourceKey, event: WatchEvent);
    /// Subscribe to watch events for the given resource key.
    fn subscribe(&self, key: &ResourceKey) -> WatchStream;
}

/// Per-kind event bus backing SSE watch endpoints.
///
/// Maintains a separate `tokio::broadcast` channel per `ResourceKey`.
/// Channels are auto-created on first `subscribe` and lazily cleaned
/// up on `publish` when all receivers are dropped.
#[derive(Debug, Clone)]
pub struct EventBus {
    channels: DashMap<ResourceKey, broadcast::Sender<WatchEvent>>,
    capacity: usize,
}

const DEFAULT_CAPACITY: usize = 1024;

impl EventBus {
    /// Creates a new `EventBus` with the given per-channel capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            channels: DashMap::new(),
            capacity,
        }
    }

    /// Creates a new `EventBus` with the default capacity of 1024.
    pub fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Creates a new `EventBus` with the given per-channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::new(capacity)
    }

    /// Publish an event to all subscribers of the given key.
    ///
    /// Fire-and-forget semantics: if the channel has no receivers,
    /// it is removed and the event is dropped. If no channel exists,
    /// this is a no-op.
    pub fn publish(&self, key: &ResourceKey, event: WatchEvent) {
        // Check if channel exists with zero receivers (dead channel).
        // Must drop the read guard before we can remove the entry.
        let dead = self
            .channels
            .get(key)
            .is_some_and(|s| s.receiver_count() == 0);

        if dead {
            // Remove dead channel — nobody is listening.
            self.channels.remove(key);
            return;
        }

        // Fire-and-forget: send to any existing channel.
        // Returns Err only if no receivers, which we already checked above.
        if let Some(sender) = self.channels.get(key) {
            let _ = sender.send(event);
        }
    }

    /// Returns the number of active channels for testing purposes.
    #[cfg(test)]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Subscribe to events for the given key.
    ///
    /// Auto-creates a broadcast channel if none exists for this key.
    /// Returns a `WatchStream` that yields `WatchEvent` values.
    pub fn subscribe(&self, key: &ResourceKey) -> WatchStream {
        // Create or reuse the channel for this key.
        // Channels are created lazily — no point allocating for kinds
        // nobody is watching.
        let sender = self
            .channels
            .entry(key.clone())
            .or_insert_with(|| broadcast::channel(self.capacity).0)
            .value()
            .clone();

        // Subscribe to the channel and wrap in our clean stream type.
        let receiver = sender.subscribe();
        WatchStream {
            inner: BroadcastStream::new(receiver),
        }
    }
}

impl EventPublisher for EventBus {
    fn publish(&self, key: &ResourceKey, event: WatchEvent) {
        EventBus::publish(self, key, event);
    }

    fn subscribe(&self, key: &ResourceKey) -> WatchStream {
        EventBus::subscribe(self, key)
    }
}

/// A clean `Stream<Item = WatchEvent>` wrapper around `BroadcastStream`.
///
/// # Why a wrapper?
///
/// Hides tokio internals from the public API so SSE handlers work with
/// `WatchEvent` directly instead of `Result<WatchEvent, RecvError>`. Also
/// keeps `EventBus` free to change the underlying stream type in future.
///
/// # Why terminate on lag?
///
/// When a subscriber falls behind (`RecvError::Lagged(n)`), the stream
/// terminates with `None`. This is honest signaling — the client must
/// re-sync via a full re-list + re-subscribe, matching Kubernetes watch
/// semantics. Silently dropping events would violate consistency.
///
/// # Why auto-create on subscribe?
///
/// Channels are created on first subscriber, not on publish. There is no
/// point holding a channel for a kind that nobody is watching.
#[derive(Debug)]
pub struct WatchStream {
    inner: BroadcastStream<WatchEvent>,
}

impl Stream for WatchStream {
    type Item = WatchEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.get_mut().inner).poll_next(cx) {
            // Normal delivery — forward the event.
            Poll::Ready(Some(Ok(event))) => Poll::Ready(Some(event)),

            // Subscriber fell behind — terminate stream.
            // Client must re-sync via full re-list + re-subscribe.
            Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(n)))) => {
                tracing::warn!(n, "watcher lagged, terminating stream");
                Poll::Ready(None)
            }

            // Channel closed (all Senders dropped) or no more events.
            Poll::Ready(None) => Poll::Ready(None),

            // No events ready yet.
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::types::{ObjectMetadata, StoredObject, UserData, WatchEventType};
    use chrono::Utc;
    use tokio_stream::StreamExt;

    fn make_key() -> ResourceKey {
        ResourceKey {
            group: "kapi.io".into(),
            version: "v1".into(),
            kind: "Schema".into(),
        }
    }

    fn make_event() -> WatchEvent {
        WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: make_key(),
                metadata: ObjectMetadata {
                    name: "test".into(),
                    resource_version: 1,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                data: UserData {
                    value: serde_json::json!({"type": "object"}),
                },
            },
        }
    }

    // Verify WatchStream is Send (required for Axum SSE handlers).
    #[test]
    fn watch_stream_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WatchStream>();
    }

    // Publish an event, single subscriber receives it.
    #[tokio::test]
    async fn single_subscriber_receives_event() {
        let bus = EventBus::default();
        let key = make_key();
        let mut stream = bus.subscribe(&key);

        let event = make_event();
        bus.publish(&key, event);

        let received = stream.next().await;
        assert!(matches!(received, Some(WatchEvent { event_type: WatchEventType::Added, .. })));
    }

    // Publish an event, multiple subscribers all receive it.
    #[tokio::test]
    async fn multiple_subscribers_receive_event() {
        let bus = EventBus::default();
        let key = make_key();

        let mut stream1 = bus.subscribe(&key);
        let mut stream2 = bus.subscribe(&key);

        bus.publish(&key, make_event());

        // Both subscribers should receive the same event.
        let e1 = stream1.next().await;
        let e2 = stream2.next().await;
        assert!(matches!(e1, Some(WatchEvent { event_type: WatchEventType::Added, .. })));
        assert!(matches!(e2, Some(WatchEvent { event_type: WatchEventType::Added, .. })));
    }

    // Dead channel cleanup: channels with zero receivers are lazily removed
    // on publish. Ensure channels survive as long as at least one subscriber
    // remains, and are cleaned up only after all subscribers are gone.
    #[tokio::test]
    async fn dead_channel_cleanup() {
        let bus = EventBus::default();
        let key = make_key();

        // Multiple subscribers — channel stays alive even if some drop.
        let stream1 = bus.subscribe(&key);
        let stream2 = bus.subscribe(&key);
        assert_eq!(bus.channel_count(), 1);

        // Drop one subscriber — channel still has stream2 alive.
        drop(stream1);
        bus.publish(&key, make_event());
        // Channel should NOT be cleaned up — stream2 is still active.
        assert_eq!(bus.channel_count(), 1);

        // Now drop the remaining subscriber and verify cleanup.
        drop(stream2);
        bus.publish(&key, make_event());
        assert_eq!(bus.channel_count(), 0);
    }

    // Dropped subscriber does not block publisher from sending to remaining subscribers.
    #[tokio::test]
    async fn dropped_subscriber_does_not_block() {
        let bus = EventBus::default();
        let key = make_key();

        let stream1 = bus.subscribe(&key);
        let mut stream2 = bus.subscribe(&key);

        // Drop the first subscriber — the channel still has 1 receiver.
        drop(stream1);

        // Publish after drop: must not panic, remaining subscriber gets the event.
        bus.publish(&key, make_event());

        let received = stream2.next().await;
        assert!(matches!(received, Some(WatchEvent { event_type: WatchEventType::Added, .. })));
    }

    // Publishing to a key with no channel is a no-op — no panic, no error,
    // and no channel is created (channels are created on subscribe, not publish).
    #[tokio::test]
    async fn publish_to_no_channel_is_noop() {
        let bus = EventBus::default();
        let key = make_key();

        // No channel exists for this key — publish should be silent.
        bus.publish(&key, make_event());

        // No channel should have been created.
        assert_eq!(bus.channel_count(), 0);
    }

    // WatchStream terminates on lag: when the subscriber falls behind and
    // the broadcast channel overwrites oldest messages, the stream yields
    // None instead of silently dropping events.
    #[tokio::test]
    async fn watch_stream_terminates_on_lag() {
        // Capacity 1 means a single unread slot — two publishes will overwrite.
        let bus = EventBus::with_capacity(1);
        let key = make_key();

        let mut stream = bus.subscribe(&key);

        // Publish two events without consuming — second overwrites the first.
        bus.publish(&key, make_event());
        bus.publish(&key, make_event());

        // The receiver missed the first event due to buffer overrun.
        // WatchStream should yield None (stream terminates).
        let received = stream.next().await;
        assert!(received.is_none(), "expected stream termination on lag, got: {received:?}");
    }
}
