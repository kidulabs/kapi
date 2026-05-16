## ADDED Requirements

### Requirement: EventPublisher trait abstracts event distribution
The system SHALL define an `EventPublisher` trait with `publish(&self, key: &ResourceKey, event: WatchEvent)` and `subscribe(&self, key: &ResourceKey) -> WatchStream` methods. The trait SHALL require `Send + Sync`.

#### Scenario: Trait is object-safe
- **WHEN** a type implements `EventPublisher`
- **THEN** it can be used as `dyn EventPublisher` inside `Arc`

#### Scenario: EventBus implements EventPublisher
- **WHEN** `EventBus` is constructed
- **THEN** it implements all `EventPublisher` trait methods, delegating to its existing inherent methods

### Requirement: EventPublisher re-exported from event module
The system SHALL re-export `EventPublisher` alongside `EventBus` and `WatchStream` from `src/event/mod.rs`.

#### Scenario: EventPublisher is importable from crate::event
- **WHEN** a module imports `use crate::event::EventPublisher`
- **THEN** the `EventPublisher` trait is in scope
