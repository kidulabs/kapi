## Why

Users need to scaffold controller implementations that use the typed client. After generating schemas and registering them with the server, users write controllers to reconcile resources. This command generates controller boilerplate with finalizer pattern, status updates, and typed client usage.

## What Changes

- **`kapibuild controller generate` command** — creates controller scaffolding
  - Creates src/controllers/<kind>_controller.rs
  - Implements Reconciler trait with finalizer pattern
  - Uses typed client for CRUD operations
  - Includes status update logic
  - Wires controller to Manager in src/main.rs

## Capabilities

### New Capabilities
- `05-kapibuild-controller-generate`: Scaffold controller implementations using typed client

### Modified Capabilities
(none)

## Impact

- **New command**: `kapibuild controller generate`
- **Dependencies**: Requires typed client, requires `kapibuild api generate` to have been run
- **Workflow**: Users run `kapibuild controller generate`, then implement reconciliation logic

## Non-goals

- Implementing reconciliation logic (user's responsibility)
- Secondary watch scaffolding
- Predicate/filter system

## Future Work

- Support for multiple controllers per resource
- Secondary watch scaffolding
- Predicate/filter scaffolding
