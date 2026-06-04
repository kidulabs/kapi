## ADDED Requirements

### Requirement: generation field in SystemMetadata

`SystemMetadata` SHALL include a `generation: u64` field. This field is server-maintained and represents the number of times the object's spec has been changed. It SHALL be initialized to 1 on CREATE.

#### Scenario: New object has generation 1
- **WHEN** an object is created via `store.create()`
- **THEN** the returned `StoredObject.system.generation` equals 1

### Requirement: update() bumps generation on spec change

The `ObjectStore::update()` method SHALL compare the incoming object's `spec.value` with the stored object's `spec.value`. If they differ (using `serde_json::Value` structural equality), the method SHALL increment `generation` by 1. If they are equal, `generation` SHALL remain unchanged.

#### Scenario: Spec change bumps generation
- **WHEN** `update()` is called with a different `spec.value` than the stored object
- **THEN** the returned `StoredObject.system.generation` is exactly 1 greater than the stored generation

#### Scenario: Same spec does not bump generation
- **WHEN** `update()` is called with the same `spec.value` but different `metadata.labels`
- **THEN** the returned `StoredObject.system.generation` equals the stored generation (unchanged)

### Requirement: update_status() does NOT bump generation

The `ObjectStore::update_status()` method SHALL NOT modify the `generation` field. It SHALL only bump `resource_version` and set `updated_at`.

#### Scenario: Status update does not bump generation
- **WHEN** `update_status()` is called on an object with `generation: N`
- **THEN** the returned `StoredObject.system.generation` equals N (unchanged)

### Requirement: ObjectStore trait documents generation contract

The `ObjectStore` trait definition SHALL include documentation specifying that:
- `create()` initializes `generation` to 1
- `update()` bumps `generation` iff `spec.value` differs from the stored value
- `update_status()` does NOT bump `generation`

#### Scenario: Trait documentation is present
- **WHEN** reading the `ObjectStore` trait definition
- **THEN** the generation behavior is documented in the trait's doc comment

### Requirement: Integration test verifies generation semantics

The integration test suite SHALL include a test that verifies generation behavior across all store implementations. The test SHALL:
1. Create an object and verify `generation == 1`
2. Update with same spec, different labels, verify `generation` unchanged
3. Update with different spec, verify `generation` incremented
4. Update status, verify `generation` unchanged
5. Update with same spec, different labels again, verify `generation` unchanged

#### Scenario: Generation test passes for InMemoryStore
- **WHEN** the integration test runs against InMemoryStore
- **THEN** all generation assertions pass

#### Scenario: Generation test passes for SQLiteStore
- **WHEN** the integration test runs against SQLiteStore
- **THEN** all generation assertions pass
