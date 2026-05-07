## 1. Update Architecture section

- [x] 1.1 Replace split traits diagram with single ObjectStore model in AppState
- [x] 1.2 Update Store layer description to reference single `ObjectStore` trait

## 2. Update Key Types section

- [x] 2.1 Replace separate `Schema` struct definition with description of Schema as StoredObject convention

## 3. Update Storage Traits section

- [x] 3.1 Remove `SchemaStore` trait definition
- [x] 3.2 Keep only `ObjectStore` trait definition

## 4. Update Design Decisions table

- [x] 4.1 Update "Storage abstraction" row: change from "Split traits" to "Single ObjectStore"
- [x] 4.2 Add decision row: "Schema validation" → "Builtin meta-schema compiled at startup"
- [x] 4.3 Add decision row: "Schema deletion" → "Block if objects exist (409 Conflict)"

## 5. Update API Surface section

- [x] 5.1 Change Schema Registry paths from `/schemas` to `/apis/kapi.io/v1/Schema`
- [x] 5.2 Update path parameters from `{group}/{version}/{kind}` to `{name}` for schema endpoints

## 6. Update Request Flow diagram

- [x] 6.1 Show Schema object creation going through same ObjectService pipeline as regular objects

## 7. Update Module Tree section

- [x] 7.1 Replace `schema/` subtree with only `meta_schema.rs`
- [x] 7.2 Update `store/mod.rs` description to show only ObjectStore trait

## 8. Revise Backlog tasks (T13–T61)

- [x] 8.1 Rewrite P2 tasks (T13–T20): single ObjectStore trait + InMemoryStore implementation
- [x] 8.2 Add new task for meta-schema module creation
- [x] 8.3 Rewrite P4 tasks (T27–T34): replace schema service/handler tasks with meta-schema validation tasks
- [x] 8.4 Rewrite P5 tasks (T35–T42): update ObjectService to include validation dispatch for Schema vs regular objects
- [x] 8.5 Update P7 tasks (T47–T50): simplified AppState with single store and service
- [x] 8.6 Update P8 tasks (T51–T54): ToSchema derives for unified types
- [x] 8.7 Update P9 tasks (T55–T61): integration tests for Schema-as-object flows, block-deletion test

## 9. Correct task completion status

- [x] 9.1 Mark T1–T12 as `[x]` completed
- [x] 9.2 Update T5 status based on actual cargo build state
