## Context

kapi's watch system currently uses `tokio::broadcast` channels per `ResourceKey` (group/version/kind). Every subscriber to a kind receives every event for that kind — there is no way to watch a specific object by name or filter by labels. The `EventPublisher` trait has `subscribe(&self, key: &ResourceKey) -> WatchStream` with no filtering capability.

The current `WatchStream` wraps `BroadcastStream<WatchEvent>` and terminates on `Lagged` errors. All filtering would have to happen in the stream's `poll_next`, which means every subscriber must consume events at the full kind rate even when filtering by name.

This change replaces the broadcast architecture with predicate routing: each watcher gets its own `mpsc` channel and a `WatchFilter`. The `publish()` method iterates watchers and sends events only to those whose filter matches. This eliminates unnecessary work for filtered watchers and aligns with Kubernetes watch semantics.

## Goals / Non-Goals

**Goals:**
- Support `?fieldSelector=metadata.name=<name>` on watch requests to filter events by object name
- Replace broadcast channels with predicate routing (per-watcher `mpsc` channels)
- Maintain existing "watch all" behavior via `WatchFilter::All`
- Return 400 for `fieldSelector` on non-watch requests or with unsupported fields
- Preserve stream termination semantics (client must re-list + re-watch on disconnect)
- Design `WatchFilter` and `FieldSelector` as extensible enums for future label support

**Non-Goals:**
- Label filtering (`labelSelector`) — requires `ObjectMeta` labels field
- `fieldSelector` / `labelSelector` on list (non-watch) requests — requires store-level filtering
- `resourceVersion` watch resume — requires ring buffer replay
- Bookmark events
- Additional `FieldSelector` variants beyond `NameEquals`
- `WatchFilter::And` combinator

## Decisions

### Decision 1: Predicate routing over broadcast+filter

**Choice**: Per-watcher `mpsc` channels with filter-based routing in `publish()`.

**Alternatives considered**:
- **Broadcast + filter in WatchStream**: Keep `tokio::broadcast` per kind, add a `WatchFilter` field to `WatchStream`, loop in `poll_next` skipping non-matching events. Simpler change but forces all subscribers to consume at the full kind rate. Filtered watchers must process every event for the kind even when they care about 0.01% of them. The lag problem is amplified for filtered watchers.
- **Per-object channels**: One `broadcast` channel per `(ResourceKey, name)`. "Watch all" would require subscribing to N channels dynamically. Memory explosion with many objects. Complex lifecycle management. Kubernetes doesn't do this either.

**Rationale**: Predicate routing eliminates unnecessary work for filtered watchers. Each watcher's `mpsc` buffer fills at the filtered rate, not the full kind rate. The implementation is only ~20 lines more than broadcast+filter. The `EventPublisher` trait abstracts the implementation, so callers don't change. The `Watcher` struct and `retain()`-based cleanup is simpler than managing broadcast channel lifecycles.

### Decision 2: `WatchFilter` as enum, not closure

**Choice**: `enum WatchFilter { All, FieldSelector(FieldSelector) }` with `enum FieldSelector { NameEquals(String) }`.

**Alternatives considered**:
- **`Box<dyn Fn(&WatchEvent) -> bool>`**: More flexible but not serializable, not debuggable, not parseable from HTTP query params, not composable in a structured way.
- **`WatchFilter::ByName(String)`**: Simpler but doesn't leave room for the field/label distinction that Kubernetes uses. `FieldSelector` and `LabelSelector` are separate concepts in the Kube API.

**Rationale**: Enums are serializable, debuggable, parseable from query params, and extensible. The `FieldSelector` wrapper mirrors the Kubernetes `fieldSelector` concept. Adding `LabelSelector` later is a natural extension. Adding `And(Box<WatchFilter>, Box<WatchFilter>)` for combining selectors is also straightforward.

### Decision 3: Kube-compatible `fieldSelector` syntax, strict on unknown fields

**Choice**: `?fieldSelector=metadata.name=<value>` with 400 Bad Request for unsupported fields.

**Alternatives considered**:
- **Simple `?name=<value>`**: Cleaner for kapi's current data model but diverges from Kube convention. Would need to be replaced or supplemented when more fields are added.
- **Lenient parsing (ignore unknown fields)**: Silently ignoring fields is confusing for API users. Explicit errors are better.

