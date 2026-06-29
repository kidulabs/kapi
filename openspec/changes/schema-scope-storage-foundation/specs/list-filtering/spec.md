## MODIFIED Requirements

### Requirement: ListOptions filter parameters
`ListOptions` SHALL include optional `field_selector: Option<FieldSelector>` and `label_selector: Option<LabelSelector>` fields for store-level filtering. Filtering applies within the namespace scope determined by the `namespace` parameter passed to `store.list()`.

#### Scenario: ListOptions with no filters
- **WHEN** `ListOptions` is created with `field_selector: None` and `label_selector: None`
- **THEN** the list operation SHALL return all objects within the namespace scope

#### Scenario: ListOptions with field selector
- **WHEN** `ListOptions` is created with `field_selector: Some(NameEquals("foo"))`
- **THEN** the list operation SHALL return only objects matching the field selector within the namespace scope

#### Scenario: ListOptions with label selector
- **WHEN** `ListOptions` is created with `label_selector: Some(...)` with requirements
- **THEN** the list operation SHALL return only objects matching the label selector within the namespace scope

### Requirement: Filtering before pagination
Store implementations SHALL apply filters before pagination to ensure correct page sizes and cursor semantics. Pagination SHALL use `(namespace, name)` ordering for cross-namespace lists and `name` ordering for namespace-scoped lists.

#### Scenario: Filter reduces result set
- **WHEN** a list request with `labelSelector=app=nginx` and `limit=10` is made
- **THEN** the response SHALL contain at most 10 items matching the filter

#### Scenario: Cross-namespace pagination uses (namespace, name) ordering
- **WHEN** a cross-namespace list with `limit=2` is made
- **THEN** results SHALL be ordered by (namespace, name) and continue token encodes both

### Requirement: InMemoryStore filtering
`InMemoryStore::list()` SHALL apply field and label filters in Rust after collecting objects but before sorting and pagination. When `namespace` is `None`, all objects for the key are collected. When `namespace` is `Some`, only objects matching that namespace are collected.

#### Scenario: InMemoryStore filters by field within namespace
- **WHEN** `list(key, Some("production"), opts)` is called with `field_selector: Some(NameEquals("foo"))`
- **THEN** only objects in "production" with `metadata.name == "foo"` SHALL be included

#### Scenario: InMemoryStore cross-namespace list
- **WHEN** `list(key, None, opts)` is called
- **THEN** objects from all namespaces are collected, filtered, sorted by (namespace, name), and paginated

### Requirement: SQLiteStore filtering
`SQLiteStore::list()` SHALL apply field and label filters as SQL WHERE clauses before pagination. When `namespace` is `None`, no namespace filter is applied. When `namespace` is `Some`, the SQL query SHALL include `AND namespace = ?`.

#### Scenario: SQLiteStore filters by field within namespace
- **WHEN** `list(key, Some("production"), opts)` is called with `field_selector: Some(NameEquals("foo"))`
- **THEN** the SQL query SHALL include `AND namespace = 'production' AND name = 'foo'`

#### Scenario: SQLiteStore cross-namespace list
- **WHEN** `list(key, None, opts)` is called
- **THEN** the SQL query SHALL NOT include a namespace filter
- **AND** results SHALL be ordered by `namespace, name`

### Requirement: fieldSelector on list requests
The handler SHALL accept `fieldSelector` on non-watch list requests and pass it to `ListOptions`.

#### Scenario: List with fieldSelector
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo`
- **THEN** the handler SHALL parse the selector and pass it to `ListOptions`

#### Scenario: List with both selectors
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo&labelSelector=app=nginx`
- **THEN** the handler SHALL parse both selectors and pass them to `ListOptions`
