## MODIFIED Requirements

### Requirement: SchemaService delete checks dependents per (group, version, kind), removes, evicts, and publishes
The `SchemaService::delete(key, name)` method SHALL:
1. Fetch the Schema from the store
2. Extract the target `(group, version, kind)` from the Schema's data
3. Check if objects exist for that exact `(group, version, kind)` using `store.exists()`. The check SHALL use the full GVK, not just the kind, so deletion of one version does not block on objects of other versions of the same kind
4. If objects exist at that GVK, return `AppError::SchemaHasObjects { kind }`
5. Call `store.transaction()` with `TransactionOp::Delete` to remove the Schema
6. Call `schema_registry.evict(name)` to remove the cached spec and status validators
7. Call `event_bus.publish()` with a `Deleted` event

#### Scenario: Delete Schema with no objects at the target GVK
- **WHEN** deleting a Schema and no objects exist at the target `(group, version, kind)`
- **THEN** the Schema SHALL be deleted, the cache entry SHALL be removed via `schema_registry.evict()`, a `Deleted` event SHALL be published, and the deleted object SHALL be returned

#### Scenario: Delete Schema with objects of the same kind but different version
- **WHEN** deleting the v1 Schema for `example.io/v1/Widget` while objects exist for `example.io/v2/Widget`
- **THEN** the v1 Schema SHALL be deleted, its cache entries SHALL be evicted, and a `Deleted` event SHALL be published
- **AND** the v2 objects SHALL remain untouched

#### Scenario: Delete Schema with existing objects at the same GVK
- **WHEN** deleting a Schema and objects exist at the target `(group, version, kind)`
- **THEN** the error SHALL be `SchemaHasObjects { kind }` and nothing SHALL be deleted, evicted, or published

### Requirement: SchemaService schema cache uses versioned schema name as key
The SchemaRegistry cache SHALL be keyed by the Schema's versioned name field (e.g., `"Widget.example.io.v1"`). SchemaService SHALL pass the versioned schema name to `schema_registry.insert()` and `schema_registry.evict()`. Two Schemas with the same `targetKind` and `targetGroup` but different `targetVersion` SHALL occupy independent cache entries.

#### Scenario: Cache insertion on Schema create uses versioned name
- **WHEN** a Schema is created with `targetKind: "Widget"`, `targetGroup: "example.io"`, `targetVersion: "v1"`
- **THEN** `schema_registry.insert("Widget.example.io.v1", compiled)` SHALL be called after successful store persistence

#### Scenario: Cache eviction on Schema delete uses versioned name
- **WHEN** a Schema with name `"Widget.example.io.v1"` is deleted
- **THEN** `schema_registry.evict("Widget.example.io.v1")` SHALL be called after successful store deletion
- **AND** cache entries for other versions (e.g., `"Widget.example.io.v2"`) SHALL be untouched

## ADDED Requirements

### Requirement: Per-version registration and validation coexistence
The SchemaService SHALL allow two Schemas with the same `targetKind` and `targetGroup` but different `targetVersion` to be registered simultaneously. Each registration SHALL produce an independent stored object (with a unique `metadata.name` derived from the versioned format `"{kind}.{group}.{version}"`), an independent cache entry, and SHALL NOT cause an `AlreadyExists` error for the other version.

#### Scenario: Register two versions of the same kind
- **WHEN** a Schema is registered for `example.io/v1/Widget` and then another for `example.io/v2/Widget`
- **THEN** both registrations succeed with `metadata.name` of `"Widget.example.io.v1"` and `"Widget.example.io.v2"` respectively
- **AND** both are listed in `GET /apis/kapi.io/v1/Schema`

#### Scenario: Registering a version that already exists returns AlreadyExists
- **WHEN** a Schema is registered for `example.io/v1/Widget` and then another Schema is registered with identical `targetGroup`, `targetKind`, and `targetVersion`
- **THEN** the second registration SHALL fail with `AlreadyExists`
- **AND** the first Schema SHALL remain unchanged
