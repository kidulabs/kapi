## MODIFIED Requirements

### Requirement: EventBus supports namespace-scoped subscriptions
The `EventBus` SHALL support namespace-scoped subscriptions via `WatchFilter::Namespace`. The `publish()` method SHALL deliver events to watchers whose filter matches, including `WatchFilter::Namespace` filters that check `event.object.metadata.namespace`.

#### Scenario: Publish delivers to namespace-scoped watcher
- **WHEN** an event is published for an object in "production" namespace
- **AND** a watcher is subscribed with `WatchFilter::Namespace("production")`
- **THEN** the watcher SHALL receive the event

#### Scenario: Publish does not deliver to different namespace watcher
- **WHEN** an event is published for an object in "production" namespace
- **AND** a watcher is subscribed with `WatchFilter::Namespace("staging")`
- **THEN** the watcher SHALL NOT receive the event

#### Scenario: Publish delivers to all-namespaces watcher
- **WHEN** an event is published for an object in any namespace
- **AND** a watcher is subscribed with `WatchFilter::All`
- **THEN** the watcher SHALL receive the event
