## EventBus Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        EventBus                                 │
│                                                                 │
│  channels: DashMap<ResourceKey, broadcast::Sender<WatchEvent>> │
│  capacity: usize  (default 1024, configurable)                 │
│                                                                 │
│  new(capacity)     → construct with given buffer size          │
│  default()         → capacity = 1024                           │
│  publish(key, evt) → fire-and-forget; no-op if no channel      │
│                      cleans dead channel on publish            │
│  subscribe(key)    → WatchStream; auto-creates channel         │
└─────────────────────────────────────────────────────────────────┘
```

### Auto-create on Subscribe (not Publish)

Channels are created when the first subscriber arrives, not when events are published. This avoids holding empty channels for kinds that nobody is watching.

```
subscribe(key):
    entry = channels.entry(key.clone())
        .or_insert_with(|| broadcast::channel(capacity).0)
    receiver = entry.subscribe()
    WatchStream { inner: BroadcastStream::new(receiver) }

publish(key, event):
    if let Some(sender) = channels.get(key):
        if sender.receiver_count() == 0:
            channels.remove(key)    // dead channel cleanup
            return                  // nobody listening
        let _ = sender.send(event)  // fire-and-forget
```

### WatchStream Wrapper

Wraps `BroadcastStream<WatchEvent>` to provide a clean `Stream<Item = WatchEvent>` (not `Result`).

**Lag handling:** On `RecvError::Lagged(n)`, the stream terminates (`Poll::Ready(None)`). This signals to the SSE client that events were missed and a full re-sync (re-list + resubscribe) is needed. This matches Kubernetes watch semantics.

**Why a wrapper:**
- Hides tokio internals from the public API
- Eliminates `Result` handling in SSE handlers
- Provides a place for inline documentation of design decisions
- Keeps `EventBus` flexible (can return different stream types in future)

```rust
pub struct WatchStream {
    inner: BroadcastStream<WatchEvent>,
}

impl Stream for WatchStream {
    type Item = WatchEvent;

    fn poll_next(...) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => Poll::Ready(Some(event)),
            Poll::Ready(Some(Err(RecvError::Lagged(n)))) => {
                tracing::warn!(n, "watcher lagged, terminating stream");
                Poll::Ready(None)  // terminate — client must re-sync
            }
            Poll::Ready(Some(Err(RecvError::Closed))) => Poll::Ready(None),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
```

### Dead Channel Cleanup

**Approach:** Lazy cleanup on publish. When publishing to a kind, check if the channel has zero receivers. If so, remove it and drop the event.

**Why not on subscribe:** A dead channel can be safely reused — new subscribers get a fresh Receiver from the existing Sender. No need to close and recreate.

**Why not periodic sweep:** For v1 (in-memory, bounded kinds), periodic cleanup is overkill. Deferred to P10.

**Tradeoff:** If a kind gets subscribers, they all leave, and nobody ever publishes again — the dead Sender stays in the map. This is a tiny memory leak (empty Sender struct, ~64 bytes) bounded by the number of registered schemas.

### Channel Capacity

Configurable via `EventBus::with_capacity(usize)`. Default: 1024.

Rationale: At ~10 events/sec per kind, a 1024-capacity buffer gives a watcher ~100 seconds of pause time before lagging. Large enough for normal use, small enough that memory is bounded.

### Clone Semantics

`EventBus` derives `Clone` — `DashMap` clones its internal `Arc`, so all clones share the same channel map. Required for Axum `State` extraction.

## P2b Cleanup

The roadmap marks P2b T33-T34 as complete, but the following was not done:

- `src/schema/types.rs` still exists with an old `Schema` struct that contradicts the design ("Schema is a StoredObject, not a separate struct")
- `src/schema/service.rs` and `src/schema/handler.rs` still exist (TODO stubs)
- `src/schema/mod.rs` still declares handler, service, types modules
- `ValidationError` lives in `schema/types.rs` instead of `object/types.rs` (T12)
- `error.rs` imports `ValidationError` from `schema::types`

**Fix:**
1. Move `ValidationError` to `object/types.rs`
2. Update `error.rs` import
3. Delete `schema/types.rs`, `schema/service.rs`, `schema/handler.rs`
4. Update `schema/mod.rs` to only declare `pub mod meta_schema`

## Future: Periodic Cleanup (P10)

A background task that periodically scans and removes all dead channels:

```rust
impl EventBus {
    fn spawn_cleanup_task(self: &Arc<Self>) {
        tokio::spawn({
            let bus = self.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    bus.channels.retain(|_, s| s.receiver_count() > 0);
                }
            }
        });
    }
}
```

Deferred to P10 — not needed for v1 but good to plan for.
