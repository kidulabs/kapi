## 1. Core Types & Schema Scope

- [x] 1.1 Add `scope: String` field to `SchemaData` in `src/object/types.rs` with `#[serde(default = "default_scope")]` defaulting to `"Namespaced"`. Add `fn default_scope() -> String { "Namespaced".to_string() }`.
- [x] 1.2 Add `namespace: Option<String>` field to `ObjectMeta` in `src/object/types.rs` with `#[serde(skip_serializing_if = "Option::is_none")]`.
- [x] 1.3 Update `ContinueToken` to encode/decode `(namespace: Option<String>, name: String)` instead of just `name`. Update `encode_continue_token` and `decode_continue_token` functions.
- [x] 1.4 Update all test helpers (`test_stored_object`, etc.) to include `namespace: None` in `ObjectMeta`.
- [x] 1.5 Run `cargo check` and fix all compilation errors from type changes.

## 2. Store Trait & InMemoryStore

- [x] 2.1 Update `ObjectStore` trait in `src/store/mod.rs`: change `get` signature to `get(&self, key: &ResourceKey, namespace: Option<&str>, name: &str)`, `list` to `list(&self, key: &ResourceKey, namespace: Option<&str>, opts: ListOptions)`, `transaction` to `transaction(&self, key: &ResourceKey, namespace: Option<&str>, name: &str, op: ...)`.
- [x] 2.2 Update `InMemoryStore` in `src/store/memory.rs`: change backing store to `DashMap<(ResourceKey, Option<String>, String), StoredObject>`. Update `create`, `get`, `list`, `transaction` implementations.
- [x] 2.3 Update `InMemoryStore::list()`: when `namespace` is `None`, collect all objects for the key; when `Some`, filter by namespace. Sort by `(namespace, name)` for cross-namespace, by `name` for namespace-scoped. Update continue token to use `(namespace, name)`.
- [x] 2.4 Update `InMemoryStore` unit tests for new signatures and namespace behavior.
- [x] 2.5 Run `cargo check` and fix all compilation errors.

## 3. SQLiteStore

- [x] 3.1 Update SQLite schema in `src/store/sqlite.rs`: add `namespace TEXT` column to `objects` table. Update primary key to `(resource_group, api_version, resource_kind, namespace, name)`. Update `labels` table foreign key accordingly.
- [x] 3.2 Update `SQLiteStore::create()`: insert `namespace` column from `object.metadata.namespace`.
- [x] 3.3 Update `SQLiteStore::get()`: add `namespace` parameter to query.
- [x] 3.4 Update `SQLiteStore::list()`: add `namespace` parameter. When `None`, no namespace filter. When `Some`, add `AND namespace = ?`. Order by `namespace, name` for cross-namespace, by `name` for namespace-scoped. Update continue token encoding.
- [x] 3.5 Update `SQLiteStore::transaction()`: add `namespace` parameter to the lookup query.
- [x] 3.6 Update `SQLiteStore` row-to-object mapping to read `namespace` column.
- [x] 3.7 Update SQLiteStore unit tests for new signatures and namespace behavior.
- [x] 3.8 Run `cargo check` and fix all compilation errors.

## 4. SchemaRegistry Scope Support

- [x] 4.1 Add `scope: String` field to the cached entry in `SchemaRegistry` (`src/schema/registry.rs`). Create a `CachedSchema` struct or add scope to existing cache value.
- [x] 4.2 Update `validate_and_compile()` to extract and return scope from `SchemaData`.
- [x] 4.3 Update `get_validator()` to return `(Arc<dyn SchemaValidator>, String)` — validator and scope.
- [x] 4.4 Add `get_scope(&self, key: &ResourceKey) -> Result<String, AppError>` method to `SchemaRegistry`.
- [x] 4.5 Update `insert()` to accept and cache scope alongside validator.
- [x] 4.6 Update `SchemaRegistry` unit tests for scope handling.
- [x] 4.7 Run `cargo check` and fix all compilation errors.

## 5. ObjectService Scope Validation

- [x] 5.1 Update `ObjectService::create()` in `src/object/service.rs`: add `namespace: Option<String>` parameter. Look up scope from schema registry. Validate namespace vs scope (cluster-scoped rejects namespace, namespaced defaults to "default"). Set `metadata.namespace` from resolved namespace (discard input meta.namespace).
- [x] 5.2 Update `ObjectService::get()`: add `namespace: Option<&str>` parameter, pass through to store.
- [x] 5.3 Update `ObjectService::list()`: add `namespace: Option<&str>` parameter, pass through to store.
- [x] 5.4 Update `ObjectService::update()`: validate `object.metadata.namespace` matches expected namespace. Add namespace parameter or extract from object.
- [x] 5.5 Update `ObjectService::delete()`: add `namespace: Option<&str>` parameter, pass through to store.
- [x] 5.6 Update `ObjectService` unit tests for namespace handling and scope validation.
- [x] 5.7 Run `cargo check` and fix all compilation errors.

## 6. SchemaService Migration

