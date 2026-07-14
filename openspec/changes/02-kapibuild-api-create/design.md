## Context

After initializing a project, users need to define API resources. This command generates skeleton Rust structs with the correct derives and attributes, providing a starting point for users to add their fields.

## Goals / Non-Goals

**Goals:**
- Generate skeleton Rust structs (Spec, optionally Status) for a new API resource
- Create api/<group>/<version>/<kind>.rs file
- Add correct #[derive(KapiResource)] and #[kapi(...)] attributes
- Update Kapifile to track the new resource
- Support --group, --version, --kind, --scope, --status flags

**Non-Goals:**
- Generating final wrapper struct (done by api generate)
- Generating JSON schemas (done by api generate)
- Controller scaffolding (separate command)

## Decisions

### 1. File naming

**Decision:** Use <kind>.rs (lowercase) instead of types.rs

**Rationale:** Each kind gets its own file. Multiple kinds in same group/version don't conflict. Clearer than generic "types.rs".

### 2. Skeleton struct content

**Decision:** Generate minimal structs with example fields.

**Rationale:** Provides starting point. Users replace with their actual fields. Keeps generated code minimal.

### 3. Module structure

**Decision:** Each kind file is standalone. User manages module declarations manually or via future automation.

**Rationale:** Keeps this command simple. Module management can be added later if needed.

## Risks / Trade-offs

**[Risk] User must manage module declarations** → Mitigation: Document clearly. Can add automation later.

**[Trade-off] Manual vs automatic module management** → Manual is simpler for now. Can add auto-management as future enhancement.
