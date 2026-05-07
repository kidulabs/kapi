## Why

The current roadmap defines two separate storage traits (`SchemaStore` and `ObjectStore`) with parallel module trees (`schema/` and `object/`). The architectural decision has been made to unify these into a single `ObjectStore` where Schema is treated as an object kind (following the Kubernetes CRD model). The roadmap must be updated to reflect this decision so that future implementation follows the correct architecture.

Additionally, the roadmap does not reflect the current implementation state. Only P0 (scaffold) and P1 (core types) are complete (~20% of 61 tasks). The backlog tasks need to be revised to match the unified architecture.

## What Changes

- **Update Architecture section** — replace split traits diagram with single ObjectStore model
- **Update Key Types section** — clarify Schema is now a StoredObject convention, not a separate struct
- **Update Storage Traits section** — show only ObjectStore trait (no SchemaStore)
- **Update Design Decisions table** — add unified store, meta-schema, and block-deletion decisions
- **Update API Surface table** — change `/schemas` paths to `/apis/kapi.io/v1/Schema`
- **Update Request Flow diagram** — show Schema objects through same pipeline as other objects
- **Update Module Tree section** — show collapsed `schema/` directory
- **Revise Backlog tasks (T13–T61)** — rewrite to reflect unified architecture, remove schema-specific tasks, add meta-schema tasks
- **Correct task completion status** — mark T1–T12 as done, update T5

## Capabilities

### New Capabilities
<!-- This change only updates roadmap.md, no new system capabilities -->

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- `roadmap.md` — architecture sections, API surface, module tree, design decisions, and all backlog tasks (T13–T61) are revised
