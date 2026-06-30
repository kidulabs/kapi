## Purpose

Define namespace-scoped watch filtering that enables clients to subscribe to events for objects within a specific namespace. This allows fine-grained event delivery where watchers can receive notifications only for objects in a particular namespace, reducing noise and bandwidth for namespace-scoped clients.

## Requirements

### Requirement: WatchFilter::Namespace variant exists
The `WatchFilter` enum SHALL include a `Namespace(String)` variant. The `matches()` method SHALL check if `event.object.metadata.namespace == Some(namespace)`.

#### Scenario: WatchFilter::Namespace matches events in namespace
- **WHEN** `WatchFilter::Namespace("production".to_string()).matches(&event)` is called
- **AND** `event.object.metadata.namespace == Some("production")`
- **THEN** the result SHALL be `true`

#### Scenario: WatchFilter::Namespace rejects events in different namespace
- **WHEN** `WatchFilter::Namespace("production".to_string()).matches(&event)` is called
- **AND** `event.object.metadata.namespace == Some("staging")`
- **THEN** the result SHALL be `false`

#### Scenario: WatchFilter::Namespace rejects cluster-scoped events
- **WHEN** `WatchFilter::Namespace("production".to_string()).matches(&event)` is called
- **AND** `event.object.metadata.namespace == None`
- **THEN** the result SHALL be `false`

### Requirement: WatchFilter::Namespace composable with And
The `WatchFilter::Namespace` variant SHALL be composable with other filters using `WatchFilter::And`.

#### Scenario: Namespace AND label selector
- **WHEN** `WatchFilter::And(Box::new(WatchFilter::Namespace("production".to_string())), Box::new(WatchFilter::LabelSelector(...)))` is used
- **THEN** events SHALL match only if they are in "production" namespace AND match the label selector

#### Scenario: Namespace AND field selector
- **WHEN** `WatchFilter::And(Box::new(WatchFilter::Namespace("production".to_string())), Box::new(WatchFilter::FieldSelector(...)))` is used
- **THEN** events SHALL match only if they are in "production" namespace AND match the field selector

### Requirement: Watch endpoint supports namespace-scoped subscription
The watch handler SHALL support namespace-scoped subscriptions. When watching a namespaced kind at `/apis/{group}/{version}/namespaces/{namespace}/{kind}?watch=true`, the handler SHALL create a `WatchFilter::Namespace(namespace)` and combine it with any field/label selectors using `WatchFilter::And`.

#### Scenario: Namespace-scoped watch
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget?watch=true`
- **THEN** the SSE stream SHALL only deliver events for objects in "production" namespace

#### Scenario: Namespace-scoped watch with label selector
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget?watch=true&labelSelector=app=nginx`
- **THEN** the SSE stream SHALL only deliver events for objects in "production" namespace with label `app=nginx`

#### Scenario: Cross-namespace watch
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true` (namespaced kind, no namespace in URL)
- **THEN** the SSE stream SHALL deliver events for objects across all namespaces (WatchFilter::All)
