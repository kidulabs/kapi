## MODIFIED Requirements

### Requirement: create validates scope, namespace existence, spec, and sets metadata
The `create(key, namespace, meta, spec)` method SHALL:
1. Look up the Schema scope for the kind
2. Validate namespace vs scope:
   - If scope is "Cluster" and namespace is Some, reject with error
   - If scope is "Namespaced" and namespace is None, set namespace to "default"
3. **Validate namespace existence**: if scope is "Namespaced", check that the namespace exists by looking up the Namespace object. If not found, return 404 Not Found.
4. Validate `meta.labels` using label validation rules
5. Validate `meta.annotations` using annotation validation rules
6. Call `schema_registry.get_validator(&key)` to obtain the validator
7. Validate `spec` against the compiled schema validator
8. Construct a `StoredObject` with `metadata.namespace = namespace`, `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`
9. Call `store.create()` to persist
10. Call `event_bus.publish()` with an `Added` event

#### Scenario: Create object in existing namespace
- **WHEN** creating an object in namespace "production" and the namespace exists
- **THEN** the object SHALL be created successfully

#### Scenario: Create object in non-existent namespace
- **WHEN** creating an object in namespace "nonexistent" and the namespace does not exist
- **THEN** the service SHALL return 404 Not Found

#### Scenario: Create object in "default" namespace
- **WHEN** creating an object without explicit namespace (defaults to "default")
- **THEN** the object SHALL be created successfully (since "default" always exists)

#### Scenario: Create cluster-scoped object skips namespace check
- **WHEN** creating a cluster-scoped object
- **THEN** namespace existence SHALL NOT be checked (cluster-scoped objects have no namespace)
