## Context

Users have skeleton structs from `api create`. They need to generate the final wrapper struct and JSON schema files. This command bridges user-defined types and kapi server requirements.

## Goals / Non-Goals

**Goals:**
- Parse skeleton structs from api/<group>/<version>/<kind>.rs
- Use kapi-derive proc-macro to generate wrapper struct at compile time
- Generate JSON schema files in schemas/<group>_<kind>.json
- Schema includes specSchema and statusSchema (if status exists)
- No controller code generated

**Non-Goals:**
- Controller scaffolding (separate command)
- Runtime schema registration
- Schema validation against server

## Decisions

### 1. Schema generation approach

**Decision:** Use a helper binary approach — generate a small Rust program that imports user types, calls schema_data(), and writes JSON files.

**Alternatives considered:**
- Parse source with syn (abandoned code approach — produced placeholder schemas, not real ones)
- build.rs integration (too magical, hard to debug)

**Rationale:** Helper binary actually compiles and runs the proc-macro, producing real schemas with schemars. User can inspect and debug the helper. Works with validation rules.

### 2. Generated schema format

**Decision:** Full SchemaData payload (targetGroup, targetVersion, targetKind, scope, specSchema, statusSchema).

**Rationale:** Can be applied directly with `kapi-cli apply -f schema.json`. No extra wrapping needed.

### 3. Wrapper struct visibility

**Decision:** Proc-macro generates wrapper struct at compile time. Not a separate file on disk.

**Rationale:** Matches kube-rs approach. Single source of truth (skeleton struct). Macro handles the rest.

## Risks / Trade-offs

**[Risk] Helper binary adds compilation step** → Mitigation: Document clearly. Can add build.rs integration later.

**[Trade-off] Explicit regeneration vs automatic** → Manual `api generate` gives control. Users must remember to run it after editing types.
