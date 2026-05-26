## MODIFIED Requirements

### Requirement: EventBus per-kind watcher management
The system SHALL maintain a `DashMap<ResourceKey, Vec<Watcher>>` where each `Watcher` holds a `WatchFilter` and a `mpsc::Sender<WatchEvent>`. The `EventBus` SHALL support configurable per-watcher channel capacity with a default of 256.

#### Scenario: Subscribe with filter creates a new watcher
- **WHEN** `subscribe(key, filter)` is called for a kind with no existing watchers
- **THEN** a new `Vec<Watcher>` is created for that key
- **AND** a `Watcher` with the given filter and a new `mpsc::Sender` is added
- **AND** a `WatchStream` wrapping the `mpsc::Receiver` is returned

#### Scenario: Subscribe reuses existing watcher list
- **WHEN** `subscribe(key, filter)` is called for a kind with existing watchers
- **THEN** a new `Watcher` is appended to the existing `Vec<Watcher>`
- **AND** a `WatchStream` wrapping the new `mpsc::Receiver` is returned

#### Scenario: Multiple subscribers with different filters
- **WHEN** two subscribers call `subscribe(key, WatchFilter::All)` and `subscribe(key, WatchFilter::FieldSelector(FieldSelector::NameEquals("a")))`
- **AND** an event for object "a" is published
- **THEN** the `All` subscriber receives the event
- **AND** the `NameEquals("a")` subscriber receives the event

#### Scenario: Filtered subscriber does not receive non-matching events
- **WHEN** a subscriber calls `subscribe(key, WatchFilter::FieldSelector(FieldSelector::NameEquals("a")))`
- **AND** an event for object "b" is published
- **THEN** the subscriber does NOT receive the event

### Requirement: EventBus publish semantics with predicate routing
The system SHALL publish events by iterating all watchers for the given key, checking each watcher's filter, and sending matching events via `mpsc::Sender::try_send`. Watchers whose send fails (Full or Closed) SHALL be removed via `Vec::retain`.

#### Scenario: Publish to active watchers with matching filter
- **WHEN** `publish(key, event)` is called and watchers exist with matching filters
- **THEN** the event is sent to each matching watcher's channel via `try_send`
- **AND** non-matching watchers are skipped (event not sent)

#### Scenario: Publish with no watchers
- **WHEN** `publish(key, event)` is called and no watchers exist for the key
- **THEN** the event is silently dropped (no-op)

#### Scenario: Publish removes watchers with full channels
- **WHEN** `publish(key, event)` is called and a watcher's `try_send` returns `TrySendError::Full`
- **THEN** the watcher is removed from the watcher list
- **AND** the watcher's `WatchStream` will return `None` on next poll (stream ends)

#### Scenario: Publish removes watchers with closed channels
- **WHEN** `publish(key, event)` is called and a watcher's `try_send` returns `TrySendError::Closed`
- **THEN** the watcher is removed from the watcher list

#### Scenario: Trace logging for filtered events
- **WHEN** `publish(key, event)` is called and a watcher's filter does not match the event
- **THEN** a `tracing::trace!` message SHALL be logged with the event's object name

#### Scenario: Trace logging for removed watchers
- **WHEN** `publish(key, event)` is called and a watcher's `try_send` fails
- **THEN** a `tracing::trace!` message SHALL be logged indicating the watcher was removed

### Requirement: WatchStream wraps mpsc Receiver
The system SHALL provide a `WatchStream` type that wraps `mpsc::Receiver<WatchEvent>` and implements `Stream<Item = WatchEvent>`. `WatchStream` SHALL NOT contain a filter field — filtering is handled by the `EventBus` during publish.

#### Scenario: Normal event delivery
- **WHEN** an event is sent to the `mpsc::Sender` paired with the `WatchStream`'s receiver
- **THEN** `WatchStream` yields `Some(WatchEvent)`

#### Scenario: Stream termination on channel close
- **WHEN** the `mpsc::Sender` is dropped (watcher removed from EventBus)
- **THEN** `WatchStream` yields `None` (stream terminates)

