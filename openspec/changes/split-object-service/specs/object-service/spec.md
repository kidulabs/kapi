## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: create delegates schema work to SchemaRegistry and sets metadata
**Reason**: Schema lifecycle (create, update, delete) is extracted to SchemaService. ObjectService now only handles regular object operations.
**Migration**: Schema create operations are routed to `SchemaService::create()` by the handler.

### Requirement: update delegates schema work to SchemaRegistry and uses centralized metadata
**Reason**: Schema lifecycle is extracted to SchemaService. ObjectService update now only handles regular objects.
**Migration**: Schema update operations are routed to `SchemaService::update()` by the handler.

### Requirement: delete delegates cache eviction to SchemaRegistry
**Reason**: Schema deletion is extracted to SchemaService. ObjectService delete now only handles regular objects with finalizer lifecycle.
**Migration**: Schema delete operations are routed to `SchemaService::delete()` by the handler.

### Requirement: Schema cache uses schema name as key
**Reason**: Schema cache management is now the responsibility of SchemaService.
**Migration**: See `schema-service` spec for cache key requirements.

### Requirement: Schema compilation uses JsonSchemaValidator via SchemaRegistry
**Reason**: Schema compilation orchestration is now the responsibility of SchemaService.
**Migration**: See `schema-service` spec for compilation requirements.

### Requirement: Schema registration compiles status validator
**Reason**: Status validator compilation is now the responsibility of SchemaService.
**Migration**: See `schema-service` spec for status validator requirements.
