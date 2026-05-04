## ADDED Requirements

### Requirement: Storage abstraction
The system SHALL define async storage traits (`SchemaStore` and `ObjectStore`) that abstract persistence from handlers and services.

#### Scenario: Traits are Send and Sync
- **WHEN** a storage engine implements `SchemaStore` and `ObjectStore`
- **THEN** the traits are `Send + Sync` so they can be shared across async tasks

### Requirement: SchemaStore trait
The system SHALL define a `SchemaStore` trait with operations to create, read, list, and delete JSON Schema definitions.

#### Scenario: SchemaStore operations
- **WHEN** the system interacts with schema storage
- **THEN** the following operations are available:
  - `register(schema: Schema) -> Result<Schema>`
  - `get(group, version, kind) -> Result<Schema>`
  - `list() -> Result<Vec<Schema>>`
  - `delete(group, version, kind) -> Result<Schema>`

### Requirement: ObjectStore trait
The system SHALL define an `ObjectStore` trait with operations to create, read, list, update, and delete objects, supporting optimistic concurrency.

#### Scenario: ObjectStore operations
- **WHEN** the system interacts with object storage
- **THEN** the following operations are available:
  - `create(key, name, data) -> Result<StoredObject>`
  - `get(key, name) -> Result<StoredObject>`
  - `list(key, opts) -> Result<ListResponse>`
  - `update(key, name, data, expected_version) -> Result<StoredObject>`
  - `delete(key, name, expected_version) -> Result<StoredObject>`

### Requirement: In-memory storage engine
The system SHALL provide an in-memory implementation of both `SchemaStore` and `ObjectStore` using thread-safe data structures.

#### Scenario: Concurrent access
- **WHEN** multiple requests access the in-memory store concurrently
- **THEN** the store handles concurrent reads and writes safely without data races

#### Scenario: Persistence not guaranteed
- **WHEN** the server restarts
- **THEN** all data in the in-memory store is lost

### Requirement: Pluggable storage design
The system SHALL allow the storage engine to be swapped without modifying handlers, services, or routes.

#### Scenario: Swap storage engine
- **WHEN** a new storage engine implements `SchemaStore` and `ObjectStore`
- **THEN** it can be wired into the application with zero changes to route handlers or service logic
