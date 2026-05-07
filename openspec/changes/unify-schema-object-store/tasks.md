## 1. Update Architecture section

- [ ] 1.1 Replace split traits diagram with single ObjectStore model in AppState
- [ ] 1.2 Update Store layer description to reference single `ObjectStore` trait

## 2. Update Key Types section

- [ ] 2.1 Replace separate `Schema` struct definition with description of Schema as StoredObject convention

## 3. Update Storage Traits section

- [ ] 3.1 Remove `SchemaStore` trait definition
- [ ] 3.2 Keep only `ObjectStore` trait definition

## 4. Update Design Decisions table

- [ ] 4.1 Update "Storage abstraction" row: change from "Split traits" to "Single ObjectStore"
- [ ] 4.2 Add decision row: "Schema validation" → "Builtin meta-schema compiled at startup"
- [ ] 4.3 Add decision row: "Schema deletion" → "Block if objects exist (409 Conflict)"

## 5. Update API Surface section

- [ ] 5.1 Change Schema Registry paths from `/schemas` to `/apis/kapi.io/v1/Schema`
- [ ] 5.2 Update path parameters from `{group}/{version}/{kind}` to `{name}` for schema endpoints

## 6. Update Request Flow diagram

- [ ] 6.1 Show Schema object creation going through same ObjectService pipeline as regular objects

## 7. Update Module Tree section

- [ ] 7.1 Replace `schema/` subtree with only `meta_schema.rs`
- [ ] 7.2 Update `store/mod.rs` description to show only ObjectStore trait

## 8. Revise Backlog tasks (T13–T61)

- [ ] 8.1 Rewrite P2 tasks (T13–T20): single ObjectStore trait + InMemoryStore implementation
- [ ] 8.2 Add new task for meta-schema module creation
- [ ] 8.3 Rewrite P4 tasks (T27–T34): replace schema service/handler tasks with meta-schema validation tasks
- [ ] 8.4 Rewrite P5 tasks (T35–T42): update ObjectService to include validation dispatch for Schema vs regular objects
- [ ] 8.5 Update P7 tasks (T47–T50): simplified AppState with single store and service
- [ ] 8.6 Update P8 tasks (T51–T54): ToSchema derives for unified types
- [ ] 8.7 Update P9 tasks (T55–T61): integration tests for Schema-as-object flows, block-deletion test

## 9. Correct task completion status

- [ ] 9.1 Mark T1–T12 as `[x]` completed
- [ ] 9.2 Update T5 status based on actual cargo build state