**Rationale**: The Kube syntax is well-established and extensible. When `metadata.namespace` or other fields are added, they slot in naturally. When `labelSelector` arrives, it's a separate query parameter. Strict validation catches user errors early.

### Decision 4: `fieldSelector` only on watch requests

**Choice**: Return 400 Bad Request if `fieldSelector` is present on a non-watch list request.

**Alternatives considered**:
- **Apply `fieldSelector` to list results too**: Requires store-level filtering support (the `ObjectStore::list` method would need filter parameters). More work, and the store doesn't support it yet.
- **Ignore `fieldSelector` on list requests**: Silently ignoring parameters is confusing.

**Rationale**: Explicit 400 is clear and honest. List filtering with selectors is a separate feature that requires store changes. Marking it as future work keeps this change focused.

### Decision 5: Module placement — `WatchFilter` in `object/types.rs`

**Choice**: `WatchFilter` and `FieldSelector` live in `object/types.rs` alongside `WatchEvent` and `ObjectMeta`.

**Alternatives considered**:
- **New `watch/filter.rs` module**: Clean separation but over-engineering for two enum variants. `WatchFilter::matches()` references `WatchEvent` and `ObjectMeta` which are already in `object/types.rs`.
- **`event/bus.rs`**: WatchFilter is consumed by EventBus but is really about the API/handler layer, not the event bus internals.

**Rationale**: `WatchFilter` is tightly coupled to `WatchEvent` and `ObjectMeta`. Put them together. Split into a separate module when it grows large enough to warrant it.

### Decision 6: Watcher cleanup via `retain()` on publish

**Choice**: When `mpsc::Sender::try_send` returns `Err` (Full or Closed), remove the watcher via `Vec::retain()` in `publish()`.

**Rationale**: This is the natural cleanup point. A failed send means the watcher is either disconnected (Closed) or too slow (Full). In both cases, removing the watcher is correct — it matches the existing "terminate on lag" semantics. The watcher's `mpsc::Receiver` will return `None` on next poll, ending the SSE stream. The client must re-list and re-watch.

### Decision 7: `try_send` with bounded `mpsc` channel

**Choice**: Use `tokio::sync::mpsc::channel` with a configurable capacity (default 256). Use `try_send` (non-blocking) in `publish()`. On `TrySendError::Full` or `TrySendError::Closed`, remove the watcher.

**Rationale**: `publish()` is synchronous (not async). We cannot `await` capacity in `publish()`. `try_send` is the only option. Bounded channels prevent unbounded memory growth. The capacity is per-watcher, so a filtered watcher's buffer fills at the filtered rate — much better than broadcast where all subscribers share one buffer at the full kind rate.

## Risks / Trade-offs

**[Risk] `publish()` is O(W) per event where W = number of watchers for that kind** → Acceptable for kapi's scale. With 10-100 watchers per kind and a string comparison per filter, this is nanoseconds. If scale becomes a concern, the `EventPublisher` trait allows swapping to a different implementation without changing callers.

**[Risk] `try_send::Full` removes the watcher entirely, ending the stream** → This matches existing "terminate on lag" semantics. The client must re-list + re-watch. With predicate routing, a filtered watcher's buffer fills at the filtered rate (not the full kind rate), so this is less likely to happen than with broadcast.

**[Risk] No `broadcast` dependency means no `BroadcastStreamRecvError::Lagged` handling** → The `WatchStream` becomes simpler — it just wraps `mpsc::Receiver`. Stream ends when the sender is dropped (which happens when the watcher is removed). No special lag handling needed.

**[Risk] `DashMap<ResourceKey, Vec<Watcher>>` requires locking the Vec for publish** → `DashMap::get_mut` returns a reference guard. The `retain()` call happens under this guard. For a Vec of 10-100 watchers, this is fast. No contention across different kinds (each kind has its own DashMap entry).

**[Trade-off] Per-watcher `mpsc` channels use more memory than shared broadcast** → Each watcher has its own buffer (default 256 slots × sizeof(WatchEvent)). For 100 watchers, this is ~100 × 256 × ~200 bytes ≈ 5MB. Acceptable. The benefit (no unnecessary work for filtered watchers) outweighs the cost.

## Open Questions

None — all design decisions have been resolved during exploration.