#### Scenario: Stream pending when no events
- **WHEN** no events are available in the `mpsc` channel
- **THEN** `WatchStream::poll_next` returns `Poll::Pending`

### Requirement: WatchStream is Send
The `WatchStream` type SHALL be `Send` to support Axum SSE handlers across thread boundaries.

#### Scenario: WatchStream is Send
- **WHEN** the code is compiled
- **THEN** `WatchStream` satisfies the `Send` trait bound

### Requirement: EventBus is Clone
The `EventBus` type SHALL be `Clone` so it can be stored in Axum `State` and extracted by handlers.

#### Scenario: EventBus clone
- **WHEN** `EventBus::clone()` is called
- **THEN** a new `EventBus` instance is created sharing the same `DashMap` of watchers

### Requirement: EventPublisher trait accepts WatchFilter
The system SHALL define an `EventPublisher` trait with `publish(&self, key: &ResourceKey, event: WatchEvent)` and `subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` methods. The trait SHALL require `Send + Sync`.

#### Scenario: Trait is object-safe
- **WHEN** a type implements `EventPublisher`
- **THEN** it can be used as `dyn EventPublisher` inside `Arc`

#### Scenario: EventBus implements EventPublisher
- **WHEN** `EventBus` is constructed
- **THEN** it implements all `EventPublisher` trait methods, delegating to its existing inherent methods

### Requirement: EventPublisher re-exported from event module
The system SHALL re-export `EventPublisher`, `EventBus`, and `WatchStream` from `src/event/mod.rs`.

#### Scenario: EventPublisher is importable from crate::event
- **WHEN** a module imports `use crate::event::EventPublisher`
- **THEN** the `EventPublisher` trait is in scope

### Requirement: Multi-subscriber delivery with filters
The system SHALL deliver each published event only to watchers whose filter matches the event.

#### Scenario: Multiple subscribers with different filters
- **WHEN** three subscribers are watching the same kind with filters `All`, `NameEquals("a")`, and `NameEquals("b")`
- **AND** events for objects "a", "b", and "c" are published
- **THEN** the `All` subscriber receives all three events
- **AND** the `NameEquals("a")` subscriber receives only the event for "a"
- **AND** the `NameEquals("b")` subscriber receives only the event for "b"

### Requirement: Dropped subscriber does not block publisher
The system SHALL continue publishing to remaining watchers when one subscriber drops.

#### Scenario: Subscriber drop during publishing
- **WHEN** a subscriber's `WatchStream` is dropped
- **AND** an event is published
- **THEN** remaining subscribers with matching filters receive the event
- **AND** no panic or error occurs

## REMOVED Requirements

### Requirement: EventBus per-kind channel management
**Reason**: Replaced by per-kind watcher list with predicate routing. Broadcast channels are no longer used.
**Migration**: Use `subscribe(key, filter)` instead of `subscribe(key)`. The `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` is replaced by `DashMap<ResourceKey, Vec<Watcher>>`.

### Requirement: EventBus configurable capacity
**Reason**: Replaced by configurable per-watcher channel capacity. The broadcast channel capacity is no longer relevant.
**Migration**: Use `EventBus::with_watcher_capacity(n)` instead of `EventBus::with_capacity(n)`.

### Requirement: WatchStream clean stream API
**Reason**: Replaced by simpler `WatchStream` wrapping `mpsc::Receiver`. The `BroadcastStreamRecvError::Lagged` handling is no longer needed.
**Migration**: `WatchStream` no longer handles `Lagged` errors. Stream termination happens when the `mpsc::Sender` is dropped (watcher removed from EventBus).

### Requirement: WatchStream terminates on lag
**Reason**: No longer applicable. With predicate routing, each watcher has its own `mpsc` channel. When the channel is full, `try_send` fails and the watcher is removed. The stream ends when the sender is dropped.
**Migration**: Stream termination now happens via `mpsc::Receiver` returning `None` when the sender is dropped. No `Lagged` error handling needed.