## Why

The ObjectStore trait lacks an efficient existence check, forcing the schema deletion guard to fetch all objects just to count them. Additionally, the magic string `"Schema"` appears 25 times across 6 files, making refactoring difficult and error-prone.

## What Changes

- Add `exists(&self, key: &ResourceKey) -> Result<bool, AppError>` method to `ObjectStore` trait
- Implement `exists()` for `InMemoryStore` (O(n) scan) and `SQLiteStore` (efficient SQL EXISTS query)
- Refactor schema deletion guard to use `exists()` instead of `list(limit: None)`
- Define constants `SCHEMA_KIND`, `SCHEMA_GROUP`, `SCHEMA_VERSION` in `src/schema/mod.rs`
- Add helper function `schema_key() -> ResourceKey` for consistent schema key construction
- Replace all 25 occurrences of magic strings with constants across 6 files

## Capabilities

### New Capabilities

- `store-exists`: Efficient existence checking for ObjectStore trait implementations

### Modified Capabilities

- `object-store`: Add exists() method to trait definition
- `object-service`: Use exists() in schema deletion guard instead of list()
- `schema-registry`: Use schema constants instead of magic strings

## Impact

**Code affected:**
- `src/store/mod.rs` - trait definition
- `src/store/memory.rs` - InMemoryStore implementation
- `src/store/sqlite.rs` - SQLiteStore implementation  
- `src/object/service.rs` - deletion guard logic + 16 magic string occurrences
- `src/schema/registry.rs` - 3 magic string occurrences
- `src/schema/mod.rs` - new constants definition
- `src/openapi/paths.rs` - 1 magic string occurrence
- `src/openapi/mod.rs` - 2 magic string occurrences (tests)
- `src/event/bus.rs` - 1 magic string occurrence (test)
- `src/object/handler.rs` - 2 magic string occurrences

**APIs:** No public API changes. Internal trait method addition.

**Performance:** Schema deletion guard improves from O(n) full list fetch to O(1) SQL EXISTS query (SQLite) or O(n) scan (InMemory, same as before but without allocation).

## Non-goals

- Extracting shared store+publish helpers (repetition is honest and explicit)
- Adding filtered existence checks (YAGNI - only need simple key-based check)
- Changing error messages to include object counts (simpler "objects exist" is sufficient)

## Future Work

- Consider adding `count()` method if we need exact counts for other operations
- If a second special kind appears (e.g., Policy), consider KindHooks trait for per-kind behavior
