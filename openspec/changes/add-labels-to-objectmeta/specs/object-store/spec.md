## MODIFIED Requirements

### Requirement: SQLiteStore schema initialization
`SQLiteStore::init_schema()` SHALL create both the `objects` table and a `labels` table using `CREATE TABLE IF NOT EXISTS`. The `labels` table SHALL have a composite primary key and a foreign key to `objects` with `ON DELETE CASCADE`. An index on the `labels` table SHALL be created for efficient lookups.

#### Scenario: Fresh database initialization
- **WHEN** `SQLiteStore::new()` is called on a fresh database
- **THEN** both `objects` and `labels` tables SHALL be created, along with their indexes

#### Scenario: Existing database restart
- **WHEN** `SQLiteStore::new()` is called on an existing database that has `objects` but not `labels`
- **THEN** the `labels` table and its index SHALL be created without affecting existing `objects` data

#### Scenario: Idempotent initialization
- **WHEN** `SQLiteStore::new()` is called multiple times on the same database
- **THEN** no errors SHALL occur and no duplicate tables or indexes SHALL be created

### Requirement: SQLiteStore create with labels
`SQLiteStore::create()` SHALL insert label rows into the `labels` table for each key-value pair in `ObjectMeta.labels`, within the same operation as the object insert.

#### Scenario: Create object with labels
- **WHEN** `create()` is called with an object that has labels `{"app": "nginx", "env": "prod"}`
- **THEN** the object row SHALL be inserted into `objects` and two rows SHALL be inserted into `labels`

#### Scenario: Create object without labels
- **WHEN** `create()` is called with an object that has empty labels
- **THEN** the object row SHALL be inserted into `objects` and no rows SHALL be inserted into `labels`

### Requirement: SQLiteStore update with labels
`SQLiteStore::update()` SHALL compute a diff between existing and new labels, then apply targeted deletes and upserts within a single transaction alongside the object update.

#### Scenario: Update with label changes
- **WHEN** `update()` is called with changed labels
- **THEN** the object update and label diff operations SHALL be applied atomically in a single transaction

#### Scenario: Update with no label changes
- **WHEN** `update()` is called with the same labels as the existing object
- **THEN** no label table writes SHALL occur, only the object update

### Requirement: SQLiteStore get/list with labels
`SQLiteStore::get()` and `SQLiteStore::list()` SHALL reconstruct `ObjectMeta.labels` by querying the `labels` table for each object.

#### Scenario: Get object with labels
- **WHEN** `get()` is called for an object that has labels in the `labels` table
- **THEN** the returned `StoredObject` SHALL have those labels in `metadata.labels`

#### Scenario: Get object without labels
- **WHEN** `get()` is called for an object with no rows in the `labels` table
- **THEN** the returned `StoredObject` SHALL have an empty `HashMap` in `metadata.labels`

#### Scenario: List objects with mixed labels
- **WHEN** `list()` is called and some objects have labels while others do not
- **THEN** each returned `StoredObject` SHALL have its correct labels (or empty map)

### Requirement: SQLiteStore delete with labels
`SQLiteStore::delete()` SHALL rely on `ON DELETE CASCADE` to automatically remove label rows when an object is deleted.

#### Scenario: Delete object with labels
- **WHEN** `delete()` is called for an object that has labels
- **THEN** the object row SHALL be deleted from `objects` and all associated label rows SHALL be automatically deleted via the foreign key cascade

### Requirement: InMemoryStore create with labels
`InMemoryStore::create()` SHALL store labels as part of `ObjectMeta` within the `StoredObject`.

#### Scenario: Create object with labels
- **WHEN** `create()` is called with an object that has labels
- **THEN** the stored `StoredObject` SHALL contain those labels in `metadata.labels`

### Requirement: InMemoryStore update with labels
`InMemoryStore::update()` SHALL replace the entire `ObjectMeta` (including labels) with the updated version.

#### Scenario: Update object labels
- **WHEN** `update()` is called with new labels
- **THEN** the stored `StoredObject.metadata.labels` SHALL be replaced with the new labels
