## 1. kapi-derive Proc-Macro

- [x] 1.1 Implement KapiAttrs struct with darling to parse #[kapi(...)] attributes
- [x] 1.2 Implement #[derive(KapiResource)] proc-macro entry point
- [x] 1.3 Generate wrapper struct with metadata, spec, and optional status fields
- [x] 1.4 Generate key() method returning ResourceKey
- [x] 1.5 Implement schema_data() method using schemars
- [x] 1.6 Add validation for required attributes (group, version, kind)
- [x] 1.7 Add validation for scope attribute (must be "Namespaced" or "Cluster")
- [x] 1.8 Write unit tests for proc-macro

## 2. Schema Generation via Helper Binary

- [x] 2.1 Implement helper binary generation (generates a small Rust program)
- [x] 2.2 Helper binary imports user types from api/ directory
- [x] 2.3 Helper binary calls schema_data() on each wrapper struct
- [x] 2.4 Helper binary writes JSON schema files to schemas/ directory
- [x] 2.5 Implement compilation of helper binary
- [x] 2.6 Implement execution of helper binary
- [x] 2.7 Implement cleanup of helper binary after execution

## 3. API Generate Command

- [x] 3.1 Implement api/ directory scanning for types.rs files
- [x] 3.2 Parse Rust structs with #[derive(KapiResource)]
- [x] 3.3 Extract group/version/kind/scope from #[kapi(...)] attributes
- [x] 3.4 Construct full SchemaData payload
- [x] 3.5 Write schema files to schemas/<group>_<kind>.json
- [x] 3.6 Test schema generation with various type definitions

## 4. Verification

- [x] 4.1 Verify generated schema files can be applied with kapi-cli apply -f schemas/
- [x] 4.2 Run cargo test -p kapi-derive to verify all tests pass
- [x] 4.3 Run cargo clippy -p kapi-derive to check for linting issues
- [x] 4.4 Run cargo clippy -p kapibuild to check for linting issues

## 5. Documentation

- [x] 5.1 Create docs/kapibuild/kapi-resource-macro.md documenting the KapiResource derive macro
- [x] 5.2 Create docs/kapibuild/validation-rules.md documenting schemars validation attributes
- [x] 5.3 Create docs/kapibuild/serde-attributes.md documenting serde attributes and their effect on schema generation
