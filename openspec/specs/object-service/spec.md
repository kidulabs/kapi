## Purpose

Define the `ObjectService` that orchestrates validation, storage, and event publishing for all object operations. The service is the single entry point for object CRUD — handlers call the service, never the store directly.
## Requirements
### Requirement: ObjectService wraps store, event bus, and schema registry
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator (received from SchemaService or constructed independently)

The service SHALL be the single owner of system metadata manipulation (resource_version, generation, timestamps) for regular objects. The store SHALL NOT modify these fields. The service SHALL operate on `spec` and `status` as `serde_json::Value` directly, with no `SpecData` envelope construction or unwrapping.

The ObjectService SHALL NOT handle Schema lifecycle operations (Schema create, update, delete). Those are the responsibility of SchemaService.

#### Scenario: Service construction with SchemaRegistry
- **WHEN** `ObjectService::new(store, event_bus, schema_registry)` is called
- **THEN** the service SHALL be constructed with the provided SchemaRegistry
- **AND** the registry's cache SHALL be shared with SchemaService

### Requirement: create validates spec against schema and sets metadata
The `create(key, meta, spec)` method SHALL:
1. Validate `meta.labels` using label validation rules (key format, value format, length limits)
2. Validate `meta.annotations` using annotation validation rules (key format, total size limit)
3. Call `schema_registry.get_validator(&key)` to obtain the validator for the object's kind
4. Validate `spec` against the compiled schema validator
5. Construct a `StoredObject` with `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`
6. Call `store.create()` to persist
7. Call `event_bus.publish()` with an `Added` event

The service SHALL construct the complete `StoredObject` with all system metadata before calling `store.create()`. The store SHALL persist the object as-is.

The service SHALL set `StoredObject.spec` to the `serde_json::Value` directly. There SHALL be no `SpecData { value: ... }` construction anywhere in the service.

Label and annotation validation SHALL occur in the service to ensure non-HTTP callers (tests, future gRPC/CLI) receive the same validation guarantees. If validation fails, an `AppError::InvalidLabel` or `AppError::InvalidAnnotation` error SHALL be returned with a descriptive message.

#### Scenario: Create object with initial metadata
- **WHEN** creating a regular object
- **THEN** the service SHALL set `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`, with `spec` set to the validated `Value` directly
- **AND** the store SHALL persist the object with those exact metadata values

#### Scenario: Create object for unregistered kind
- **WHEN** creating an object for a kind with no registered Schema
- **THEN** the error is `NotFound` (no schema found for this kind)

#### Scenario: Create object with invalid spec
- **WHEN** creating an object whose spec fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` fetches the schema from the store, compiles it on-demand, caches it, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `schema_registry.get_validator()` returns `AppError::StoredSchemaCompilationFailed`
- **AND** no object is created

#### Scenario: Create with valid labels
- **WHEN** `create()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with labels

#### Scenario: Create with invalid label key
- **WHEN** `create()` is called with a label key that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with invalid label value
- **WHEN** `create()` is called with a label value that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with empty labels map
- **WHEN** `create()` is called with an empty labels `HashMap`
- **THEN** validation SHALL pass and the object SHALL be persisted

#### Scenario: Create with invalid annotations
- **WHEN** `create()` is called with annotations that violate format rules (empty key, key exceeding 256 chars, or total size exceeding 256KB)
- **THEN** an `AppError::InvalidAnnotation` error SHALL be returned with a descriptive message

### Requirement: get delegates to store
The `get(key, name)` method SHALL delegate to `store.get(key, name)` without additional validation.

#### Scenario: Get existing object
- **WHEN** `get` is called for an existing object
- **THEN** the `StoredObject` is returned

#### Scenario: Get missing object
- **WHEN** `get` is called for a non-existent object
- **THEN** the error is `NotFound`

### Requirement: list delegates to store
The `list(key, opts)` method SHALL delegate to `store.list(key, opts)` without additional validation.

#### Scenario: List objects with pagination
- **WHEN** `list` is called with limit and continue token
- **THEN** the paginated `ListResponse` is returned

### Requirement: update validates spec and uses centralized metadata
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. Validate `object.metadata.annotations` using annotation validation rules
3. Call `schema_registry.get_validator(&key)` to obtain the validator
4. Validate spec against the compiled schema validator
5. Use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata

The service SHALL use a centralized metadata wrapper that automatically handles:
- `resource_version = existing.resource_version + 1`
- `generation = existing.generation + 1` if `existing.spec != new_obj.spec` (direct `Value` equality), else `existing.generation`
- `updated_at = Utc::now()`
- `created_at = existing.created_at` (preserved)

The service SHALL perform OCC (optimistic concurrency control) check inside the transaction callback: if `object.system.resource_version != existing.system.resource_version`, return `TransactionOp::Abort(AppError::Conflict)`.

Label and annotation validation SHALL occur in the service to ensure non-HTTP callers receive the same validation guarantees. If validation fails, an `AppError::InvalidLabel` or `AppError::InvalidAnnotation` error SHALL be returned and no persistence SHALL occur.

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the service SHALL increment `resource_version`, preserve `created_at`, update `updated_at`, bump `generation` if spec changed, and publish a `Modified` event

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the transaction callback SHALL return `TransactionOp::Abort(AppError::Conflict)` and no event is published

