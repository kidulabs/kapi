## ADDED Requirements

### Requirement: Typed resource lifecycle helpers

The `TypedResource` trait SHALL provide default helpers for lifecycle inspection and conversion to the raw `StoredObject` representation. The helpers SHALL use the resource’s existing key, metadata, system metadata, spec, and status accessors.

#### Scenario: Detect a resource marked for deletion

- **WHEN** a typed resource has `system.deletion_timestamp` set
- **THEN** its deletion-state helper SHALL return `true`

#### Scenario: Detect a resource that is not being deleted

- **WHEN** a typed resource has no `system.deletion_timestamp`
- **THEN** its deletion-state helper SHALL return `false`

#### Scenario: Detect a finalizer

- **WHEN** a typed resource’s metadata contains a requested finalizer name
- **THEN** its finalizer-presence helper SHALL return `true`

#### Scenario: Convert a typed resource to a stored object

- **WHEN** a typed resource is converted to `StoredObject`
- **THEN** the result SHALL preserve its `ResourceKey`, metadata, system metadata, serialized spec, and serialized status

#### Scenario: Report conversion serialization failure

- **WHEN** spec or status serialization fails during conversion
- **THEN** the helper SHALL return `TypedError::Serialization`

### Requirement: Backward-compatible typed-resource implementations

The lifecycle and conversion helpers SHALL have default implementations so existing `TypedResource` implementations do not need to add methods immediately.

#### Scenario: Existing typed resource implementation

- **WHEN** an existing type implements the pre-change `TypedResource` methods
- **THEN** it SHALL continue to compile and receive the default lifecycle and conversion behavior
