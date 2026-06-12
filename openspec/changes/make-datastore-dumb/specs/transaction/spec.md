## MODIFIED Requirements

### Requirement: TransactionOp enum variants

The `TransactionOp` enum SHALL define four variants: `Apply(StoredObject)`, `Delete`, `Abort(AppError)`, and `NoOp(T)`.

#### Scenario: Apply persists object as-is
- **WHEN** the callback returns `TransactionOp::Apply(obj)`
- **THEN** the store SHALL persist the provided object exactly as-is, without modifying any system metadata fields (resource_version, generation, updated_at)
- **AND** the store SHALL return `Ok(obj)`

#### Scenario: Delete removes the object
- **WHEN** the callback returns `TransactionOp::Delete`
- **THEN** the store SHALL hard-delete the object and return the deleted object

#### Scenario: Abort returns error without changes
- **WHEN** the callback returns `TransactionOp::Abort(err)`
- **THEN** the store SHALL NOT modify the object and SHALL return `Err(err)`

#### Scenario: NoOp returns value without changes
- **WHEN** the callback returns `TransactionOp::NoOp(val)`
- **THEN** the store SHALL NOT modify the object and SHALL return `Ok(val)`

### Requirement: Automatic resource version bumping

The store SHALL NOT automatically bump `resource_version` or update `updated_at` when `TransactionOp::Apply` is returned. The caller (service layer) is responsible for setting all system metadata before returning `TransactionOp::Apply`.

#### Scenario: No automatic version bump on Apply
- **WHEN** `TransactionOp::Apply(obj)` is returned with `obj.system.resource_version = 5`
- **THEN** the stored object's `resource_version` SHALL be exactly 5 (not incremented by the store)
- **AND** the stored object's `updated_at` SHALL be exactly what was in `obj` (not modified by the store)

#### Scenario: No version bump on other ops
- **WHEN** `TransactionOp::Delete`, `Abort`, or `NoOp` is returned
- **THEN** `resource_version` and `updated_at` SHALL NOT be modified

## REMOVED Requirements

None. All existing requirements are either modified above or remain unchanged.