#### Scenario: Update with same spec does not bump generation
- **WHEN** `update` is called with the same `spec` (direct `Value` equality) but different `metadata.labels`
- **THEN** the centralized metadata wrapper SHALL preserve `generation` (not increment it)

#### Scenario: Update with different spec bumps generation
- **WHEN** `update` is called with a different `spec` `Value`
- **THEN** the centralized metadata wrapper SHALL increment `generation` by 1

#### Scenario: Update with valid labels
- **WHEN** `update()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with updated labels

#### Scenario: Update with invalid labels
- **WHEN** `update()` is called with invalid labels
- **THEN** an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur

#### Scenario: Update with invalid annotations
- **WHEN** `update()` is called with annotations that violate format rules
- **THEN** an `AppError::InvalidAnnotation` error SHALL be returned and no persistence SHALL occur

### Requirement: delete handles finalizer lifecycle for regular objects
The `delete(key, name)` method SHALL handle the finalizer-based deletion lifecycle for regular (non-Schema) objects:
1. Fetch the existing object from the store
2. If `finalizers` is empty: hard-delete via `store.transaction()` with `TransactionOp::Delete`, publish `Deleted` event
3. If `finalizers` is non-empty and `deletion_timestamp` is None: mark for deletion via `store.transaction()`, set `deletion_timestamp`, publish `Modified` event
4. If `deletion_timestamp` is already set: return the object without changes, no event published (idempotent)

The ObjectService SHALL NOT handle Schema deletion. Schema deletion is the responsibility of SchemaService.

#### Scenario: Delete regular object without finalizers
- **WHEN** deleting a non-Schema object with empty finalizers
- **THEN** the object SHALL be hard-deleted, a `Deleted` event SHALL be published, and the deleted object SHALL be returned

#### Scenario: Delete regular object with finalizers marks for deletion
- **WHEN** deleting a non-Schema object with non-empty finalizers
- **THEN** the object SHALL remain in storage with `system.deletionTimestamp` set, a `Modified` event SHALL be published

#### Scenario: Idempotent delete on already-deleting object
- **WHEN** deleting an object that already has `deletionTimestamp` set
- **THEN** the object SHALL remain unchanged, no event SHALL be published, and the response SHALL be 200 OK with the object
### Requirement: Service provides subscribe() with WatchFilter for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe(key, filter)`.

#### Scenario: Subscribe with WatchFilter::All returns a WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::All)` is called
- **THEN** a `WatchStream` is returned that delivers all events for the given resource key

#### Scenario: Subscribe with WatchFilter::FieldSelector returns a filtered WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())))` is called
- **THEN** a `WatchStream` is returned that delivers only events matching the filter

### Requirement: ObjectService create ignores status field
The `create` method SHALL ignore any `status` field present in the request body. Objects are always created with `status: None`.

#### Scenario: Create with status in body
- **WHEN** `create` is called with a body containing a `status` field
- **THEN** the `status` field SHALL be removed from the body before storage
- **AND** the created object SHALL have `status: None`

### Requirement: ObjectService get_status method returns Value directly
The `ObjectService` SHALL provide a `get_status(key: ResourceKey, name: String) -> Result<Option<Value>, AppError>` method that:
1. Looks up the Schema for the given kind to check if `statusSchema` is defined
2. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
3. Fetches the object from the store
4. Returns the `status` field as `Option<serde_json::Value>` directly (no envelope)

#### Scenario: Get status for object with status set
- **WHEN** `get_status` is called for an object whose `status` is `Some(Value::Object({"phase": "Running"}))`
- **THEN** the method SHALL return `Ok(Some(Value::Object({"phase": "Running"})))`

#### Scenario: Get status for object without status set
- **WHEN** `get_status` is called for an object whose `status` is `None`
- **THEN** the method SHALL return `Ok(None)`

#### Scenario: Get status for kind without statusSchema
- **WHEN** `get_status` is called for a kind whose Schema has no `statusSchema`
- **THEN** the method SHALL return `Err(AppError::StatusSubresourceNotEnabled { kind })`

### Requirement: ObjectService update_status method uses centralized metadata
The `ObjectService` SHALL provide an `update_status(key: ResourceKey, name: String, status: Value)` method that:
1. Looks up the Schema for the given kind to check if `statusSchema` is defined
2. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
3. Validates the status value against the `statusSchema`
4. If validation fails, returns `AppError::SchemaValidation`
5. Uses `store.transaction()` with a callback that modifies only the `status` field and returns `TransactionOp::Apply` via the centralized metadata wrapper
6. Publishes a `WatchEvent` with `event_type: StatusModified` containing the updated `StoredObject`
7. Returns the updated `StoredObject`

The centralized metadata wrapper SHALL automatically preserve `generation` because the `spec` `Value` is not changed by the callback. The service SHALL set `updated.status = Some(status)` directly (no `SpecData { value: status }` construction).

