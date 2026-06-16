## Purpose

Define finalizer support for async object lifecycle management. Finalizers allow controllers to register interest in an object's cleanup, deferring hard deletion until all finalizers are removed.

## Requirements

### Requirement: Finalizers field on ObjectMeta
Objects SHALL carry a `finalizers` field of type `Vec<String>` in their metadata. The field SHALL default to an empty vector when not provided. Finalizer names SHALL be validated as label-key-shaped strings.

#### Scenario: Object created without finalizers
- **WHEN** a client creates an object without providing `metadata.finalizers`
- **THEN** the stored object SHALL have `metadata.finalizers` as an empty vector `[]`

#### Scenario: Object created with finalizers
- **WHEN** a client creates an object with `metadata.finalizers: ["example.io/cleanup", "kapi.io/finalizer"]`
- **THEN** the stored object SHALL have `metadata.finalizers` containing exactly those strings

#### Scenario: Finalizers serialized in API response
- **WHEN** an object is returned in any API response (create, get, list, update)
- **THEN** the `metadata.finalizers` field SHALL always be present, even if empty

### Requirement: Finalizer name validation
Finalizer names SHALL be validated using the same rules as label keys. Each finalizer name MUST be a valid label-key-shaped string (optional DNS subdomain prefix, `/`, then name). The maximum number of finalizers per object SHALL be 20.

#### Scenario: Valid finalizer name with prefix
- **WHEN** a finalizer name is `example.io/cleanup` or `kapi.io/finalizer`
- **THEN** validation SHALL pass

#### Scenario: Valid finalizer name without prefix
- **WHEN** a finalizer name is `cleanup` or `my-finalizer`
- **THEN** validation SHALL pass

#### Scenario: Invalid finalizer name
- **WHEN** a finalizer name contains invalid characters (e.g., spaces, special chars)
- **THEN** validation SHALL fail with an `InvalidFinalizer` error

#### Scenario: Too many finalizers
- **WHEN** an object has more than 20 finalizers
- **THEN** validation SHALL fail with an `InvalidFinalizer` error

### Requirement: deletion_timestamp field on SystemMetadata
Objects SHALL carry a `deletion_timestamp` field of type `Option<DateTime<Utc>>` in their system metadata. The field SHALL be server-managed and SHALL NOT be settable by clients. The field SHALL be omitted from JSON serialization when `None`.

#### Scenario: deletion_timestamp is None on create
- **WHEN** an object is created
- **THEN** the `system.deletionTimestamp` field SHALL be `None` and SHALL NOT appear in the JSON response

#### Scenario: deletion_timestamp is set on mark-for-deletion
- **WHEN** an object with finalizers is deleted
- **THEN** the `system.deletionTimestamp` field SHALL be set to the current time and SHALL appear in the JSON response

#### Scenario: deletion_timestamp is preserved on update
- **WHEN** an object with `deletionTimestamp` set is updated
- **THEN** the `system.deletionTimestamp` field SHALL be preserved (not cleared or changed)

### Requirement: DELETE with empty finalizers performs hard delete
When a DELETE request is made on an object with an empty `finalizers` list, the system SHALL immediately remove the object from storage and publish a `Deleted` event.

#### Scenario: DELETE object without finalizers
- **WHEN** a DELETE request is made on an object with `metadata.finalizers: []`
- **THEN** the object SHALL be removed from storage, a `Deleted` event SHALL be published, and the response SHALL be 200 OK with the deleted object

### Requirement: DELETE with non-empty finalizers marks for deletion
When a DELETE request is made on an object with a non-empty `finalizers` list, the system SHALL set `deletionTimestamp` and return the object without removing it from storage. A `Modified` event SHALL be published.

#### Scenario: DELETE object with finalizers
- **WHEN** a DELETE request is made on an object with `metadata.finalizers: ["example.io/cleanup"]`
- **THEN** the object SHALL remain in storage with `system.deletionTimestamp` set, a `Modified` event SHALL be published, and the response SHALL be 200 OK with the marked object

### Requirement: Idempotent DELETE on already-deleting object
When a DELETE request is made on an object that already has `deletionTimestamp` set, the system SHALL return 200 OK with the object and SHALL NOT publish any event.

#### Scenario: DELETE on object with deletionTimestamp already set
- **WHEN** a DELETE request is made on an object with `system.deletionTimestamp` already set
- **THEN** the object SHALL remain unchanged, no event SHALL be published, and the response SHALL be 200 OK with the object

### Requirement: UPDATE on deleting object rejects non-finalizer changes
When an object has `deletionTimestamp` set, the system SHALL reject any update that modifies fields other than `finalizers`. The error SHALL be `ObjectBeingDeleted` with HTTP 409 Conflict.

#### Scenario: UPDATE spec on deleting object
- **WHEN** an update request modifies `spec` on an object with `system.deletionTimestamp` set
- **THEN** the response SHALL be 409 Conflict with `ObjectBeingDeleted` error

#### Scenario: UPDATE labels on deleting object
- **WHEN** an update request modifies `metadata.labels` on an object with `system.deletionTimestamp` set
- **THEN** the response SHALL be 409 Conflict with `ObjectBeingDeleted` error

