## ADDED Requirements

### Requirement: ObjectStore provides existence checking
The ObjectStore trait SHALL provide an `exists` method that checks whether any objects exist for a given resource key.

#### Scenario: Check existence when objects exist
- **WHEN** `exists(key)` is called for a resource key that has one or more objects
- **THEN** the method SHALL return `Ok(true)`

#### Scenario: Check existence when no objects exist
- **WHEN** `exists(key)` is called for a resource key that has no objects
- **THEN** the method SHALL return `Ok(false)`

#### Scenario: Existence check is efficient
- **WHEN** `exists(key)` is called on a backend with many objects
- **THEN** the implementation SHALL NOT fetch all objects into memory (e.g., use SQL EXISTS, not SELECT *)

### Requirement: InMemoryStore implements exists
The InMemoryStore SHALL implement the `exists` method by checking if any entries match the given resource key.

#### Scenario: InMemoryStore existence check with matching objects
- **WHEN** `exists(key)` is called and the in-memory store contains objects with that key
- **THEN** the method SHALL return `Ok(true)`

#### Scenario: InMemoryStore existence check with no matching objects
- **WHEN** `exists(key)` is called and the in-memory store contains no objects with that key
- **THEN** the method SHALL return `Ok(false)`

### Requirement: SQLiteStore implements exists
The SQLiteStore SHALL implement the `exists` method using an efficient SQL EXISTS query.

#### Scenario: SQLiteStore existence check with matching objects
- **WHEN** `exists(key)` is called and the database contains objects with that key
- **THEN** the method SHALL return `Ok(true)` using a SQL query with EXISTS clause

#### Scenario: SQLiteStore existence check with no matching objects
- **WHEN** `exists(key)` is called and the database contains no objects with that key
- **THEN** the method SHALL return `Ok(false)` using a SQL query with EXISTS clause
