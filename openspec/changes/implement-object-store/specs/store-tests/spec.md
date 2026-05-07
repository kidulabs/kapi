## ADDED Requirements

### Requirement: Unit tests cover create and get operations
The test suite SHALL include tests that verify: creating an object and immediately retrieving it returns identical data; creating a duplicate returns `Conflict`; getting a non-existent object returns `NotFound`.

#### Scenario: Create then get returns matching object
- **WHEN** an object is created and then retrieved by the same key and name
- **THEN** the returned object's data, key, and name match the created values

#### Scenario: Create duplicate returns conflict
- **WHEN** two create calls use the same key and name
- **THEN** the second call returns `AppError::Conflict`

#### Scenario: Get missing returns NotFound
- **WHEN** get is called for a key/name that was never created
- **THEN** the result is `AppError::NotFound`

### Requirement: Unit tests cover list operations including pagination
The test suite SHALL include tests that verify: listing all objects returns them sorted by name; listing with a limit returns the correct number of items with a continue token; listing with a continue token resumes from the correct position; listing an empty key returns an empty list.

#### Scenario: List returns sorted results
- **WHEN** objects are created with names "c", "a", "b" and listed
- **THEN** the returned order is "a", "b", "c"

#### Scenario: List with limit produces continue token
- **WHEN** 5 objects exist and list is called with `limit = Some(2)`
- **THEN** 2 items are returned and `continue_token` is `Some`

#### Scenario: List with continue token skips correctly
- **WHEN** list is called with a continue token from a previous limited list
- **THEN** the results start after the last item of the previous batch

### Requirement: Unit tests cover update with optimistic concurrency
The test suite SHALL include tests that verify: updating with the correct version succeeds and increments the version; updating with a wrong version returns `Conflict`; updating a non-existent object returns `NotFound`.

#### Scenario: Update with correct version succeeds
- **WHEN** an object is created and then updated with its current version
- **THEN** the update succeeds and the new version is greater than the old version

#### Scenario: Update with stale version returns conflict
- **WHEN** an object is updated twice, the second update using the original version
- **THEN** the second update returns `AppError::Conflict`

### Requirement: Unit tests cover delete with optional version check
The test suite SHALL include tests that verify: deleting an existing object returns it; deleting with a matching version succeeds; deleting with a mismatched version returns `Conflict` and leaves the object intact; deleting with `None` version succeeds unconditionally; deleting a non-existent object returns `NotFound`.

#### Scenario: Delete returns the removed object
- **WHEN** an object is created and then deleted
- **THEN** the delete returns the object and a subsequent get returns `NotFound`

#### Scenario: Delete with wrong version leaves object intact
- **WHEN** delete is called with a version that does not match
- **THEN** the error is `AppError::Conflict` and the object still exists with its original data
