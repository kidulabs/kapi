## Context

The ObjectStore trait currently provides CRUD operations (create, get, list, update, delete) but no efficient way to check if objects exist for a given resource key. The schema deletion guard in ObjectService works around this by calling `list()` with `limit: None`, which fetches all objects just to count them.

Additionally, the string `"Schema"` is hardcoded in 25 places across 6 files, along with `"kapi.io"` and `"v1"` for the schema's group and version. This makes refactoring difficult and creates inconsistency risk.

## Goals / Non-Goals

**Goals:**
- Add efficient existence checking to ObjectStore trait
- Eliminate magic strings for schema kind, group, and version
- Improve schema deletion guard performance

**Non-Goals:**
- Adding filtered existence checks (only simple key-based check needed)
- Extracting shared store+publish helpers (repetition is honest)
- Changing error messages to include object counts

## Decisions

### Decision 1: Add `exists()` method to ObjectStore trait

**Choice:** Add `async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError>` to the trait.

**Alternatives considered:**
- `count()` method returning `usize` — more information but not needed; existence is sufficient for the deletion guard
- `list(limit: Some(1))` pattern — already in use, but still allocates a Vec and doesn't express intent clearly
- Default trait implementation using `list()` — would work but defeats the purpose of efficient backend-specific implementations

**Rationale:** `exists()` is the minimal API that expresses intent. Backends can optimize (SQL `EXISTS`, etcd `--count-only`) without over-fetching. The boolean return is sufficient for the deletion guard use case.

### Decision 2: Place schema constants in `src/schema/mod.rs`

**Choice:** Define `SCHEMA_KIND`, `SCHEMA_GROUP`, `SCHEMA_VERSION` constants and a `schema_key()` helper in `src/schema/mod.rs`.

**Alternatives considered:**
- New `src/constants.rs` file — adds a file for just 4 items; schema constants belong with the schema module
- Place in `src/store/mod.rs` — store module shouldn't know about schema-specific constants
- Place in `src/object/types.rs` — types module is for data structures, not constants

**Rationale:** Schema constants are schema concerns. The `schema_key()` helper co-locates with the constants it uses. Other modules import from `crate::schema::*`.

### Decision 3: Simplify SchemaHasObjects error

**Choice:** Remove the `count` field from `AppError::SchemaHasObjects`, keeping only `kind`.

**Alternatives considered:**
- Keep count field, use `count()` method — adds a method we don't otherwise need
- Keep count field, use `list(limit: None)` — defeats the purpose of adding `exists()`

**Rationale:** The exact count adds marginal value to the error message. "Cannot delete schema: objects of kind Widget exist" is clear enough without "5 objects exist". This keeps the API surface minimal.

## Risks / Trade-offs

- **[Breaking trait change]** Adding `exists()` to ObjectStore requires all implementations to provide it → Mitigation: Only two implementations exist (InMemoryStore, SQLiteStore), both straightforward to update.
- **[Error message loses detail]** Removing count from SchemaHasObjects → Mitigation: The kind name is sufficient for users to understand the issue. If count becomes needed later, `count()` can be added without breaking `exists()`.
- **[Refactoring scope]** 25 magic string replacements across 6 files → Mitigation: Mechanical find-and-replace, low risk. Tests will catch any missed occurrences.
