## Context

The project is a Kubernetes-apiserver-inspired API server in Rust. Currently at ~20% completion: P0 (scaffold) and P1 (core types) are done. P2 through P9 (48 of 61 tasks) are not started — all service, handler, store, event, middleware, routing, and test files are stubs (`//! TODO`).

The architectural decision has been made to unify `SchemaStore` and `ObjectStore` into a single `ObjectStore` where Schema is a special object kind. The `roadmap.md` document still describes the old two-trait architecture and has task definitions that don't reflect this decision.

## Goals / Non-Goals

**Goals:**
- Update `roadmap.md` to describe the unified single-ObjectStore architecture
- Revise backlog tasks to reflect the correct module structure and implementation approach
- Correct task completion status to match actual implementation
- Preserve the roadmap's role as the project planning document

**Non-Goals:**
- No code changes in this change
- No implementation of storage, services, handlers, or tests
- No changes to existing type definitions in source files

## Decisions

### D1: Roadmap is the sole artifact changed

This change updates only `roadmap.md`. Code changes will be implemented in subsequent changes, guided by the updated roadmap.

### D2: Preserve roadmap structure, update content

The roadmap's section layout (Architecture, API Surface, Key Types, Storage Traits, Design Decisions, Module Tree, Backlog) is retained. Content within each section is updated to reflect the unified architecture.

### D3: Tasks are rewritten, not deleted

Existing task numbers (T13–T61) are revised in place rather than renumbered. This preserves any external references to task IDs. Tasks that are no longer relevant (e.g., "Define SchemaStore trait") are replaced with their unified equivalents (e.g., "Define single ObjectStore trait").

### D4: New tasks added for meta-schema

Tasks for creating the meta-schema module and meta-schema validation logic are added to the backlog, as these are new requirements introduced by the unified architecture.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| Task numbering changes | Tasks are revised in place; IDs preserved where possible |
| Scope creep during roadmap update | Strictly limit changes to roadmap.md; code changes are separate |
| Loss of detail from old tasks | Old tasks are transformed, not removed — intent is preserved |
