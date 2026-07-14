## Context

Users have schemas generated and registered. They need controller scaffolding that uses the typed client. This command generates controller boilerplate with finalizer pattern, status updates, and typed client usage.

## Goals / Non-Goals

**Goals:**
- Generate src/controllers/<kind>_controller.rs
- Implement Reconciler trait with finalizer pattern
- Use typed client for CRUD operations
- Include status update logic
- Wire controller to Manager in src/main.rs

**Non-Goals:**
- Implementing reconciliation logic (user's responsibility)
- Secondary watch scaffolding
- Predicate/filter system

## Decisions

### 1. Controller template

**Decision:** Include finalizer pattern, status update, and typed client usage as starting boilerplate.

**Rationale:** Provides complete working example. Users can remove what they don't need. Demonstrates best practices.

### 2. Typed client integration

**Decision:** Controller uses TypedClient<Widget> for all CRUD operations.

**Rationale:** Type-safe. No manual serialization/deserialization. Matches the typed client design.

### 3. Module wiring

**Decision:** Update src/controllers/mod.rs and src/main.rs to wire the new controller.

**Rationale:** Automates the tedious wiring. Users can focus on reconciliation logic.

## Risks / Trade-offs

**[Risk] Template may not fit all use cases** → Mitigation:** Keep template simple. Users customize as needed.

**[Trade-off] Boilerplate vs minimal** → More boilerplate provides better starting point. Users can delete what they don't need.
