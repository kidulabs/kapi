## ADDED Requirements

### Requirement: kapibuild controller generate command
The system SHALL provide a `kapibuild controller generate` command that creates controller scaffolding using the typed client with the following flags:
- `--group <group>` (required)
- `--version <version>` (required)
- `--kind <kind>` (required)

The command SHALL:
- Create `src/controllers/<kind>_controller.rs` with Reconciler skeleton
- Implement finalizer pattern (ensure_finalizer, is_deleting, remove_finalizer)
- Use typed client for CRUD operations
- Include status update logic
- Update `src/main.rs` to wire controller to manager
- Update `src/controllers/mod.rs` to export new controller module

#### Scenario: Create controller for existing API
- **WHEN** user runs `kapibuild controller generate --group example.io --version v1 --kind Widget`
- **THEN** system creates src/controllers/widget_controller.rs and updates src/main.rs

#### Scenario: Create controller for non-existent API
- **WHEN** user runs `kapibuild controller generate` for a kind that doesn't exist in src/api/
- **THEN** system returns an error suggesting to run `kapibuild api create` first

### Requirement: Controller skeleton structure
The system SHALL generate controller skeleton with:
- Reconciler trait implementation
- Finalizer pattern (ensure_finalizer, is_deleting, remove_finalizer)
- Typed client usage for fetching and updating objects
- Status update logic
- Placeholder for reconciliation logic

#### Scenario: Controller skeleton content
- **WHEN** system generates a controller skeleton
- **THEN** controller has Reconciler impl with finalizer pattern, typed client usage, and status update logic

### Requirement: Typed client integration
The system SHALL generate controller code that uses TypedClient<Widget> for CRUD operations and raw client for finalizer management.

#### Scenario: Fetch object with typed client
- **WHEN** controller needs to fetch a Widget
- **THEN** controller uses typed_client.get() and receives a typed Widget struct

#### Scenario: Update status with typed client
- **WHEN** controller needs to update Widget status
- **THEN** controller uses typed client status update method with typed WidgetStatus struct

#### Scenario: Finalizer management with raw client
- **WHEN** controller needs to manage finalizers
- **THEN** controller uses raw KapiClient for finalizer operations since finalizer helpers operate on raw StoredObject

### Requirement: Module wiring
The system SHALL update src/controllers/mod.rs to export the new controller module and update src/main.rs to wire the controller to the manager.

#### Scenario: Module export
- **WHEN** system generates a controller
- **THEN** src/controllers/mod.rs includes `pub mod <kind>_controller;`

#### Scenario: Manager wiring
- **WHEN** system generates a controller
- **THEN** src/main.rs includes controller wiring with manager.controller_for(Widget::key()).reconcile_with(WidgetReconciler).register()
