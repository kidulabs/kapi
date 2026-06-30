## ADDED Requirements

### Requirement: Storage key includes namespace
The storage key SHALL be `(ResourceKey, namespace: Option<String>, name)`. Cluster-scoped objects SHALL have `namespace = None`. Namespaced objects SHALL have `namespace = Some(namespace_string)`. Name uniqueness SHALL be scoped to `(namespace, kind)` — two objects can have the same name if they are in different namespaces or if one is cluster-scoped.

#### Scenario: Same name in different namespaces
- **WHEN** two objects are created with the same name but different namespaces
- **THEN** both SHALL be stored successfully

#### Scenario: Same name in same namespace rejected
- **WHEN** two objects are created with the same name and same namespace
- **THEN** the second create SHALL fail with `AlreadyExists`

#### Scenario: Cluster-scoped and namespaced with same name
- **WHEN** a cluster-scoped object and a namespaced object have the same name
- **THEN** both SHALL be stored successfully (different namespace values)

### Requirement: ObjectMeta includes namespace field
The `ObjectMeta` struct SHALL include a `namespace` field of type `Option<String>`. The field SHALL be serialized as `namespace` in JSON with `#[serde(skip_serializing_if = "Option::is_none")]`. Cluster-scoped objects SHALL have `namespace = None`. Namespaced objects SHALL have `namespace = Some(namespace_string)`.

#### Scenario: ObjectMeta serialization with namespace
- **WHEN** an `ObjectMeta` with `namespace: Some("production")` is serialized
- **THEN** the JSON SHALL contain `"namespace": "production"`

#### Scenario: ObjectMeta serialization without namespace
- **WHEN** an `ObjectMeta` with `namespace: None` is serialized
- **THEN** the JSON SHALL NOT contain a `namespace` key

#### Scenario: ObjectMeta deserialization with namespace
- **WHEN** JSON with `"namespace": "production"` is deserialized
- **THEN** the resulting `ObjectMeta` SHALL have `namespace = Some("production")`

#### Scenario: ObjectMeta deserialization without namespace
- **WHEN** JSON without `namespace` field is deserialized
- **THEN** the resulting `ObjectMeta` SHALL have `namespace = None`

### Requirement: Store trait accepts namespace parameter
The `ObjectStore` trait methods `get`, `list`, and `transaction` SHALL accept a `namespace: Option<&str>` parameter. The `create` method SHALL use the namespace from the `StoredObject.metadata.namespace` field.

#### Scenario: get with namespace
- **WHEN** `get(key, Some("production"), "foo")` is called
- **THEN** the object with `namespace = Some("production")` and `name = "foo"` SHALL be returned

#### Scenario: get without namespace (cluster-scoped)
- **WHEN** `get(key, None, "foo")` is called
- **THEN** the cluster-scoped object with `name = "foo"` SHALL be returned

#### Scenario: list with namespace
- **WHEN** `list(key, Some("production"), opts)` is called
- **THEN** only objects in the `"production"` namespace SHALL be returned

#### Scenario: list without namespace (all namespaces)
- **WHEN** `list(key, None, opts)` is called
- **THEN** objects from all namespaces SHALL be returned

#### Scenario: transaction with namespace
- **WHEN** `transaction(key, Some("production"), "foo", op)` is called
- **THEN** the transaction SHALL operate on the object in the `"production"` namespace

### Requirement: InMemoryStore key includes namespace
The `InMemoryStore` SHALL use `DashMap<(ResourceKey, Option<String>, String), StoredObject>` as its backing store. The key SHALL include the namespace as the second element.

#### Scenario: InMemoryStore stores objects with namespace
- **WHEN** an object with `namespace = Some("production")` is created
- **THEN** it SHALL be stored under key `(ResourceKey, Some("production"), name)`

#### Scenario: InMemoryStore stores cluster-scoped objects
- **WHEN** an object with `namespace = None` is created
- **THEN** it SHALL be stored under key `(ResourceKey, None, name)`

### Requirement: SQLiteStore schema includes namespace column
The SQLite `objects` table SHALL include a `namespace` column of type `TEXT`. The column SHALL be nullable (cluster-scoped objects have `namespace = NULL`). The primary key SHALL be `(resource_group, api_version, resource_kind, namespace, name)`.

#### Scenario: SQLite schema with namespace column
- **WHEN** the SQLite schema is initialized
- **THEN** the `objects` table SHALL have a `namespace TEXT` column
- **AND** the primary key SHALL include `namespace`

#### Scenario: SQLite stores cluster-scoped objects with NULL namespace
- **WHEN** a cluster-scoped object is created
- **THEN** the `namespace` column SHALL be `NULL`

#### Scenario: SQLite stores namespaced objects with namespace value
- **WHEN** a namespaced object is created in "production"
- **THEN** the `namespace` column SHALL be `"production"`

### Requirement: Continue token encodes namespace and name
The `ContinueToken` SHALL encode both `namespace` and `name` to support cross-namespace pagination. The token format SHALL be a base64-encoded JSON object with `namespace` and `name` fields.

#### Scenario: Continue token with namespace
- **WHEN** a continue token is generated for an object in "production" namespace with name "foo"
- **THEN** the token SHALL encode `{"namespace": "production", "name": "foo"}`

#### Scenario: Continue token for cluster-scoped object
- **WHEN** a continue token is generated for a cluster-scoped object with name "foo"
- **THEN** the token SHALL encode `{"namespace": null, "name": "foo"}`

#### Scenario: Continue token resumes from correct position
- **WHEN** a list is resumed with a continue token
- **THEN** objects SHALL be skipped up to and including the encoded `(namespace, name)`
