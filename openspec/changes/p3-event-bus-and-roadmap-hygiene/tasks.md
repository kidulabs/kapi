## Phase 1: P2b Cleanup (prerequisite)

- [ ] T1: Move `ValidationError` struct from `src/schema/types.rs` to `src/object/types.rs`
- [ ] T2: Update `src/error.rs` to import `ValidationError` from `crate::object::types`
- [ ] T3: Delete `src/schema/types.rs`
- [ ] T4: Delete `src/schema/service.rs`
- [ ] T5: Delete `src/schema/handler.rs`
- [ ] T6: Update `src/schema/mod.rs` to only declare `pub mod meta_schema`
- [ ] T7: Verify `cargo build` succeeds with no warnings

## Phase 2: EventBus Implementation

- [ ] T8: Define `EventBus` struct in `src/event/bus.rs` with `channels: DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` and `capacity: usize`
- [ ] T9: Implement `EventBus::new(capacity)`, `EventBus::default()` (capacity=1024), and `EventBus::with_capacity(usize)`
- [ ] T10: Implement `EventBus::publish(key, event)` — fire-and-forget, no-op if no channel, clean up dead channel on publish if `receiver_count() == 0`
- [ ] T11: Define `WatchStream` wrapper struct around `BroadcastStream<WatchEvent>` with inline documentation explaining: why wrapper (clean API, hide tokio internals), why terminate on lag (honest signaling, K8s semantics), why auto-create on subscribe (no point holding channel if nobody listens)
- [ ] T12: Implement `Stream<Item = WatchEvent>` for `WatchStream` — handle `Ok(event)` → yield event, `Lagged(n)` → log warning + terminate stream, `Closed` → terminate stream
- [ ] T13: Implement `EventBus::subscribe(key) -> WatchStream` — auto-creates channel via `entry().or_insert_with()`
- [ ] T14: Derive `Clone` on `EventBus` (required for Axum State)
- [ ] T15: Verify `WatchStream` is `Send` (required for Axum SSE)
- [ ] T16: Update `src/event/mod.rs` to export `EventBus` and `WatchStream`

## Phase 3: Tests

- [ ] T17: Test: publish an event, single subscriber receives it
- [ ] T18: Test: publish an event, multiple subscribers all receive it
- [ ] T19: Test: dropped subscriber does not block publisher
- [ ] T20: Test: dead channel cleanup — subscribe, drop subscriber, publish, verify channel removed from map
- [ ] T21: Test: publish to kind with no channel is a no-op (no panic, no error)
- [ ] T22: Test: WatchStream terminates on lag (simulate by creating channel with capacity 1, publish 2 events before consuming)
- [ ] T23: Verify `cargo test` passes with no warnings

## Phase 4: Roadmap Updates

- [ ] T24: Update P3 section in `roadmap.md` with finalized task descriptions (T26-T30 + T27b + T30b)
- [ ] T25: Add P10 section for periodic event bus cleanup future work
- [ ] T26: Add roadmap hygiene tasks section
- [ ] T27: Correct P2b checkbox states to reflect actual completion
