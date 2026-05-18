## 1. Handler: Schema name generation in create handler

- [ ] 1.1 Add `extract_schema_name(&body)` helper function in `src/object/handler.rs` that reads `targetKind` and `targetGroup` from the JSON body and returns `Some("{targetKind}.{targetGroup}")` or `None` if either field is missing or not a string. Include code comments explaining the purpose and format.
- [ ] 1.2 Modify the `create` handler to branch on `path.kind == "Schema"`: for Schema, call `extract_schema_name` and return `AppError::InvalidSchema` if it returns `None`; for non-Schema, continue extracting name from `metadata.name` as before. Include code comments explaining the branching logic.
- [ ] 1.3 Remove the `metadata` field from the body before passing to `service.create()` for Schema registrations (it's already removed for non-Schema, ensure the same applies).

## 2. Tests: Update service tests for nameless registration

- [ ] 2.1 Update `register_test_schema()` helper in `src/object/service.rs` to use the generated name format `Widget.example.io` consistently, with comments noting the name is now backend-generated.
- [ ] 2.2 Update all test cases that call `service.create()` for Schema objects to use the generated name format instead of arbitrary names. Verify each test still passes.
- [ ] 2.3 Add a new test case: Schema create with missing `targetKind` returns `InvalidSchema` error.
- [ ] 2.4 Add a new test case: Schema create with missing `targetGroup` returns `InvalidSchema` error.
- [ ] 2.5 Run `cargo test` to verify all tests pass.

## 3. Specs: Update delta specs

- [ ] 3.1 Review delta specs in `openspec/changes/schema-nameless-registration/specs/` for completeness and accuracy against the implementation.
- [ ] 3.2 Run `openspec validate schema-nameless-registration` to ensure specs are well-formed.

## 4. Roadmap: Check deviation and update roadmap

- [ ] 4.1 Read the current roadmap file (check `openspec/specs/roadmap-update/` or project root for roadmap document) and compare against the current implementation state.
- [ ] 4.2 Identify any deviations between the roadmap and actual implementation — note features added, removed, or changed in scope.
- [ ] 4.3 Update the roadmap file to reflect the current state, including this change (schema nameless registration).
- [ ] 4.4 If deviations are significant, add a section documenting what changed and why.

## 5. Schema validation: Mark as future item

- [ ] 5.1 Identify where schema validation status is tracked (roadmap, project docs, or spec files).
- [ ] 5.2 Add a note or section marking schema validation as a future/deferred item, with a brief explanation of what it covers and why it's deferred.
- [ ] 5.3 Ensure this does not conflict with any existing roadmap entries.

## 6. Code quality: Comments and documentation

- [ ] 6.1 Ensure all new code (handler changes, helper functions) has clear code comments explaining purpose, inputs, and outputs.
- [ ] 6.2 Review existing handler and service code for areas where comments would improve navigability — add module-level and function-level doc comments where missing.
- [ ] 6.3 Run `cargo fmt` and `cargo clippy` to ensure code style compliance.
