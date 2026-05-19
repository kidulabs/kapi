## 1. Project Setup

- [x] 1.1 Create `tests/Cargo.toml` with `[[bin]]` for integration test binary
  ```toml
  [dependencies]
  kapi = { path = ".." }
  tokio = { version = "1", features = ["full"] }
  serde_json = "1"
  futures = "0.3"
  axum = "0.7"
  ```
  // Setup includes creating the Cargo.toml with path dependency on kapi crate

- [x] 1.2 Create `tests/src/` directory structure with `lib.rs` and module files
  // Each module file (object_crud.rs, watch_events.rs, etc.) will be a separate binary section

- [x] 1.3 Create `tests/src/main.rs` that runs all test modules
  // Simple main that could invoke submodules or be structured as a test harness

## 2. Test Infrastructure (`tests/src/lib.rs`)

- [x] 2.1 Implement `TestApp` struct that constructs a real `Router` with `AppState`
  ```rust
  pub struct TestApp {
      router: Router,
      store: Arc<InMemoryStore>,
      event_bus: Arc<EventBus>,
  }
  // TestApp::new() compiles meta-schema, creates ObjectService, builds Router
  ```
  // Builds the actual app from lib.rs, not a mock — tests exercise real wiring

- [x] 2.2 Implement `TestClient` wrapper around `axum::test::TestClient`
  ```rust
  pub struct TestClient {
      inner: axum::test::TestClient<Router, AppState>,
  }
  // Provides typed methods: create_schema, create_object, get_object, list_objects,
  // update_object, delete_object, watch
  ```
  // Thin wrapper that delegates to TestClient, adds typed API surface

- [x] 2.3 Add fixture helpers for Widget schema and objects
  ```rust
  pub fn widget_schema() -> Value
  pub fn widget(name: &str, color: &str, size: i64) -> Value
  pub fn widget_with_rv(name: &str, color: &str, size: i64, rv: u64) -> StoredObject
  ```
  // Reusable test data builders — reduces repetition in test modules

- [x] 2.4 Add response helpers for parsing JSON bodies and status codes
  ```rust
  impl TestClient {
      pub fn parse_body<T: DeserializeOwned>(response: Response) -> T
      pub fn assert_status(response: Response, expected: StatusCode)
  }
  ```
  // Common assertions factored out — tests focus on business assertions, not HTTP mechanics

## 3. Object CRUD Tests (`tests/src/object_crud.rs`)

- [x] 3.1 Test: register schema → create object → get object → verify data
  ```rust
  #[tokio::test]
  async fn create_schema_then_object() {
      // POST Schema → assert 201
      // POST Widget → assert 201 + correct data
      // GET Widget/{name} → assert 200 + correct data
  }
  ```
  // Exercises the happy path from schema registration through first read

- [x] 3.2 Test: full CRUD flow — create → update → delete
  ```rust
  #[tokio::test]
  async fn full_crud_flow() {
      // Create object
      // Update with correct rv → 200, new rv
      // Delete → 200
      // GET → 404
  }
  ```
  // Verifies the complete object lifecycle

- [x] 3.3 Test: list pagination — single page (limit > items)
  // Create 2 objects, list with limit=5, assert 2 items, no continue token

- [x] 3.4 Test: list pagination — two pages with continue token
  // Create 4 objects, limit=2, verify page1 has continue, page2 has no continue

- [x] 3.5 Test: list pagination — resumes from correct position
  // Create ["a","b","c","d"], page1=["a","b"], page2=["c","d"], no duplicates/missing

- [x] 3.6 Test: list pagination — exhausted (no continue token on last page)
  // Last page should have items but no continue token

## 4. Watch Events Tests (`tests/src/watch_events.rs`)

- [x] 4.1 Implement helper: `await_event_or_timeout(stream, timeout)` 
  ```rust
  async fn await_event_or_timeout(
      stream: impl Stream<Item = WatchEvent>,
      duration: Duration,
  ) -> Option<WatchEvent>
  // Polls stream, returns event if received within timeout, None on timeout
  // Timeout message should indicate what event was expected and what happened
  ```
  // Reusable timeout wrapper for SSE reliability

- [x] 4.2 Test: watch Schema collection receives Added event
  ```rust
  #[tokio::test]
  async fn watch_schema_receives_added_event() {
      // Start watching BEFORE creating (subscribe first, create second)
      // Create Schema
      // Assert event received within 2s with eventType=Added
  }
  ```
  // Critical: subscribe before create ensures event is not missed

- [x] 4.3 Test: watch object kind receives Added/Modified/Deleted events
  // Register Widget schema, watch Widget collection, create/update/delete objects

## 5. Schema Deletion Tests (`tests/src/schema_deletion.rs`)

- [x] 5.1 Test: delete schema with no objects → 200 OK
  ```rust
  #[tokio::test]
  async fn delete_schema_no_objects_succeeds() {
      // Register Widget schema
      // DELETE Schema/Widget.example.io
      // assert 200 OK
  }
  ```

- [x] 5.2 Test: delete schema with existing objects → 409 Conflict with count
  ```rust
  #[tokio::test]
  async fn delete_schema_with_objects_returns_conflict() {
      // Register Widget schema
      // Create 2 Widget objects
      // DELETE Schema/Widget.example.io
      // assert 409 with count >= 1
  }
  ```

## 6. Schema Validation Tests (`tests/src/schema_validation.rs`)

- [x] 6.1 Test: valid schema is accepted → 201
  // POST valid Schema with proper jsonSchema → 201

- [x] 6.2 Test: invalid jsonSchema type → 422
  ```rust
  #[tokio::test]
  async fn invalid_json_schema_type_rejected() {
      // jsonSchema with "type": "not-a-real-type"
      // POST → assert 422
  }
  ```

- [x] 6.3 Test: missing required fields (targetKind) → 422
  // POST Schema missing targetKind → 422

## 7. Optimistic Concurrency Tests (`tests/src/optimistic_concurrency.rs`)

- [x] 7.1 Test: update with correct resourceVersion → 200, new rv > old rv
  ```rust
  #[tokio::test]
  async fn update_correct_rv_succeeds() {
      // Create object (rv=X)
      // PUT with rv=X → 200, response rv > X
  }
  ```

- [x] 7.2 Test: update with wrong resourceVersion → 409 Conflict
  ```rust
  #[tokio::test]
  async fn update_wrong_rv_returns_conflict() {
      // Create object (rv=X)
      // PUT with rv=X+99 → 409
  }
  ```

## 8. Verification (T62, T63)

- [x] 8.1 Run `cargo test` — all tests pass, no warnings
  // Unit tests + integration tests both pass

- [x] 8.2 Run `cargo clippy -- -D warnings` — no warnings
  // Ensures code quality standards met

- [x] 8.3 Run `cargo doc --no-deps` — documentation generates without errors
  // No broken doc links or missing documentation

- [x] 8.4 Mark P9 tasks as complete in `roadmap.md`
  // Check off T56-T63 in the roadmap