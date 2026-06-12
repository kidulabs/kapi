## MODIFIED Requirements

### Requirement: ObjectService wraps store, event bus, and schema registry
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator

The service SHALL be the single owner of system metadata manipulation (resource_version, generation, timestamps). The store SHALL NOT modify these fields. The service SHALL operate on `spec` and `status` as `serde_json::Value` directly, with no `SpecData` envelope construction or unwrapping.

#### Scenario: Service construction with SchemaRegistry
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called
- **THEN** the service constructs a `SchemaRegistry` internally from `store` and `meta_validator`
- **AND** the registry's cache starts empty
- **AND** no store query is performed during construction

### Requirement: create delegates schema work to SchemaRegistry and sets metadata
The `create(key, meta, spec)` method SHALL:
1. Validate `meta.labels` using label validation rules (key format, value format, length limits)
2. If `key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)` to validate and compile, then construct a `StoredObject` with `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`, with `spec` set directly to the `Value` and `status: None`, then `store.create()`, then `schema_registry.insert()` to cache, then `event_bus.publish()`
3. If `key.kind != "Schema"`: call `schema_registry.get_validator(&key)` to obtain the validator, validate `spec` against it, then construct a `StoredObject` with initial metadata, then `store.create()`, then `event_bus.publish()`

The service SHALL set `StoredObject.spec` to the `serde_json::Value` directly. There SHALL be no `SpecData { value: ... }` construction anywhere in the service.

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passed meta-schema validation and its jsonSchema compiles
- **THEN** the service SHALL construct a `StoredObject` with `resource_version = 1`, `generation = 1`, and current timestamps, with `spec` set to the validated `Value` directly
- **AND** the schema is stored with the generated name, the compiled validator is cached under that name, and an `Added` event is published

#### Scenario: Create object with initial metadata
- **WHEN** creating a regular object
- **THEN** the service SHALL set `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`, with `spec` set to the validated `Value` directly
- **AND** the store SHALL persist the object with those exact metadata values

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

### Requirement: update delegates schema work to SchemaRegistry and uses centralized metadata
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. If `object.key.kind == "Schema"`: call `schema_registry.validate_and_compile(&object.spec)`, then use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata, then `schema_registry.insert()`, then `event_bus.publish()`
3. If `object.key.kind != "Schema"`: call `schema_registry.get_validator(&key)`, validate spec, then use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata, then `event_bus.publish()`

The service SHALL use a centralized metadata wrapper that automatically handles:
- `resource_version = existing.resource_version + 1`
- `generation = existing.generation + 1` if `existing.spec != new_obj.spec` (direct `Value` equality), else `existing.generation`
- `updated_at = Utc::now()`
- `created_at = existing.created_at` (preserved)

The service SHALL perform OCC (optimistic concurrency control) check inside the transaction callback: if `object.system.resource_version != existing.system.resource_version`, return `TransactionOp::Abort(AppError::Conflict)`.

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

## MODIFIED Requirements

### Requirement: Service provides centralized metadata wrapper
The service SHALL provide a helper function `apply_with_metadata` that wraps transaction callbacks and automatically handles system metadata updates. The wrapper SHALL:
- Accept a callback that mutates the object (domain changes only)
- Automatically set `resource_version = existing.resource_version + 1`
- Automatically set `updated_at = Utc::now()`
- Automatically preserve `created_at = existing.created_at`
- Automatically bump `generation` if `new_obj.spec != existing.spec` (direct `Value` equality via `serde_json::Value`'s `PartialEq`), else preserve it
- Return `TransactionOp::Apply` with the updated object

The wrapper compares `spec` directly as `serde_json::Value`. There is no `.value` field access in the wrapper.

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

## REMOVED Requirements

### Requirement: Service provides centralized metadata wrapper (with spec.value comparison)
**Reason**: The previous requirement described the wrapper comparing `new_obj.spec.value != existing.spec.value`. After deleting the `SpecData` envelope, `spec` is a `serde_json::Value` directly and the comparison is `new_obj.spec != existing.spec` (no `.value` field access).
**Migration**: The wrapper now compares `spec` directly via `serde_json::Value`'s `PartialEq`. All other wrapper semantics (rv increment, timestamp updates, created_at preservation) are preserved unchanged.

### Requirement: Service performs OCC check in transaction callback (with spec.value in wrapper)
**Reason**: The previous requirement described the OCC check alongside the wrapper's `spec.value` comparison logic. After deleting the `SpecData` envelope, the OCC check is unchanged but its surrounding context no longer references `.value`.
**Migration**: The OCC check semantics (compare resource_version, return `TransactionOp::Abort(AppError::Conflict)` on mismatch) are preserved unchanged.
