## MODIFIED Requirements

### Requirement: Service provides subscribe() with WatchFilter for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe(key, filter)`.

#### Scenario: Subscribe with WatchFilter::All returns a WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::All)` is called
- **THEN** a `WatchStream` is returned that delivers all events for the given resource key

#### Scenario: Subscribe with WatchFilter::FieldSelector returns a filtered WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())))` is called
- **THEN** a `WatchStream` is returned that delivers only events matching the filter