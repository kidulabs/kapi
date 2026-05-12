## Phase 1: P2b Cleanup (prerequisite)

- [x] T1: Move `ValidationError` struct from `src/schema/types.rs` to `src/object/types.rs`
- [x] T2: Update `src/error.rs` to import `ValidationError` from `crate::object::types`
- [x] T3: Delete `src/schema/types.rs`
- [x] T4: Delete `src/schema/service.rs`
- [x] T5: Delete `src/schema/handler.rs`
- [x] T6: Update `src/schema/mod.rs` to only declare `pub mod meta_schema`
- [x] T7: Verify `cargo build` succeeds with no warnings

## Phase 2: EventBus Implementation

- [x] T8: Define `EventBus` struct in `src/event/bus.rs` with `channels: DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` and `capacity: usize`
- [x] T9: Implement `EventBus::new(capacity)`, `EventBus::default()` (capacity=1024), and `EventBus::with_capacity(usize)`
- [x] T10: Implement `EventBus::publish(key, event)` — fire-and-forget, no-op if no channel, clean up dead channel on publish if `receiver_count() == 0`
- [x] T11: Define `WatchStream` wrapper struct around `BroadcastStream<WatchEvent>` with inline documentation explaining: why wrapper (clean API, hide tokio internals), why terminate on lag (honest signaling, K8s semantics), why auto-create on subscribe (no point holding channel if nobody listens)
- [x] T12: Implement `Stream<Item = WatchEvent>` for `WatchStream` — handle `Ok(event)` → yield event, `Lagged(n)` → log warning + terminate stream, `Closed` → terminate stream
- [x] T13: Implement `EventBus::subscribe(key) -> WatchStream` — auto-creates channel via `entry().or_insert_with()`
- [x] T14: Derive `Clone` on `EventBus` (required for Axum State)
- [x] T15: Verify `WatchStream` is `Send` (required for Axum SSE)
- [x] T16: Update `src/event/mod.rs` to export `EventBus` and `WatchStream`

## Phase 3: Tests

- [x] T17: Test: publish an event, single subscriber receives it
- [x] T18: Test: publish an event, multiple subscribers all receive it
- [x] T19: Test: dropped subscriber does not block publisher
- [x] T20: Test: dead channel cleanup — subscribe, drop subscriber, publish, verify channel removed from map
- [x] T21: Test: publish to kind with no channel is a no-op (no panic, no error)
- [x] T22: Test: WatchStream terminates on lag (simulate by creating channel with capacity 1, publish 2 events before consuming)
- [x] T23: Verify `cargo test` passes with no warnings

## Phase 4: Roadmap Updates

- [x] T24: Update P3 section in `roadmap.md` with finalized task descriptions (T26-T30 + T27b + T30b)
- [x] T25: Add P10 section for periodic event bus cleanup future work
- [x] T26: Add roadmap hygiene tasks section
- [x] T27: Correct P2b checkbox states to reflect actual completion