- [x] 6.1 Update `SchemaService` in `src/object/schema_service.rs`: Schema is cluster-scoped. Set `scope: "Cluster"` when creating Schema objects. Update `create`, `update`, `delete` to pass `namespace: None` to store.
- [x] 6.2 Update Schema constants: ensure Schema objects are stored with `namespace: None`.
- [x] 6.3 Update `SchemaService` unit tests.
- [x] 6.4 Run `cargo check` and fix all compilation errors.

## 7. Handler & Route Changes

- [x] 7.1 Add new path structs in `src/object/handler.rs`: `NamespaceObjectPath` with `{group, version, namespace, kind}` and `NamespaceObjectNamePath` with `{group, version, namespace, kind, name}`.
- [x] 7.2 Update `routes.rs` in `src/routes.rs`: add namespace-scoped routes `/apis/{group}/{version}/namespaces/{namespace}/{kind}` and `/apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}` alongside existing routes.
- [x] 7.3 Update create handler: extract optional namespace from path, pass to service. Discard `metadata.namespace` from body.
- [x] 7.4 Update get handler: extract optional namespace from path, pass to service.
- [x] 7.5 Update list handler: extract optional namespace from path, pass to service. Support cross-namespace list (namespace=None for namespaced kinds).
- [x] 7.6 Update update handler: extract optional namespace from path. Validate `metadata.namespace` matches URL namespace or set from URL if absent.
- [x] 7.7 Update delete handler: extract optional namespace from path, pass to service.
- [x] 7.8 Update status handlers (get_status, update_status): add namespace-scoped routes.
- [x] 7.9 Update handler unit tests.
- [x] 7.10 Run `cargo check` and fix all compilation errors.

## 8. Event Bus & Watch

- [x] 8.1 Verify `EventBus` works with namespace-scoped objects. Events carry `StoredObject` which now includes namespace. No structural changes needed to EventBus itself.
- [x] 8.2 Update watch handler to pass namespace context when subscribing (if needed for future namespace-scoped watch).
- [x] 8.3 Run `cargo check`.

## 9. OpenAPI Spec Generation & Swagger UI

- [x] 9.1 Update `src/openapi/` to reflect new URL patterns with namespace path parameter.
- [x] 9.2 Update OpenAPI tests.
- [x] 9.3 Review Swagger UI templates in `src/openapi/` to ensure they correctly display namespace-scoped and cluster-scoped endpoints.
- [x] 9.4 Add example requests/responses in OpenAPI spec showing namespace-scoped operations (e.g., `POST /apis/example.io/v1/namespaces/production/widgets`).
- [x] 9.5 Add example requests/responses showing cluster-scoped operations (e.g., `GET /apis/kapi.io/v1/schemas`).
- [x] 9.6 Add example showing cross-namespace list operations (e.g., `GET /apis/example.io/v1/widgets` returning objects from multiple namespaces).
- [x] 9.7 Verify Swagger UI renders correctly by starting the server and manually checking `/swagger-ui` endpoint displays all endpoint variations with proper examples.
- [x] 9.8 Run `cargo check`.

## 10. Integration Tests

- [x] 10.1 Update integration tests in `tests/` crate for new URL patterns (namespace-scoped and cluster-scoped).
- [x] 10.2 Add integration tests for cross-namespace list.
- [x] 10.3 Add integration tests for same name in different namespaces.
- [x] 10.4 Add integration tests for scope validation (cluster-scoped with namespace rejected, etc.).
- [x] 10.5 Add integration tests for continue token with namespace.
- [x] 10.6 Run integration tests against both InMemory and SQLite stores.

## 11. E2E Tests (kapi-e2e-tests skill)

- [x] 11.1 Update existing e2e tests in `docs/testprompt.md` for new URL patterns.
- [x] 11.2 Add e2e test scenarios for namespace-scoped CRUD operations.
- [x] 11.3 Add e2e test scenarios for cross-namespace list.
- [x] 11.4 Add e2e test scenarios for cluster-scoped resources (Schema).
- [x] 11.5 Add e2e test scenarios for scope validation errors.

## 12. Documentation & Roadmap

- [x] 12.1 Update `AGENTS.md` with namespace-aware architecture description.
- [x] 12.2 Check `docs/` directory for any documentation that needs updating (API docs, architecture docs).
- [x] 12.3 Check `openspec/specs/roadmap-update/` and roadmap items — update or remove items impacted by namespace changes.
- [x] 12.4 Update any README or API documentation files.

## 13. Verification

- [x] 13.1 Run `cargo clippy --all-targets --all-features -- -D warnings` and fix all warnings.
- [x] 13.2 Run `cargo test` (unit tests) and ensure all pass.
- [x] 13.3 Run integration tests: `cargo test --package kapi-tests` and ensure all pass.
- [x] 13.4 Run kapi-e2e-tests skill tests and ensure all pass.
- [x] 13.5 Run `cargo fmt --check` and fix formatting if needed.
- [x] 13.6 Manual verification: start server, test namespace-scoped and cluster-scoped CRUD via curl/HTTP client.
- [x] 13.7 DO NOT auto-commit. User wants to review and verify changes before committing.
