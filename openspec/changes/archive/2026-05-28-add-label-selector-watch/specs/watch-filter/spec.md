## MODIFIED Requirements

### Requirement: WatchFilter enum variants
`WatchFilter` SHALL have three variants: `All`, `FieldSelector(FieldSelector)`, and `LabelSelector(LabelSelector)`.

#### Scenario: WatchFilter::All matches everything
- **WHEN** `WatchFilter::All` is evaluated against any event
- **THEN** it SHALL match (return true)

#### Scenario: WatchFilter::FieldSelector delegates to field matching
- **WHEN** `WatchFilter::FieldSelector(fs)` is evaluated against an event
- **THEN** it SHALL delegate to `fs.matches(event)`

#### Scenario: WatchFilter::LabelSelector delegates to label matching
- **WHEN** `WatchFilter::LabelSelector(ls)` is evaluated against an event
- **THEN** it SHALL delegate to `ls.matches(&event.object.metadata.labels)`

### Requirement: WatchFilter matches method
`WatchFilter::matches()` SHALL evaluate the filter against a `WatchEvent` and return true if the event should be delivered to the watcher.

#### Scenario: LabelSelector matches event labels
- **WHEN** `WatchFilter::LabelSelector` with `Equals{key:"app", value:"nginx"}` is evaluated against an event with object labels `{"app": "nginx"}`
- **THEN** it SHALL return true

#### Scenario: LabelSelector does not match event labels
- **WHEN** `WatchFilter::LabelSelector` with `Equals{key:"app", value:"nginx"}` is evaluated against an event with object labels `{"app": "apache"}`
- **THEN** it SHALL return false
