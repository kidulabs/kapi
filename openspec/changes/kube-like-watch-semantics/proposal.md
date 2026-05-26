## Why

Watch currently broadcasts every event for a resource kind to all subscribers — there is no way to watch a specific object by name or filter by labels. This forces clients to receive and discard irrelevant events, which doesn't match Kubernetes watch semantics and won't scale when label filtering is added. The event bus uses `tokio::broadcast` channels which force all subscribers to consume at the full kind rate, even when filtering by name.

## What Changes

- **BREAKING**: Replace `tokio::broadcast`-based `EventBus` with predicate routing using per-watcher `mpsc` channels. Each watcher receives only events matching its filter.
- **BREAKING**: Change `EventPublisher::subscribe` signature to accept a `WatchFilter` parameter.
- Add `WatchFilter` enum (`All`, `FieldSelector(FieldSelector)`) and `FieldSelector` enum (`NameEquals(String)`) to `object/types.rs`.
- Add `?fieldSelector=metadata.name=<name>` query parameter support for watch requests, using Kubernetes-compatible syntax.
- Return 400 Bad Request for `fieldSelector` on non-watch requests or with unsupported fields.
- Add `InvalidFieldSelector` error variant to `AppError`.
- Simplify `WatchStream` — no filter loop, just wraps `mpsc::Receiver`.
- Add trace logging for filtered-out events and removed watchers.

## Capabilities

### New Capabilities
- `watch-filter`: Defines `WatchFilter` and `FieldSelector` types for filtering watch event streams, with `matches()` logic and Kubernetes-compatible `fieldSelector` query parameter parsing.

### Modified Capabilities
- `event-bus`: Replace broadcast channels with predicate routing (per-watcher `mpsc` channels + `Vec<Watcher>`). Change `subscribe` to accept `WatchFilter`. Remove `broadcast` dependency, add `mpsc`. Change `WatchStream` to wrap `mpsc::Receiver`. Add watcher cleanup on send failure.
- `object-handlers`: Add `field_selector` field to `ListQuery`. Parse `fieldSelector` query parameter into `WatchFilter`. Return 400 for `fieldSelector` on non-watch requests or with unsupported fields. Pass `WatchFilter` to `ObjectService::subscribe`.
- `object-service`: Change `subscribe` method signature to accept `WatchFilter` and pass it through to `EventPublisher::subscribe`.
- `core-types`: Add `WatchFilter` and `FieldSelector` enums with `matches()` method. Add `InvalidFieldSelector` error variant to `AppError`.

## Impact

- **API**: New `?fieldSelector=metadata.name=<name>` query parameter on watch endpoints. 400 responses for invalid/unsupported field selectors.
- **Event bus**: Internal rewrite from broadcast to predicate routing. No external API change to event publishing.
- **WatchStream**: Simplified — no filter loop, no `BroadcastStreamRecvError::Lagged` handling. Stream ends when `mpsc::Receiver` returns `None`.
- **Dependencies**: Remove `tokio_stream::wrappers::BroadcastStream` usage. Add `tokio::sync::mpsc` for per-watcher channels.
- **Tests**: Unit tests for `EventBus` rewritten for predicate routing semantics. Integration tests for field selector watch filtering and error cases.

## Non-goals

- Label filtering (`labelSelector`) — future work when `ObjectMeta` gains labels.
- `fieldSelector` on list (non-watch) requests — future work requiring store-level filtering.
- `resourceVersion` for watch resume/bookmarks — future work requiring a ring buffer.
- Additional `FieldSelector` variants (`NameNotEquals`, `NameIn`) — future work.
- `WatchFilter::And` combinator for combining selectors — future work.

## Future Work

- Add `labels: HashMap<String, String>` to `ObjectMeta` and `LabelSelector` variant to `WatchFilter`.
- Add `?labelSelector=<expr>` query parameter for label-based watch filtering.
- Add `fieldSelector` and `labelSelector` support on list (non-watch) requests, requiring store-level filtering.
- Add `resourceVersion` watch parameter with ring buffer replay for watch resume.
- Add bookmark events for periodic resourceVersion updates during watch.
- Add `FieldSelector::NameNotEquals` and `FieldSelector::NameIn` variants.
- Add `WatchFilter::And` combinator for composing field and label selectors.