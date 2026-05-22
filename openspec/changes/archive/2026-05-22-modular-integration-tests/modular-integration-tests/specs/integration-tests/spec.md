## MODIFIED Requirements

### Requirement: Cargo verification passes

The system SHALL maintain clean build and test status.

#### Scenario: All tests pass
- **WHEN** integration test binary runs
- **THEN** all test scenarios execute against each available store implementation
- **AND** the suite terminates immediately on first failure
- **AND** all tests pass with no warnings

#### Scenario: Clippy passes
- **WHEN** `cargo clippy -- -D warnings` runs
- **THEN** no warnings or errors

#### Scenario: Documentation builds
- **WHEN** `cargo doc --no-deps` runs
- **THEN** documentation generates without errors

## ADDED Requirements

### Requirement: Multi-store test execution

The integration test suite SHALL run all test scenarios against each registered store implementation.

#### Scenario: Tests run against InMemoryStore
- **WHEN** integration tests execute
- **THEN** all scenarios run with an InMemoryStore-backed TestApp

#### Scenario: Tests run against SQLiteStore
- **WHEN** integration tests execute
- **THEN** all scenarios run with a SQLiteStore-backed TestApp using a temporary database file
- **AND** the temporary file is deleted when the suite exits

#### Scenario: Test output groups by store
- **WHEN** integration tests execute against multiple stores
- **THEN** output is grouped by store name with a header (e.g., `=== memory ===`)
- **AND** each test within a group shows pass/fail status

### Requirement: Modular TestApp construction

TestApp SHALL support construction with an arbitrary `Arc<dyn ObjectStore>` and SHALL NOT provide a default no-argument constructor.

#### Scenario: TestApp created with explicit store
- **WHEN** test code calls `TestApp::with_store(store)`
- **THEN** the returned TestApp uses the provided store for all operations

#### Scenario: TestApp::new() does not exist
- **WHEN** code attempts to call `TestApp::new()`
- **THEN** compilation fails (method does not exist)

### Requirement: Store factory registration

The test harness SHALL provide a registry of available store implementations via a factory pattern.

#### Scenario: all_test_stores returns available stores
- **WHEN** `all_test_stores()` is called
- **THEN** it returns a Vec containing at least InMemoryStore and SQLiteStore factories
- **AND** each factory produces a fresh `Arc<dyn ObjectStore>` when invoked

#### Scenario: Future stores can be added
- **WHEN** a new store implementation is available
- **THEN** it can be added to `all_test_stores()` by appending a single `TestStore` entry
