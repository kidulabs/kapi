## 1. Namespace Constants & Schema Registration

- [x] 1.1 Add Namespace constants in `src/schema/mod.rs` or `src/object/types.rs`: `NAMESPACE_KIND = "Namespace"`, `NAMESPACE_GROUP = "kapi.io"`, `NAMESPACE_VERSION = "v1"`.
- [x] 1.2 Create a function to generate the built-in Namespace schema JSON: `{ "type": "object", "properties": {} }` with `scope: "Cluster"`.
- [x] 1.3 Add a bootstrap function that registers the Namespace schema at startup (via SchemaService or direct store insertion).
- [x] 1.4 Run `cargo check` and fix compilation errors.

## 2. "default" Namespace Bootstrap

- [x] 2.1 Add a bootstrap function in `src/lib.rs` or a new `src/bootstrap.rs` that creates the "default" Namespace object if it doesn't exist.
- [x] 2.2 Call the bootstrap function during server startup, after store initialization but before the server starts accepting requests.
- [x] 2.3 Ensure bootstrap failure causes server startup to fail with a clear error message.
- [x] 2.4 Add unit tests for bootstrap logic.
- [x] 2.5 Run `cargo check` and fix compilation errors.

## 3. "default" Namespace Protection

- [x] 3.1 Add a check in the delete path (ObjectService or handler) to reject DELETE for "default" namespace with 403 Forbidden.
- [x] 3.2 Add an error variant `AppError::DefaultNamespaceUndeletable` or similar.
- [x] 3.3 Add unit tests for "default" namespace deletion rejection.
- [x] 3.4 Run `cargo check` and fix compilation errors.

## 4. Namespace Existence Validation

- [x] 4.1 Update `ObjectService::create()` in `src/object/service.rs`: after resolving namespace, check if the Namespace object exists by calling `store.get(namespace_key, None, namespace_name)`.
- [x] 4.2 If namespace doesn't exist, return `AppError::NotFound { what: "namespace", identifier: namespace_name }`.
- [x] 4.3 Skip namespace existence check for cluster-scoped kinds (they have no namespace).
- [x] 4.4 Add unit tests for namespace existence validation.
- [x] 4.5 Run `cargo check` and fix compilation errors.

## 5. Namespace Deletion Blocking

- [x] 5.1 Update the delete path for Namespace objects: before deleting, check if any objects exist in that namespace by calling `store.list(namespace_key, Some(namespace), ListOptions { limit: Some(1), ... })`.
- [x] 5.2 If objects exist, return `AppError::Conflict` or a new `AppError::NamespaceNotEmpty { namespace, object_count }`.
- [x] 5.3 Add unit tests for namespace deletion blocking.
- [x] 5.4 Run `cargo check` and fix compilation errors.

## 6. WatchFilter::Namespace

- [x] 6.1 Add `Namespace(String)` variant to `WatchFilter` enum in `src/object/types.rs`.
- [x] 6.2 Update `WatchFilter::matches()` to handle `Namespace` variant: check `event.object.metadata.namespace == Some(namespace)`.
- [x] 6.3 Update all `match` statements on `WatchFilter` to handle the new variant.
- [x] 6.4 Add unit tests for `WatchFilter::Namespace` matching logic.
- [x] 6.5 Run `cargo check` and fix compilation errors.

## 7. Namespace-Scoped Watch

- [x] 7.1 Update the watch handler in `src/object/handler.rs`: when watching a namespaced kind at `/apis/{g}/{v}/namespaces/{ns}/{kind}?watch=true`, create `WatchFilter::Namespace(ns)`.
- [x] 7.2 Combine `WatchFilter::Namespace` with field/label selectors using `WatchFilter::And` when both are present.
- [x] 7.3 For cross-namespace watch (no namespace in URL), use `WatchFilter::All`.
- [x] 7.4 Update handler unit tests for namespace-scoped watch.
- [x] 7.5 Run `cargo check` and fix compilation errors.

## 8. Integration Tests

- [x] 8.1 Add integration tests for Namespace CRUD operations (create, get, list, update, delete).
- [x] 8.2 Add integration tests for "default" namespace bootstrap (verify it exists after startup).
- [x] 8.3 Add integration tests for "default" namespace deletion rejection (403).
- [x] 8.4 Add integration tests for namespace existence validation on object creation (404 for non-existent namespace).
- [x] 8.5 Add integration tests for namespace deletion blocking (409 for non-empty namespace).
- [x] 8.6 Add integration tests for namespace-scoped watch (WatchFilter::Namespace).
- [x] 8.7 Run integration tests against both InMemory and SQLite stores.

## 9. E2E Tests (kapi-e2e-tests skill)

- [x] 9.1 Add e2e test scenarios for Namespace CRUD operations.
- [x] 9.2 Add e2e test scenarios for "default" namespace protection.
- [x] 9.3 Add e2e test scenarios for namespace existence validation.
- [x] 9.4 Add e2e test scenarios for namespace deletion blocking.
- [x] 9.5 Add e2e test scenarios for namespace-scoped watch.

## 10. Documentation & Roadmap

- [x] 10.1 Update `AGENTS.md` with Namespace resource description and lifecycle rules.
- [x] 10.2 Check `docs/` directory for any documentation that needs updating (API docs, architecture docs).
- [x] 10.3 Check roadmap items — update or remove items impacted by namespace resource changes.
- [x] 10.4 Update any README or API documentation files.

## 11. Verification

- [x] 11.1 Run `cargo clippy --all-targets --all-features -- -D warnings` and fix all warnings.
- [x] 11.2 Run `cargo test` (unit tests) and ensure all pass.
- [x] 11.3 Run integration tests: `cargo test --package kapi-tests` and ensure all pass.
- [x] 11.4 Run kapi-e2e-tests skill tests and ensure all pass.
- [x] 11.5 Run `cargo fmt --check` and fix formatting if needed.
- [x] 11.6 Manual verification: start server, verify "default" namespace exists, test Namespace CRUD via curl/HTTP client.
- [x] 11.7 DO NOT auto-commit. User wants to review and verify changes before committing.
