## MODIFIED Requirements

### Requirement: ValidationError location
`ValidationError` SHALL be defined in `src/object/types.rs` (not `src/schema/types.rs`).

#### Scenario: ValidationError accessible from object module
- **WHEN** `error.rs` imports `ValidationError`
- **THEN** it imports from `crate::object::types::ValidationError`

### Requirement: Schema module scope
`src/schema/mod.rs` SHALL only declare `pub mod meta_schema`.

#### Scenario: Schema module contains only meta_schema
- **WHEN** the schema module is compiled
- **THEN** it contains only `meta_schema.rs`
- **AND** `schema/types.rs`, `schema/service.rs`, `schema/handler.rs` do not exist

### Requirement: No separate Schema struct
Schema objects SHALL be represented as `StoredObject` with `kind="Schema"` in group `"kapi.io"`, not as a separate `Schema` struct.

#### Scenario: Schema struct removed
- **WHEN** the codebase is compiled
- **THEN** `src/schema/types.rs` does not exist
- **AND** no `Schema` struct is defined outside of `StoredObject`
