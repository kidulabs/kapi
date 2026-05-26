## 1. Core Types

- [x] 1.1 Add `FieldSelector` enum (`NameEquals(String)`) to `src/object/types.rs` with `Debug` and `Clone` derives
- [x] 1.2 Add `WatchFilter` enum (`All`, `FieldSelector(FieldSelector)`) to `src/object/types.rs` with `Debug` and `Clone` derives
- [x] 1.3 Implement `WatchFilter::matches(&self, event: &WatchEvent) -> bool` method
- [x] 1.4 Add `InvalidFieldSelector(String)` variant to `AppError` in `src/error.rs` with HTTP 400 response mapping
- [x] 1.5 Verify with `cargo check`

## 2. Event Bus Rewrite

- [x] 2.1 Replace `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` with `DashMap<ResourceKey, Vec<Watcher>>` in `EventBus`, where `Watcher { filter: WatchFilter, sender: mpsc::Sender<WatchEvent> }`
- [x] 2.2 Add `watcher_capacity: usize` field to `EventBus` (default 256), replace `capacity` field
- [x] 2.3 Rewrite `EventBus::subscribe` to accept `WatchFilter`, create `mpsc::channel`, push `Watcher` into the key's `Vec`, return `WatchStream` wrapping the `mpsc::Receiver`
- [x] 2.4 Rewrite `EventBus::publish` to iterate watchers with `retain()`, call `try_send` for matching events, remove watchers on `Full` or `Closed`, add `tracing::trace!` for filtered events and removed watchers
- [x] 2.5 Rewrite `WatchStream` to wrap `mpsc::Receiver<WatchEvent>` instead of `BroadcastStream<WatchEvent>`. Remove `filter` field and `Lagged` error handling. `poll_next` simply delegates to the receiver's `poll_recv` via `Stream` impl.
- [x] 2.6 Update `EventPublisher` trait: change `subscribe(&self, key: &ResourceKey)` to `subscribe(&self, key: &ResourceKey, filter: WatchFilter)`
- [x] 2.7 Remove `broadcast` dependency usage from `event/bus.rs` (replace with `mpsc`), remove `tokio_stream::wrappers::BroadcastStream` import
- [x] 2.8 Add `watcher_count(&self, key: &ResourceKey) -> Option<usize>` method to `EventBus` for test assertions (behind `#[cfg(test)]`)
- [x] 2.9 Verify with `cargo check`

## 3. Service Layer

- [x] 3.1 Update `ObjectService::subscribe` signature to accept `filter: WatchFilter` and pass it to `EventPublisher::subscribe`
- [x] 3.2 Verify with `cargo check`

## 4. Handler Layer

- [x] 4.1 Add `field_selector: Option<String>` field to `ListQuery` struct in `src/object/handler.rs` with `#[serde(rename = "fieldSelector")]`
- [x] 4.2 Implement `parse_field_selector(raw: &str) -> Result<WatchFilter, AppError>` function in `src/object/handler.rs`
- [x] 4.3 Update `list` handler: parse `field_selector` into `WatchFilter`, return 400 for `fieldSelector` on non-watch requests, pass `WatchFilter` to `watch()` function
- [x] 4.4 Update `watch` function signature to accept `WatchFilter` parameter, pass it to `ObjectService::subscribe`
- [x] 4.5 Verify with `cargo check`

## 5. OpenAPI Spec

- [x] 5.1 Add `fieldSelector` query parameter to the list/watch endpoint in `build_kind_paths` (`src/openapi/paths.rs`): name `fieldSelector`, in `query`, type `string`, description "Filter watch events by field selector (e.g., metadata.name=my-obj). Only valid with watch=true."
- [x] 5.2 Add `fieldSelector` query parameter to the Schema list endpoint in `build_static_paths` (`src/openapi/paths.rs`)
- [x] 5.3 Add `400` response to the list/watch endpoint for `InvalidFieldSelector` error
- [x] 5.4 Add `InvalidFieldSelector` component schema to `build_static_components` in `src/openapi/components.rs`
- [x] 5.5 Update existing OpenAPI unit tests in `src/openapi/mod.rs` to verify the new `fieldSelector` parameter and `400` response are present
- [x] 5.6 Verify with `cargo test`

