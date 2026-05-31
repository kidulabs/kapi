## MODIFIED Requirements

### Requirement: delete delegates cache eviction to SchemaRegistry
The `delete(key, name)` method SHALL:
1. If `key.kind == "Schema"`: fetch the Schema, extract target kind, check if objects of that kind exist using `store.exists()`; if so, return `SchemaHasObjects`. Then `store.delete()`, then `schema_registry.evict()`, then `event_bus.publish()`
2. If `key.kind != "Schema"`: `store.delete()`, then `event_bus.publish()`

#### Scenario: Delete Schema with no objects
- **WHEN** deleting a Schema and no objects of the target kind exist
- **THEN** the Schema is deleted, the cache entry is removed via `schema_registry.evict()`, a `Deleted` event is published, and the deleted object is returned

#### Scenario: Delete Schema with existing objects
- **WHEN** deleting a Schema and objects of the target kind exist
- **THEN** the error is `SchemaHasObjects { kind }` and nothing is deleted, evicted, or published

#### Scenario: Delete regular object
- **WHEN** deleting a non-Schema object
- **THEN** the object is deleted, a `Deleted` event is published, and the deleted object is returned
