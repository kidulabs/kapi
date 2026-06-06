use std::pin::Pin;
use std::task::{Context, Poll};

use dashmap::DashMap;
use futures_util::Stream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use crate::object::types::{WatchEvent, WatchFilter};
use crate::store::ResourceKey;

/// Trait abstracting event distribution for SSE watch endpoints.
///
/// This trait isolates `ObjectService` from the concrete `EventBus`
/// implementation, enabling mock-based testing and future event bus
/// backends without touching the service layer.
pub trait EventPublisher: Send + Sync + 'static {
    /// Publish a watch event for the given resource key.
    fn publish(&self, key: &ResourceKey, event: WatchEvent);
    /// Subscribe to watch events for the given resource key, filtered by the
    /// provided WatchFilter. Use WatchFilter::All to receive all events.
    fn subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream;
    /// Returns the number of watchers for a given key, if any exist.
    fn watcher_count(&self, key: &ResourceKey) -> Option<usize> {
        let _ = key;
        None
    }
}

/// A single watcher holding a filter and its mpsc sender.
/// EventBus::publish iterates watchers, sending events only to matching ones.
#[derive(Debug, Clone)]
pub struct Watcher {
    /// Filter determining which events this watcher receives
    pub filter: WatchFilter,
    /// Channel sender — publish() calls try_send; Full/Closed removes the watcher
    pub sender: mpsc::Sender<WatchEvent>,
}

/// Per-kind event bus with predicate routing for SSE watch endpoints.
///
/// Maintains a `DashMap<ResourceKey, Vec<Watcher>>` where each watcher has
/// its own `mpsc::Sender` and `WatchFilter`. On publish, events are delivered
/// only to watchers whose filter matches. Watchers with full or closed channels
/// are removed via retain().
///
#[derive(Debug, Clone)]
pub struct EventBus {
    /// Per-kind watcher lists. Each watcher has an mpsc sender and a filter.
    watchers: DashMap<ResourceKey, Vec<Watcher>>,
    /// Per-watcher mpsc channel capacity (default 256)
    watcher_capacity: usize,
}

const DEFAULT_WATCHER_CAPACITY: usize = 256;

impl EventBus {
    /// Creates a new `EventBus` with the given per-watcher channel capacity.
    pub fn new(watcher_capacity: usize) -> Self {
        Self {
            watchers: DashMap::new(),
            watcher_capacity,
        }
    }

    /// Creates a new `EventBus` with the given per-watcher channel capacity.
    pub fn with_watcher_capacity(watcher_capacity: usize) -> Self {
        Self::new(watcher_capacity)
    }

    /// Publish an event to all matching watchers of the given key.
    ///
    /// Iterates watchers, checks each filter via WatchFilter::matches, and
    /// sends matching events via try_send. Watchers whose channel is full or
    /// closed are removed. No-op if no watchers exist for the key.
    ///
    /// # Watcher cleanup (lazy)
    ///
    /// When a subscriber disconnects, the HTTP client drops the WatchStream,
    /// which drops the mpsc::Receiver. The Watcher struct (with its Sender)
    /// stays in the Vec until the next publish() call for that key.
    ///
    /// We detect dead watchers lazily here in retain(): try_send returns
    /// TrySendError::Closed when the receiver is gone, and we return false
    /// to remove the watcher. There is no cheap way to detect a dropped
    /// receiver from the sender side without attempting a send — polling
    /// is_closed() on every subscriber drop would be wasteful. Lazy cleanup
    /// on publish is simple, correct, and incurs only a failed try_send per
    /// dead watcher per publish.
    pub fn publish(&self, key: &ResourceKey, event: WatchEvent) {
        if let Some(mut watchers) = self.watchers.get_mut(key) {
            let object_name = event.object.metadata.name.clone();
            // retain() removes dead watchers while iterating active ones
            watchers.retain(|w| {
                if !w.filter.matches(&event) {
                    tracing::trace!(name = %object_name, "event filtered out by watcher filter");
                    return true;
                }
                // try_send is non-blocking — publish() is not async
                match w.sender.try_send(event.clone()) {
                    Ok(()) => {
                        tracing::trace!(name = %object_name, "event delivered to watcher");
                        true
                    }
                    Err(TrySendError::Full(_)) => {
                        tracing::trace!(name = %object_name, "watcher buffer full, removing");
                        false
                    }
                    // Receiver was dropped (client disconnected) — remove watcher
                    Err(TrySendError::Closed(_)) => {
                        tracing::trace!(name = %object_name, "watcher channel closed, removing");
                        false
                    }
                }
            });
        }
    }

    /// Subscribe to events for the given key, filtered by WatchFilter.
    ///
    /// Creates a new mpsc::channel, pushes a Watcher with the given filter
    /// into the key's Vec, and returns a WatchStream wrapping the receiver.
    /// If no watchers exist for this key, a new Vec is created.
    pub fn subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream {
        let (tx, rx) = mpsc::channel(self.watcher_capacity);
        let watcher = Watcher { filter, sender: tx };
        tracing::trace!(
            group = %key.group,
            version = %key.version,
            kind = %key.kind,
            "watcher subscribed"
        );
        self.watchers.entry(key.clone()).or_default().push(watcher);
        WatchStream { inner: rx }
    }

    /// Returns the number of watchers for a given key.
    /// Used in integration tests to verify lazy cleanup behavior.
    pub fn watcher_count(&self, key: &ResourceKey) -> Option<usize> {
        self.watchers.get(key).map(|w| w.len())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_WATCHER_CAPACITY)
    }
}

impl EventPublisher for EventBus {
    fn publish(&self, key: &ResourceKey, event: WatchEvent) {
        EventBus::publish(self, key, event);
    }

    fn subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream {
        EventBus::subscribe(self, key, filter)
    }