## 6. Unit Tests

- [x] 6.1 Rewrite `EventBus` unit tests in `src/event/bus.rs` for predicate routing semantics: single subscriber, multiple subscribers with different filters, filtered subscriber receives only matching events, dead watcher cleanup, publish to no watchers, watcher buffer full removal, `WatchStream` is `Send`
- [x] 6.2 Add unit test: `WatchFilter::All` matches all events
- [x] 6.3 Add unit test: `WatchFilter::FieldSelector(NameEquals)` matches and rejects correctly
- [x] 6.4 Add unit test: `parse_field_selector` valid input, unsupported field, malformed input
- [x] 6.5 Verify with `cargo test`

## 7. Integration Tests

- [x] 7.1 Add test: watch by name — only matching events arrive (`test_watch_by_name_matching_events`)
- [x] 7.2 Add test: watch by name — non-matching events filtered (`test_watch_by_name_non_matching_filtered`)
- [x] 7.3 Add test: invalid fieldSelector returns 400 (`test_watch_invalid_field_selector`)
- [x] 7.4 Add test: fieldSelector on non-watch request returns 400 (`test_field_selector_on_non_watch_returns_400`)
- [x] 7.5 Add test: watch by name + watch all simultaneously (`test_watch_by_name_and_watch_all_simultaneously`)
- [x] 7.6 Register new test functions in `tests/src/main.rs`
- [x] 7.7 Verify with `cargo test -p kapi-tests`

## 8. Documentation and Roadmap

- [x] 8.1 Update `docs/architecture.md`: change "Per-kind `tokio::broadcast` channels" to predicate routing description (per-kind `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender`), update the architecture diagram, update the "Watch Events" request flow to show `fieldSelector` parameter and `WatchFilter` passing, update the "Event bus" row in the Design Decisions table
- [x] 8.2 Update `docs/api-reference.md`: add `fieldSelector` query parameter to the Watch section with syntax and examples, add `InvalidFieldSelector` to the error responses table, document the 400 response for `fieldSelector` on non-watch requests
- [x] 8.3 Extract future work items from `proposal.md` and add concise line items to `roadmap.md`: label filtering with `labelSelector`, `fieldSelector`/`labelSelector` on list requests, `resourceVersion` watch resume with ring buffer, bookmark events, `FieldSelector::NameNotEquals`/`NameIn` variants, `WatchFilter::And` combinator
- [x] 8.4 Update `AGENTS.md` architecture diagram to reflect predicate routing (per-kind `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender`) instead of broadcast channels

## 9. Inline Code Comments

- [x] 9.1 Add inline comments to `EventBus::publish` explaining the predicate routing logic: iterate watchers, filter matching, `try_send`, `retain` for cleanup, trace logging
- [x] 9.2 Add inline comments to `EventBus::subscribe` explaining watcher creation: `mpsc::channel`, `Watcher` struct, `WatchStream` wrapping
- [x] 9.3 Add inline comments to `WatchStream::poll_next` explaining the `mpsc::Receiver` delegation and stream termination semantics
- [x] 9.4 Add inline comments to `parse_field_selector` explaining the Kube-compatible syntax, supported fields, and error cases
- [x] 9.5 Add inline comments to `WatchFilter::matches` explaining the filter evaluation logic and extensibility for `LabelSelector` and `And`

## 10. Cleanup and Verification

- [x] 10.1 Remove unused `broadcast` import from `event/bus.rs` if no longer needed
- [x] 10.2 Remove `BroadcastStreamRecvError` usage and `tokio_stream::wrappers::BroadcastStream` import
- [x] 10.3 Run `cargo clippy --all-targets -- -D warnings` and fix any warnings
- [x] 10.4 Run `cargo test --workspace` and ensure all tests pass