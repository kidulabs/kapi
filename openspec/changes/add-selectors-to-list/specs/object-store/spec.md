## MODIFIED Requirements

### Requirement: ObjectStore list signature
`ObjectStore::list()` SHALL accept `ListOptions` with optional `field_selector` and `label_selector` fields.

#### Scenario: List with no filters
- **WHEN** `list()` is called with `ListOptions { field_selector: None, label_selector: None, ... }`
- **THEN** the store SHALL return all objects for the given key (existing behavior)

#### Scenario: List with filters
- **WHEN** `list()` is called with `ListOptions { field_selector: Some(...), label_selector: Some(...), ... }`
- **THEN** the store SHALL return only objects matching both filters

### Requirement: InMemoryStore list filtering
`InMemoryStore::list()` SHALL apply filters in Rust after collecting objects but before sorting and pagination.

#### Scenario: Filter applied before pagination
- **WHEN** `list()` is called with a filter and `limit=10`
- **THEN** the filter SHALL be applied first, then the result truncated to 10 items

#### Scenario: Filter with continue token
- **WHEN** `list()` is called with a filter and a continue token
- **THEN** the filter SHALL be applied, then the cursor skip, then truncation

### Requirement: SQLiteStore list filtering
`SQLiteStore::list()` SHALL apply filters as SQL WHERE clauses before pagination.

#### Scenario: SQL query with field filter
- **WHEN** `list()` is called with `field_selector: Some(NameEquals("foo"))`
- **THEN** the SQL query SHALL include `AND name = ?` with the value bound

#### Scenario: SQL query with label filter
- **WHEN** `list()` is called with `label_selector: Some(...)` with requirements
- **THEN** the SQL query SHALL include `EXISTS`/`NOT EXISTS` subqueries for each requirement

#### Scenario: SQL query with both filters
- **WHEN** `list()` is called with both field and label selectors
- **THEN** the SQL query SHALL include both the field condition and label subqueries

#### Scenario: SQL query with no filters
- **WHEN** `list()` is called with no filters
- **THEN** the SQL query SHALL be the same as before (no additional WHERE clauses)