#### Scenario: Update status for kind with statusSchema
- **WHEN** `update_status` is called for a kind with `statusSchema` defined
- **THEN** the status is validated against `statusSchema`, stored with incremented `resource_version` and updated `updated_at`, `generation` is preserved, and a `StatusModified` event is published

#### Scenario: Update status does not bump generation
- **WHEN** `update_status` is called on an object with `generation: N`
- **THEN** the returned `StoredObject.system.generation` equals N (unchanged)
- **AND** the returned `StoredObject.system.resource_version` is incremented by 1

#### Scenario: Update status for kind without statusSchema
- **WHEN** `update_status` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

#### Scenario: Update status with invalid status
- **WHEN** `update_status` is called with a status value that fails `statusSchema` validation
- **THEN** the error is `AppError::SchemaValidation` with the list of validation errors

#### Scenario: Update status for non-existent object
- **WHEN** `update_status` is called for an object that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: Service provides centralized metadata wrapper
The service SHALL provide a helper function `apply_with_metadata` that wraps transaction callbacks and automatically handles system metadata updates. The wrapper SHALL:
- Accept a callback that mutates the object (domain changes only)
- Automatically set `resource_version = existing.resource_version + 1`
- Automatically set `updated_at = Utc::now()`
- Automatically preserve `created_at = existing.created_at`
- Automatically bump `generation` if `new_obj.spec != existing.spec` (direct `Value` equality via `serde_json::Value`'s `PartialEq`), else preserve it
- Return `TransactionOp::Apply` with the updated object

#### Scenario: Wrapper increments resource_version
- **WHEN** the wrapper is called with an existing object having `resource_version = 5`
- **THEN** the returned object SHALL have `resource_version = 6`

#### Scenario: Wrapper updates updated_at
- **WHEN** the wrapper is called
- **THEN** the returned object SHALL have `updated_at` set to the current time

#### Scenario: Wrapper preserves created_at
- **WHEN** the wrapper is called with an existing object having `created_at = T`
- **THEN** the returned object SHALL have `created_at = T` (unchanged)

#### Scenario: Wrapper bumps generation on spec change
- **WHEN** the wrapper is called and the callback changes `spec`
- **THEN** the returned object SHALL have `generation = existing.generation + 1`

#### Scenario: Wrapper preserves generation on no spec change
- **WHEN** the wrapper is called and the callback does not change `spec`
- **THEN** the returned object SHALL have `generation = existing.generation` (unchanged)

### Requirement: Service performs OCC check in transaction callback
The service SHALL perform optimistic concurrency control (OCC) checks inside transaction callbacks. When updating an object, the callback SHALL compare the incoming object's `resource_version` with the existing object's `resource_version`. If they do not match, the callback SHALL return `TransactionOp::Abort(AppError::Conflict)`.

#### Scenario: OCC check passes with matching version
- **WHEN** the callback is invoked with an existing object and the incoming object has a matching `resource_version`
- **THEN** the callback SHALL proceed with the update and return `TransactionOp::Apply`

#### Scenario: OCC check fails with mismatched version
- **WHEN** the callback is invoked with an existing object and the incoming object has a different `resource_version`
- **THEN** the callback SHALL return `TransactionOp::Abort(AppError::Conflict)` and no changes SHALL be made

### Requirement: Validation error mapping in object operations
The system SHALL map `SchemaValidationError` from `SchemaValidator::validate()` to the domain `ValidationError` type when validating regular objects. Meta-schema validation errors are handled internally by `SchemaRegistry::validate_and_compile()` and returned as `AppError::InvalidSchema`. A `validate_labels()` function SHALL validate a `HashMap<String, String>` against label validation rules, returning `Result<(), AppError>` with descriptive error messages identifying the offending key or value.

#### Scenario: Meta-schema errors mapped to strings
- **WHEN** meta-schema validation fails during Schema create or update
- **THEN** `schema_registry.validate_and_compile()` collects error messages and returns `AppError::InvalidSchema`

#### Scenario: Object validation errors mapped to ValidationError
- **WHEN** object validation fails during object create or update
- **THEN** `SchemaValidationError` values are mapped to `object::types::ValidationError { path, message }` and returned as `AppError::SchemaValidation`

#### Scenario: Validate key with prefix
- **WHEN** `validate_labels()` is called with key `app.example.io/name`
- **THEN** validation SHALL check prefix format (DNS subdomain, max 253 chars) and name format (max 256 chars, valid characters)

#### Scenario: Validate empty value
- **WHEN** `validate_labels()` is called with a label whose value is an empty string
- **THEN** validation SHALL pass (empty values are allowed)

### Requirement: Service publishes events after mutations only
The service SHALL publish events only after successful store operations. If the store returns an error, no event is published.

#### Scenario: Failed create does not publish
- **WHEN** `create` fails due to a duplicate (AlreadyExists) or validation error
- **THEN** no `Added` event is published

#### Scenario: Failed update does not publish
- **WHEN** `update` fails due to a version conflict
- **THEN** no `Modified` event is published

