## 1. kapi-derive Proc-Macro

- [ ] 1.1 Implement KapiAttrs struct with darling to parse #[kapi(...)] attributes
- [ ] 1.2 Implement #[derive(KapiResource)] proc-macro entry point
- [ ] 1.3 Generate wrapper struct with metadata, spec, and optional status fields
- [ ] 1.4 Generate key() method returning ResourceKey
- [ ] 1.5 Implement schema_data() method using schemars
- [ ] 1.6 Add validation for required attributes (group, version, kind)
- [ ] 1.7 Add validation for scope attribute (must be "Namespaced" or "Cluster")
- [ ] 1.8 Write unit tests for proc-macro

## 2. Schema Generation via Helper Binary

- [ ] 2.1 Implement helper binary generation (generates a small Rust program)
- [ ] 2.2 Helper binary imports user types from api/ directory
- [ ] 2.3 Helper binary calls schema_data() on each wrapper struct
- [ ] 2.4 Helper binary writes JSON schema files to schemas/ directory
- [ ] 2.5 Implement compilation of helper binary
- [ ] 2.6 Implement execution of helper binary
- [ ] 2.7 Implement cleanup of helper binary after execution

## 3. API Generate Command

- [ ] 3.1 Implement api/ directory scanning for types.rs files
- [ ] 3.2 Parse Rust structs with #[derive(KapiResource)]
- [ ] 3.3 Extract group/version/kind/scope from #[kapi(...)] attributes
- [ ] 3.4 Construct full SchemaData payload
- [ ] 3.5 Write schema files to schemas/<group>_<kind>.json
- [ ] 3.6 Test schema generation with various type definitions

## 4. Verification

- [ ] 4.1 Verify generated schema files can be applied with kapi-cli apply -f schemas/
- [ ] 4.2 Run cargo test -p kapi-derive to verify all tests pass
- [ ] 4.3 Run cargo clippy -p kapi-derive to check for linting issues
- [ ] 4.4 Run cargo clippy -p kapibuild to check for linting issues

## 5. Documentation

- [ ] 5.1 Create docs/kapibuild/kapi-resource-macro.md documenting the KapiResource derive macro
- [ ] 5.2 Create docs/kapibuild/validation-rules.md documenting schemars validation attributes
- [ ] 5.3 Create docs/kapibuild/serde-attributes.md documenting serde attributes and their effect on schema generation
