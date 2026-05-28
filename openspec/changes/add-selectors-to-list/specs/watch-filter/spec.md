## MODIFIED Requirements

### Requirement: WatchFilter And combinator
`WatchFilter` SHALL include an `And(Box<WatchFilter>, Box<WatchFilter>)` variant for composing two filters with AND semantics.

#### Scenario: And combinator matches when both filters match
- **WHEN** `WatchFilter::And(FieldSelector(NameEquals("foo")), LabelSelector(Equals{key:"app", value:"nginx"}))` is evaluated against an event with `name="foo"` and labels `{"app": "nginx"}`
- **THEN** it SHALL return true

#### Scenario: And combinator fails when first filter fails
- **WHEN** `WatchFilter::And(FieldSelector(NameEquals("foo")), LabelSelector(...))` is evaluated against an event with `name="bar"`
- **THEN** it SHALL return false (short-circuit on first filter)

#### Scenario: And combinator fails when second filter fails
- **WHEN** `WatchFilter::And(FieldSelector(NameEquals("foo")), LabelSelector(Equals{key:"app", value:"nginx"}))` is evaluated against an event with `name="foo"` and labels `{"app": "apache"}`
- **THEN** it SHALL return false

#### Scenario: Nested And combinators
- **WHEN** `WatchFilter::And(And(a, b), c)` is evaluated
- **THEN** it SHALL match only when all three filters (a, b, c) match

### Requirement: WatchFilter matches method with And
`WatchFilter::matches()` SHALL evaluate `And(a, b)` as `a.matches(event) && b.matches(event)`.

#### Scenario: And evaluation order
- **WHEN** `WatchFilter::And(a, b)` is evaluated
- **THEN** `a.matches(event)` SHALL be evaluated first, and `b.matches(event)` SHALL be evaluated only if `a` matches (short-circuit)
