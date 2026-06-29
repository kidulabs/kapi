## MODIFIED Requirements

### Requirement: ObjectService wraps store, event bus, and schema registry
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator

The service SHALL be the single owner of system metadata manipulation (resource_version, generation, timestamps) for regular objects. The store SHALL NOT modify these fields. The service SHALL operate on `spec` and `status` as `serde_json::Value` directly, with no `SpecData` envelope construction or unwrapping.

The ObjectService SHALL NOT handle Schema lifecycle operations (Schema create, update, delete). Those are the responsibility of SchemaService.

The service SHALL be responsible for scope validation: checking that the URL namespace matches the Schema scope and rejecting invalid combinations.

#### Scenario: Service construction with SchemaRegistry
- **WHEN** `ObjectService::new(store, event_bus, schema_registry)` is called
- **THEN** the service SHALL be constructed with the provided SchemaRegistry

### Requirement: create validates scope, namespace, spec, and sets metadata
The `create(key, namespace, meta, spec)` method SHALL:
1. Look up the Schema scope for the kind
2. Validate namespace vs scope:
   - If scope is "Cluster" and namespace is Some, reject with error
   - If scope is "Namespaced" and namespace is None, set namespace to "default"
3. Validate `meta.labels` using label validation rules
4. Validate `meta.annotations` using annotation validation rules
5. Call `schema_registry.get_validator(&key)` to obtain the validator
6. Validate `spec` against the compiled schema validator
7. Construct a `StoredObject` with `metadata.namespace = namespace`, `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`
8. Call `store.create()` to persist
9. Call `event_bus.publish()` with an `Added` event

The service SHALL discard any `namespace` from the input `meta` — the URL namespace (or "default") takes precedence.

#### Scenario: Create namespaced object with URL namespace
- **WHEN** creating an object with `namespace = Some("production")`
- **THEN** the object SHALL be stored with `metadata.namespace = Some("production")`

#### Scenario: Create namespaced object without namespace defaults to "default"
- **WHEN** creating an object with `namespace = None` for a namespaced kind
- **THEN** the object SHALL be stored with `metadata.namespace = Some("default")`

#### Scenario: Create cluster-scoped object
- **WHEN** creating an object with `namespace = None` for a cluster-scoped kind
- **THEN** the object SHALL be stored with `metadata.namespace = None`

#### Scenario: Create cluster-scoped object with namespace rejected
- **WHEN** creating an object with `namespace = Some("production")` for a cluster-scoped kind
- **THEN** the service SHALL reject with an error

#### Scenario: Create object with invalid spec
- **WHEN** creating an object whose spec fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object in same namespace
- **WHEN** creating an object with a name that already exists in the same namespace
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with same name in different namespace
- **WHEN** creating an object with a name that exists in a different namespace
- **THEN** the object SHALL be created successfully

### Requirement: get delegates to store with namespace
The `get(key, namespace, name)` method SHALL delegate to `store.get(key, namespace, name)` without additional validation.

#### Scenario: Get existing object
- **WHEN** `get` is called for an existing object
- **THEN** the `StoredObject` is returned

#### Scenario: Get missing object
- **WHEN** `get` is called for a non-existent object
- **THEN** the error is `NotFound`

### Requirement: list delegates to store with namespace
The `list(key, namespace, opts)` method SHALL delegate to `store.list(key, namespace, opts)` without additional validation. When `namespace` is `None`, objects from all namespaces are returned.

#### Scenario: List objects in namespace
- **WHEN** `list` is called with `namespace = Some("production")`
- **THEN** only objects in "production" namespace are returned

#### Scenario: List objects across all namespaces
- **WHEN** `list` is called with `namespace = None`
- **THEN** objects from all namespaces are returned

### Requirement: update validates scope, namespace, spec, and uses centralized metadata
The `update(object)` method SHALL:
1. Look up the Schema scope for the kind
2. Validate namespace vs scope (same rules as create)
3. Validate `object.metadata.labels` using label validation rules
4. Validate `object.metadata.annotations` using annotation validation rules
5. Call `schema_registry.get_validator(&key)` to obtain the validator
6. Validate spec against the compiled schema validator
7. Use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata

The service SHALL validate that `object.metadata.namespace` matches the expected namespace (from URL). If they don't match, reject with error.

#### Scenario: Update with correct namespace
- **WHEN** `update` is called with matching namespace
- **THEN** the update proceeds normally

#### Scenario: Update with mismatched namespace
- **WHEN** `update` is called with `metadata.namespace` that doesn't match the expected namespace
- **THEN** the service SHALL reject with an error

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the service SHALL increment `resource_version`, preserve `created_at`, update `updated_at`, bump `generation` if spec changed, and publish a `Modified` event

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the transaction callback SHALL return `TransactionOp::Abort(AppError::Conflict)` and no event is published

### Requirement: delete handles finalizer lifecycle with namespace
The `delete(key, namespace, name)` method SHALL handle the finalizer-based deletion lifecycle for regular (non-Schema) objects:
1. Fetch the existing object from the store using `(key, namespace, name)`
2. If `finalizers` is empty: hard-delete via `store.transaction()` with `TransactionOp::Delete`, publish `Deleted` event
3. If `finalizers` is non-empty and `deletion_timestamp` is None: mark for deletion via `store.transaction()`, set `deletion_timestamp`, publish `Modified` event
4. If `deletion_timestamp` is already set: return the object without changes, no event published (idempotent)

#### Scenario: Delete regular object without finalizers
- **WHEN** deleting a non-Schema object with empty finalizers
- **THEN** the object SHALL be hard-deleted, a `Deleted` event SHALL be published

#### Scenario: Delete regular object with finalizers marks for deletion
- **WHEN** deleting a non-Schema object with non-empty finalizers
- **THEN** the object SHALL remain in storage with `system.deletionTimestamp` set

### Requirement: Service provides subscribe() with WatchFilter for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe(key, filter)`.

#### Scenario: Subscribe with WatchFilter::All returns a WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::All)` is called
- **THEN** a `WatchStream` is returned that delivers all events for the given resource key
