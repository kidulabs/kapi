## Purpose

Define the `ListOptions` filter parameters (`field_selector`, `label_selector`) and the filtering behavior for store implementations when listing objects. Filtering SHALL happen before pagination to ensure correct page sizes and cursor semantics.

## Requirements

### Requirement: ListOptions filter parameters
`ListOptions` SHALL include optional `field_selector: Option<FieldSelector>` and `label_selector: Option<LabelSelector>` fields for store-level filtering.

#### Scenario: ListOptions with no filters
- **WHEN** `ListOptions` is created with `field_selector: None` and `label_selector: None`
- **THEN** the list operation SHALL return all objects (no filtering)

#### Scenario: ListOptions with field selector
- **WHEN** `ListOptions` is created with `field_selector: Some(NameEquals("foo"))`
- **THEN** the list operation SHALL return only objects matching the field selector

#### Scenario: ListOptions with label selector
- **WHEN** `ListOptions` is created with `label_selector: Some(...)` with requirements
- **THEN** the list operation SHALL return only objects matching the label selector

#### Scenario: ListOptions with both selectors
- **WHEN** `ListOptions` is created with both `field_selector` and `label_selector`
- **THEN** the list operation SHALL return only objects matching both selectors (AND semantics)

### Requirement: Filtering before pagination
Store implementations SHALL apply filters before pagination to ensure correct page sizes and cursor semantics.

#### Scenario: Filter reduces result set
- **WHEN** a list request with `labelSelector=app=nginx` and `limit=10` is made, and 50 objects exist but only 3 have `app=nginx`
- **THEN** the response SHALL contain 3 items (not 10)

#### Scenario: Filter with pagination
- **WHEN** a list request with `labelSelector=app=nginx`, `limit=2`, and a continue token is made
- **THEN** the response SHALL contain the next 2 objects matching the filter, past the cursor

### Requirement: InMemoryStore filtering
`InMemoryStore::list()` SHALL apply field and label filters in Rust after collecting objects but before sorting and pagination.

#### Scenario: InMemoryStore filters by field
- **WHEN** `list()` is called with `field_selector: Some(NameEquals("foo"))`
- **THEN** only objects with `metadata.name == "foo"` SHALL be included in results

#### Scenario: InMemoryStore filters by label
- **WHEN** `list()` is called with `label_selector: Some(...)` with `Equals{key:"app", value:"nginx"}`
- **THEN** only objects with label `app=nginx` SHALL be included in results

#### Scenario: InMemoryStore filters by both
- **WHEN** `list()` is called with both field and label selectors
- **THEN** only objects matching both filters SHALL be included

### Requirement: SQLiteStore filtering
`SQLiteStore::list()` SHALL apply field and label filters as SQL WHERE clauses before pagination.

#### Scenario: SQLiteStore filters by field
- **WHEN** `list()` is called with `field_selector: Some(NameEquals("foo"))`
- **THEN** the SQL query SHALL include `AND name = 'foo'` in the WHERE clause

#### Scenario: SQLiteStore filters by label equality
- **WHEN** `list()` is called with `label_selector` containing `Equals{key:"app", value:"nginx"}`
- **THEN** the SQL query SHALL include an `EXISTS` subquery checking for label `app=nginx`

#### Scenario: SQLiteStore filters by label inequality
- **WHEN** `list()` is called with `label_selector` containing `NotEquals{key:"env", value:"prod"}`
- **THEN** the SQL query SHALL include a condition that matches objects without the `env` label OR with `env` label not equal to `prod`

#### Scenario: SQLiteStore filters by label existence
- **WHEN** `list()` is called with `label_selector` containing `Exists{key:"gpu"}`
- **THEN** the SQL query SHALL include an `EXISTS` subquery checking for presence of label key `gpu`

#### Scenario: SQLiteStore filters by label non-existence
- **WHEN** `list()` is called with `label_selector` containing `NotExists{key:"experimental"}`
- **THEN** the SQL query SHALL include a `NOT EXISTS` subquery checking for absence of label key `experimental`

#### Scenario: SQLiteStore filters by multiple label requirements
- **WHEN** `list()` is called with `label_selector` containing multiple requirements
- **THEN** the SQL query SHALL include multiple `EXISTS`/`NOT EXISTS` clauses (AND semantics)

### Requirement: fieldSelector on list requests
The handler SHALL accept `fieldSelector` on non-watch list requests and pass it to `ListOptions`. The previous 400 error for `fieldSelector` on list SHALL be removed.

#### Scenario: List with fieldSelector
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo` (no watch)
- **THEN** the handler SHALL parse the selector and pass it to `ListOptions`

#### Scenario: List with both selectors
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo&labelSelector=app=nginx` (no watch)
- **THEN** the handler SHALL parse both selectors and pass them to `ListOptions`
