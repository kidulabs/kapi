## Context

Unit tests in `src/` cover individual components (store, service, event bus, OpenAPI generation) in isolation using mocks and direct method calls. Integration tests do not yet exist. The `tests/` directory is empty.

Integration tests exercise the full HTTP request path:
```
TestClient → Router → [middleware] → Handler → ObjectService → Store/EventBus
```

All components must be wired correctly for tests to pass.

## Goals / Non-Goals

**Goals:**
- Test complete HTTP flows: register schema → CRUD objects → watch events
- Verify pagination behavior across multiple pages with continue tokens
- Verify schema deletion guard (409 when objects exist, 200 when clean)
- Verify schema registration validation (422 on invalid jsonSchema)
- Verify optimistic concurrency (409 on wrong resourceVersion, 200 on correct)
- Ensure tests are reliable (no flakiness from timing issues)

**Non-Goals:**
- Authentication/authorization (deferred to future change)
- Multi-node clustering or persistent storage
- Performance or load testing
- Testing internal implementation details

## Decisions

### Test structure: isolated binary in `tests/`

**Decision:** Place integration tests in a separate `tests/` binary, not co-located with unit tests in `src/`.

**Rationale:**
- Clear separation between unit tests (fast, isolated) and integration tests (full-stack, slower)
- CI can run them separately (unit tests on every commit, integration tests on PR)
- Future auth middleware changes automatically affect integration tests since they go through the full router
- `tests/` binary is the idiomatic Rust pattern for integration tests

**Alternatives considered:**
- `src/integration.rs` with `#[tokio::test]` — co-locates with unit tests, harder to run separately in CI
- Tower `TestService` per middleware — tests layers in isolation, but doesn't exercise the full router chain

### TestApp: builds real Router with real store

**Decision:** `TestApp` constructs a real `Router` with `AppState` containing real `InMemoryStore` and `EventBus`.

**Rationale:**
- Tests exercise the actual wiring — if routes are misconfigured or service is not properly injected, tests catch it
- No mocking needed — the in-memory store is fast and deterministic
- Future changes to router or middleware composition are automatically tested

### TestClient: thin wrapper around `axum::test::TestClient`

**Decision:** `TestClient` wraps `axum::test::TestClient` with typed methods for each API operation.

**Rationale:**
- `axum::test::TestClient` handles the mechanics of sending requests and parsing responses
- Typed methods (`create_schema`, `create_object`, etc.) provide clarity and reduce boilerplate in tests
- Future auth changes will manifest as additional methods or parameters on `TestClient`

### Watch reliability: subscribe-before-create + timeout

**Decision:** Watch tests subscribe to the SSE stream BEFORE creating the object, and use a 2-second timeout on receiving events.

**Rationale:**
- Subscribing after creating risks missing the event if the handler is fast
- Timeout prevents tests from hanging indefinitely if the event is never received
- 2 seconds is generous for in-memory operations — flakiness would indicate a real bug

**Alternatives considered:**
- Polling with retry — adds complexity, same reliability characteristics
- No timeout — tests could hang forever on bug

### Pagination: behavior-only assertions

**Decision:** Pagination tests verify correct items, counts, and continuation — not the base64 encoding of continue tokens.

**Rationale:**
- Continue token encoding is tested in unit tests (`store::memory::tests`)
- Integration tests exercise the API contract, not implementation details
- If the encoding scheme changes, unit tests catch it, integration tests still pass

## Risks / Trade-offs

[Risk] SSE tests may be inherently timing-sensitive
→ **Mitigation:** Subscribe before create + 2s timeout + explicit assertion message on failure

[Risk] Tests depend on internal module structure (TestApp accesses kapi modules directly)
→ **Mitigation:** TestApp wraps the actual app construction from lib.rs — if refactoring happens, TestApp is updated along with it

[Risk] Integration tests could become slow as the suite grows
→ **Mitigation:** Keep tests fast by using in-memory store (no I/O). CI can run unit tests on every commit, integration tests on PR.

## Open Questions

(None — all technical decisions have been made.)

## Migration Plan

1. Create `tests/Cargo.toml` with path dependency on `..`
2. Create `tests/src/lib.rs` with `TestApp`, `TestClient`, and fixtures
3. Create test modules: `object_crud.rs`, `watch_events.rs`, `schema_deletion.rs`, `schema_validation.rs`, `optimistic_concurrency.rs`
4. Run `cargo test` to verify all tests pass
5. Run `cargo clippy -- -D warnings` to ensure clean
6. Run `cargo doc --no-deps` to verify documentation generation