    fn watcher_count(&self, key: &ResourceKey) -> Option<usize> {
        EventBus::watcher_count(self, key)
    }
}

/// A clean `Stream<Item = WatchEvent>` wrapper around `mpsc::Receiver`.
///
/// # Why a wrapper?
///
/// Hides tokio internals from the public API so SSE handlers work with
/// `WatchEvent` directly. Also keeps `EventBus` free to change the
/// underlying stream type without touching handler code.
///
/// # Why mpsc instead of broadcast?
///
/// With predicate routing, each watcher gets its own mpsc channel.
/// Filtering happens in EventBus::publish, so WatchStream is simpler —
/// no Lagged handling, no filter loop. The stream ends when the sender
/// is dropped (watcher removed from EventBus due to Full/Closed).
#[derive(Debug)]
pub struct WatchStream {
    inner: mpsc::Receiver<WatchEvent>,
}

impl Stream for WatchStream {
    type Item = WatchEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Delegate directly to mpsc::Receiver::poll_recv
        // Stream ends (None) when the sender is dropped (watcher removed)
        self.get_mut().inner.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::types::{
        FieldSelector, ObjectMeta, SpecData, StoredObject, SystemMetadata, WatchEventType,
    };
    use crate::schema::SCHEMA_KIND;
    use chrono::Utc;
    use std::collections::HashMap;
    use tokio_stream::StreamExt;

    fn make_key() -> ResourceKey {
        ResourceKey {
            group: "kapi.io".into(),
            version: "v1".into(),
            kind: SCHEMA_KIND.into(),
        }
    }

    fn make_event() -> WatchEvent {
        WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: make_key(),
                metadata: ObjectMeta {
                    name: "test".into(),
                    labels: HashMap::new(),
                },
                system: SystemMetadata {
                    resource_version: 1,
                    generation: 1,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({"type": "object"}),
                },
                status: None,
            },
        }
    }

    // Verify WatchStream is Send (required for Axum SSE handlers).
    #[test]
    fn watch_stream_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WatchStream>();
    }

    // Single subscriber with WatchFilter::All receives published events.
    #[tokio::test]
    async fn single_subscriber_receives_event() {
        let bus = EventBus::default();
        let key = make_key();
        let mut stream = bus.subscribe(&key, WatchFilter::All);

        let event = make_event();
        bus.publish(&key, event);

        let received = stream.next().await;
        assert!(matches!(
            received,
            Some(WatchEvent {
                event_type: WatchEventType::Added,
                ..
            })
        ));
    }

    // Multiple subscribers with WatchFilter::All all receive the same event.
    #[tokio::test]
    async fn multiple_subscribers_receive_event() {
        let bus = EventBus::default();
        let key = make_key();

        let mut stream1 = bus.subscribe(&key, WatchFilter::All);
        let mut stream2 = bus.subscribe(&key, WatchFilter::All);

        bus.publish(&key, make_event());

        let e1 = stream1.next().await;
        let e2 = stream2.next().await;
        assert!(matches!(
            e1,
            Some(WatchEvent {
                event_type: WatchEventType::Added,
                ..
            })
        ));
        assert!(matches!(
            e2,
            Some(WatchEvent {
                event_type: WatchEventType::Added,
                ..
            })
        ));
    }

    // Watchers with different filters: matching watchers receive, non-matching don't.
    #[tokio::test]
    async fn filtered_subscriber_receives_only_matching_events() {
        let bus = EventBus::default();
        let key = make_key();

        let mut all_stream = bus.subscribe(&key, WatchFilter::All);
        let mut filtered = bus.subscribe(
            &key,
            WatchFilter::FieldSelector(FieldSelector::NameEquals("other".into())),
        );

        let event = make_event(); // name = "test"
        bus.publish(&key, event);

        // All subscriber gets the event
        let received = all_stream.next().await;
        assert!(received.is_some());

        // Filtered subscriber (name="other") does NOT get event for name="test"
        // Use timeout to verify no event arrives within 100ms
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(100), filtered.next()).await;
        assert!(received.is_err(), "expected timeout, got: {received:?}");
    }

    // Dead watcher cleanup: watchers with dropped receivers are removed.
    #[tokio::test]
    async fn dead_watcher_cleanup() {
        let bus = EventBus::default();
        let key = make_key();

        // Subscribe two watchers
        let stream1 = bus.subscribe(&key, WatchFilter::All);
        let _stream2 = bus.subscribe(&key, WatchFilter::All);

        // Two watchers
        assert_eq!(bus.watcher_count(&key), Some(2));

        // Drop first watcher and publish — dead watcher should be cleaned up
        drop(stream1);
        bus.publish(&key, make_event());

        // Only one watcher remains
        assert_eq!(bus.watcher_count(&key), Some(1));
    }

    // Dropped subscriber does not block publisher from sending to remaining subscribers.
    #[tokio::test]
    async fn dropped_subscriber_does_not_block() {
        let bus = EventBus::default();
        let key = make_key();

        let _stream1 = bus.subscribe(&key, WatchFilter::All);
        let mut stream2 = bus.subscribe(&key, WatchFilter::All);

        // Publish after first subscriber was dropped (but hasn't been cleaned up yet)
        bus.publish(&key, make_event());

        let received = stream2.next().await;
        assert!(matches!(
            received,
            Some(WatchEvent {
                event_type: WatchEventType::Added,
                ..
            })
        ));
    }

    // Publishing to a key with no watchers is a no-op.
    #[tokio::test]
    async fn publish_to_no_watchers_is_noop() {
        let bus = EventBus::default();
        let key = make_key();

        // No watchers exist for this key — publish should be silent
        bus.publish(&key, make_event());

        // No watchers were created
        assert_eq!(bus.watcher_count(&key), None);
    }
}