#### Scenario: UPDATE annotations on deleting object
- **WHEN** an update request modifies `metadata.annotations` on an object with `system.deletionTimestamp` set
- **THEN** the response SHALL be 409 Conflict with `ObjectBeingDeleted` error

### Requirement: UPDATE on deleting object rejects finalizer addition
When an object has `deletionTimestamp` set, the system SHALL reject any update that adds new finalizers. Only finalizer removal SHALL be allowed.

#### Scenario: UPDATE adds finalizer on deleting object
- **WHEN** an update request adds a new finalizer to `metadata.finalizers` on an object with `system.deletionTimestamp` set
- **THEN** the response SHALL be 409 Conflict with `ObjectBeingDeleted` error

#### Scenario: UPDATE removes finalizer on deleting object
- **WHEN** an update request removes a finalizer from `metadata.finalizers` on an object with `system.deletionTimestamp` set
- **THEN** the update SHALL succeed and a `Modified` event SHALL be published

### Requirement: UPDATE that empties finalizers on deleting object performs hard delete
When an update request removes all finalizers from an object with `deletionTimestamp` set, the system SHALL immediately remove the object from storage and publish a `Deleted` event.

#### Scenario: UPDATE empties finalizers on deleting object
- **WHEN** an update request sets `metadata.finalizers: []` on an object with `system.deletionTimestamp` set
- **THEN** the object SHALL be removed from storage, a `Deleted` event SHALL be published, and the response SHALL be 200 OK with the deleted object (including `deletionTimestamp`)

### Requirement: ObjectBeingDeleted error variant
The system SHALL define an `ObjectBeingDeleted { name: String }` variant in `AppError` that returns HTTP 409 Conflict with the error message.

#### Scenario: ObjectBeingDeleted returns 409
- **WHEN** an `AppError::ObjectBeingDeleted { name }` is returned from a handler
- **THEN** the HTTP response SHALL be 409 Conflict with the error message

### Requirement: InvalidFinalizer error variant
The system SHALL define an `InvalidFinalizer(String)` variant in `AppError` that returns HTTP 400 Bad Request with the error message.

#### Scenario: InvalidFinalizer returns 400
- **WHEN** an `AppError::InvalidFinalizer(msg)` is returned from a handler
- **THEN** the HTTP response SHALL be 400 Bad Request with the error message

### Requirement: Finalizers on Schema objects
Schema objects SHALL NOT support finalizers in v1. The finalizer field SHALL be ignored for Schema objects, and DELETE on Schema objects SHALL follow the existing `SchemaHasObjects` guard logic.

#### Scenario: Schema created with finalizers
- **WHEN** a client creates a Schema with `metadata.finalizers: ["example.io/cleanup"]`
- **THEN** the finalizers SHALL be stored but SHALL NOT affect DELETE behavior (Schema deletion follows `SchemaHasObjects` guard)

### Requirement: Backward compatibility with existing objects
Objects stored before finalizer support was added SHALL deserialize correctly with `finalizers` defaulting to an empty vector and `deletionTimestamp` defaulting to `None`.

#### Scenario: Existing object without finalizers field
- **WHEN** an object stored before finalizer support is read from storage
- **THEN** the `metadata.finalizers` field SHALL default to an empty vector

#### Scenario: Existing object without deletionTimestamp field
- **WHEN** an object stored before finalizer support is read from storage
- **THEN** the `system.deletionTimestamp` field SHALL default to `None`

### Requirement: CREATE same-name after DELETE-with-finalizers
When an object has `deletionTimestamp` set (marked for deletion but not yet deleted), a CREATE request with the same name SHALL fail with `AlreadyExists` error.

#### Scenario: CREATE same-name on deleting object
- **WHEN** a CREATE request is made with the same name as an object with `system.deletionTimestamp` set
- **THEN** the response SHALL be 409 Conflict with `AlreadyExists` error

### Requirement: Event semantics for mark-for-deletion
When an object is marked for deletion (DELETE with non-empty finalizers), the system SHALL publish a `Modified` event with the object containing `deletionTimestamp` set. The `resource_version` SHALL be bumped.

#### Scenario: Mark-for-deletion publishes Modified event
- **WHEN** an object with finalizers is deleted
- **THEN** a `Modified` event SHALL be published with `system.deletionTimestamp` set and `resource_version` bumped

### Requirement: Event semantics for hard delete
When an object is hard-deleted (DELETE with empty finalizers, or UPDATE that empties finalizers on a deleting object), the system SHALL publish a `Deleted` event with the pre-deletion object.

#### Scenario: Hard delete publishes Deleted event
- **WHEN** an object is hard-deleted
- **THEN** a `Deleted` event SHALL be published with the pre-deletion object (including `deletionTimestamp` if it was set)

### Requirement: Event suppression on idempotent DELETE
When a DELETE request is made on an object that already has `deletionTimestamp` set and no state change occurs, the system SHALL NOT publish any event.

#### Scenario: Idempotent DELETE suppresses event
- **WHEN** a DELETE request is made on an object with `system.deletionTimestamp` already set
- **THEN** no event SHALL be published
