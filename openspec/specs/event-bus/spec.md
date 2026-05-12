## Purpose

Define the event bus system for real-time resource watch notifications. The event bus enables clients to subscribe to resource changes via SSE (Server-Sent Events) using a per-kind broadcast channel pattern.

## Requirements

### Requirement: EventBus per-kind channel management
The system SHALL maintain a separate broadcast channel per `ResourceKey`, stored in a `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>`.

#### Scenario: Channel auto-created on first subscribe
- **WHEN** a subscriber calls `subscribe(key)` for a kind with no existing channel
- **THEN** a new broadcast channel is created and stored
- **AND** the subscriber receives a `WatchStream` from that channel

#### Scenario: Subscribe reuses existing channel
- **WHEN** a subscriber calls `subscribe(key)` for a kind with an existing channel
- **THEN** the existing channel is reused
- **AND** the subscriber receives a new `WatchStream` from that channel

### Requirement: EventBus publish semantics
The system SHALL publish events to the per-kind broadcast channel with fire-and-forget semantics.

#### Scenario: Publish to active channel
- **WHEN** `publish(key, event)` is called and a channel exists with active receivers
- **THEN** the event is sent to all receivers

#### Scenario: Publish with no channel
- **WHEN** `publish(key, event)` is called and no channel exists for the key
- **THEN** the event is silently dropped (no-op)

#### Scenario: Publish to dead channel triggers cleanup
- **WHEN** `publish(key, event)` is called and a channel exists but has zero receivers
- **THEN** the channel is removed from the map
- **AND** the event is dropped

### Requirement: EventBus configurable capacity
The system SHALL support configurable broadcast channel capacity with a default of 1024.

#### Scenario: Default capacity
- **WHEN** `EventBus::default()` or `EventBus::new()` is called without capacity
- **THEN** channels are created with capacity 1024

#### Scenario: Custom capacity
- **WHEN** `EventBus::with_capacity(n)` is called
- **THEN** channels are created with capacity `n`

### Requirement: WatchStream clean stream API
The system SHALL provide a `WatchStream` type that implements `Stream<Item = WatchEvent>` (not `Result`).

#### Scenario: Normal event delivery
- **WHEN** an event is published to an active channel
- **THEN** `WatchStream` yields `Some(WatchEvent)`

#### Scenario: Stream termination on lag
- **WHEN** the subscriber falls behind and `RecvError::Lagged(n)` occurs
- **THEN** `WatchStream` yields `None` (stream terminates)
- **AND** a warning is logged with the number of missed events

#### Scenario: Stream termination on channel close
- **WHEN** the channel is closed
- **THEN** `WatchStream` yields `None` (stream terminates)

### Requirement: WatchStream is Send
The `WatchStream` type SHALL be `Send` to support Axum SSE handlers across thread boundaries.

### Requirement: EventBus is Clone
The `EventBus` type SHALL be `Clone` so it can be stored in Axum `State` and extracted by handlers.

### Requirement: Multi-subscriber delivery
The system SHALL deliver each published event to all active subscribers of that kind.

#### Scenario: Multiple subscribers receive same event
- **WHEN** two subscribers are subscribed to the same kind
- **AND** an event is published
- **THEN** both subscribers receive the event

### Requirement: Dropped subscriber does not block publisher
The system SHALL continue publishing to remaining subscribers when one subscriber drops.

#### Scenario: Subscriber drop during publishing
- **WHEN** a subscriber's `WatchStream` is dropped
- **AND** an event is published
- **THEN** remaining subscribers receive the event
- **AND** no panic or error occurs
