## Purpose

Define the `LabelSelector` and `LabelRequirement` types for filtering objects by label key-value pairs, including parsing from query strings and matching semantics.

## Requirements

### Requirement: LabelSelector type
The system SHALL provide a `LabelSelector` type containing a vector of `LabelRequirement` conditions. An empty requirements vector SHALL match all objects.

#### Scenario: Empty selector matches all
- **WHEN** a `LabelSelector` with no requirements is evaluated against any object
- **THEN** it SHALL match (return true)

#### Scenario: Selector with requirements
- **WHEN** a `LabelSelector` with requirements `[Equals{key:"app", value:"nginx"}]` is evaluated
- **THEN** it SHALL match only objects with label `app=nginx`

### Requirement: LabelRequirement equality
The `LabelRequirement::Equals` variant SHALL match when the object has a label with the specified key and value.

#### Scenario: Key exists with matching value
- **WHEN** `Equals{key:"app", value:"nginx"}` is evaluated against labels `{"app": "nginx", "env": "prod"}`
- **THEN** it SHALL match

#### Scenario: Key exists with different value
- **WHEN** `Equals{key:"app", value:"nginx"}` is evaluated against labels `{"app": "apache"}`
- **THEN** it SHALL NOT match

#### Scenario: Key does not exist
- **WHEN** `Equals{key:"app", value:"nginx"}` is evaluated against labels `{"env": "prod"}`
- **THEN** it SHALL NOT match

### Requirement: LabelRequirement inequality
The `LabelRequirement::NotEquals` variant SHALL match when the object does not have the specified key, or has the key with a different value.

#### Scenario: Key exists with different value
- **WHEN** `NotEquals{key:"env", value:"prod"}` is evaluated against labels `{"env": "staging"}`
- **THEN** it SHALL match

#### Scenario: Key exists with matching value
- **WHEN** `NotEquals{key:"env", value:"prod"}` is evaluated against labels `{"env": "prod"}`
- **THEN** it SHALL NOT match

#### Scenario: Key does not exist
- **WHEN** `NotEquals{key:"env", value:"prod"}` is evaluated against labels `{"app": "nginx"}`
- **THEN** it SHALL match (absence satisfies inequality)

### Requirement: LabelRequirement existence
The `LabelRequirement::Exists` variant SHALL match when the object has a label with the specified key, regardless of value.

#### Scenario: Key exists
- **WHEN** `Exists{key:"gpu"}` is evaluated against labels `{"gpu": "true"}`
- **THEN** it SHALL match

#### Scenario: Key exists with empty value
- **WHEN** `Exists{key:"gpu"}` is evaluated against labels `{"gpu": ""}`
- **THEN** it SHALL match

#### Scenario: Key does not exist
- **WHEN** `Exists{key:"gpu"}` is evaluated against labels `{"app": "nginx"}`
- **THEN** it SHALL NOT match

### Requirement: LabelRequirement non-existence
The `LabelRequirement::NotExists` variant SHALL match when the object does not have a label with the specified key.

#### Scenario: Key does not exist
- **WHEN** `NotExists{key:"experimental"}` is evaluated against labels `{"app": "nginx"}`
- **THEN** it SHALL match

#### Scenario: Key exists
- **WHEN** `NotExists{key:"experimental"}` is evaluated against labels `{"experimental": "true"}`
- **THEN** it SHALL NOT match

### Requirement: LabelSelector AND semantics
When a `LabelSelector` has multiple requirements, ALL requirements MUST match (AND semantics).

#### Scenario: All requirements match
- **WHEN** selector has `[Equals{key:"app", value:"nginx"}, Exists{key:"env"}]` and labels are `{"app": "nginx", "env": "prod"}`
- **THEN** it SHALL match

#### Scenario: One requirement fails
- **WHEN** selector has `[Equals{key:"app", value:"nginx"}, Exists{key:"env"}]` and labels are `{"app": "nginx"}`
- **THEN** it SHALL NOT match (missing `env` label)

### Requirement: Label selector parsing
The system SHALL parse label selector strings into `LabelSelector` types. Supported syntax: `key=value` (equality), `key!=value` (inequality), `key` (existence), `!key` (non-existence), comma-separated (AND).

#### Scenario: Parse equality selector
- **WHEN** parsing `"app=nginx"`
- **THEN** result SHALL be `LabelSelector{requirements: [Equals{key:"app", value:"nginx"}]}`

#### Scenario: Parse inequality selector
- **WHEN** parsing `"env!=prod"`
- **THEN** result SHALL be `LabelSelector{requirements: [NotEquals{key:"env", value:"prod"}]}`

#### Scenario: Parse existence selector
- **WHEN** parsing `"gpu"`
- **THEN** result SHALL be `LabelSelector{requirements: [Exists{key:"gpu"}]}`

#### Scenario: Parse non-existence selector
- **WHEN** parsing `"!experimental"`
- **THEN** result SHALL be `LabelSelector{requirements: [NotExists{key:"experimental"}]}`

#### Scenario: Parse AND combinator
- **WHEN** parsing `"app=nginx,env=prod"`
- **THEN** result SHALL be `LabelSelector{requirements: [Equals{key:"app", value:"nginx"}, Equals{key:"env", value:"prod"}]}`

#### Scenario: Parse mixed operators
- **WHEN** parsing `"app=nginx,!experimental,gpu"`
- **THEN** result SHALL be `LabelSelector{requirements: [Equals{key:"app", value:"nginx"}, NotExists{key:"experimental"}, Exists{key:"gpu"}]}`

#### Scenario: Parse empty string
- **WHEN** parsing `""`
- **THEN** result SHALL be `LabelSelector{requirements: []}` (matches all)

#### Scenario: Parse with whitespace
- **WHEN** parsing `"app=nginx, env=prod"`
- **THEN** result SHALL trim whitespace and parse correctly

#### Scenario: Parse malformed selector
- **WHEN** parsing `"app="` (empty value)
- **THEN** result SHALL be an `InvalidLabelSelector` error

#### Scenario: Parse selector with invalid key
- **WHEN** parsing `"invalid key!=value"` (space in key)
- **THEN** result SHALL be an `InvalidLabelSelector` error
