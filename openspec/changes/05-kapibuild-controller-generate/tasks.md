## 1. CLI Structure

- [ ] 1.1 Add `controller generate` subcommand to CLI with required flags (group, version, kind)

## 2. Controller Scaffolding

- [ ] 2.1 Implement src/controllers/<kind>_controller.rs file creation
- [ ] 2.2 Generate Reconciler trait implementation
- [ ] 2.3 Generate finalizer pattern (ensure_finalizer, is_deleting, remove_finalizer)
- [ ] 2.4 Generate typed client usage for fetching objects
- [ ] 2.5 Generate status update logic using typed client
- [ ] 2.6 Generate placeholder for reconciliation logic

## 3. Module Wiring

- [ ] 3.1 Update src/controllers/mod.rs to export new controller module
- [ ] 3.2 Update src/main.rs to wire controller to manager
- [ ] 3.3 Add controller wiring: manager.controller_for(Widget::key()).reconcile_with(WidgetReconciler).register()

## 4. Validation

- [ ] 4.1 Validate that API exists before creating controller
- [ ] 4.2 Return error suggesting `kapibuild api create` if API doesn't exist

## 5. Testing

- [ ] 5.1 Test `kapibuild controller generate` for existing API
- [ ] 5.2 Test `kapibuild controller generate` for non-existent API returns error
- [ ] 5.3 Verify generated controller compiles
- [ ] 5.4 Run cargo clippy -p kapibuild to check for linting issues

## 6. Documentation

- [ ] 6.1 Create docs/kapibuild/controller-patterns.md documenting common controller patterns
- [ ] 6.2 Create docs/kapibuild/workflow.md documenting the complete development workflow
- [ ] 6.3 Create docs/kapibuild/troubleshooting.md documenting common issues and solutions
- [ ] 6.4 Update README.md to mention kapibuild and link to documentation
