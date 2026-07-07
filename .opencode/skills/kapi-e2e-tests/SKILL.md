---
name: kapi-e2e-tests
description: "Run kapi end-to-end tests. Triggers: test kapi, run tests, e2e, verify kapi, test labels, test watch, test finalizers, test status, test generation, test annotations, test persistence, test label selection, test label selector, test list filtering, run all tests, test cli, test cli get, test cli apply, test cli delete, test cli watch, test cli status, test cli namespace, test cli output"
---

# kapi-e2e-tests

Run kapi end-to-end tests. Supports running all tests or specific test areas.

## When to Use

Use this skill when the user asks to:
- Run kapi tests / e2e tests
- Test specific areas (labels, annotations, finalizers, watch, status, etc.)
- Verify kapi functionality
- Run the automated test suite
- Test the CLI (kapi command)
- Test CLI commands (get, apply, delete, watch, status)
- Test both API and CLI together

## Test Areas

The tests are organized into these areas:

### Raw API Tests (curl)

| Area | Tests | Description |
|------|-------|-------------|
| `watch` | 1, 2, 3, 4 | Watch basics: fieldSelector, lifecycle, cleanup, concurrent |
| `labels` | 5, 6, 7, 8, 9 | Labels: create, update, Schema, validation, list |
| `annotations` | 38, 39, 7, 40, 9 | Annotations: create, update, Schema, validation, list |
| `label-selectors` | 11, 12, 13, 14, 15, 16, 17 | Label selectors on watch: equality, AND, !key, !=, validation, empty, mixed |
| `list-filtering` | 18, 19, 20, 21, 22, 23, 24 | List filtering: fieldSelector, labelSelector, combined, pagination |
| `status` | 25, 26, 27, 28, 29, 30, 31, 32 | Status subresource: create/update, validation, 404, replace, events |
| `generation` | 33, 34, 35, 36, 37 | Generation: start, metadata-only, spec, status, independence |
| `finalizers` | 41, 42, 43, 44, 45, 46, 47, 48, 49 | Finalizers: create, delete, update, validation, watch, persistence |
| `persistence` | 10, 49 | SQLite persistence: labels/annotations and finalizers survive restart |
| `concurrent` | 50, 51 | Concurrent spec/status updates, failed ops don't publish events |
| `schema-scope` | 52-59 | Schema registration: scope field, cluster-scoped Schema |
| `namespace-resource` | 84-93 | Namespace-as-resource: CRUD, default protection, existence validation, deletion blocking |
| `namespace-crud` | 60-68 | Namespace-scoped CRUD: create, get, list, update, delete |
| `cross-namespace` | 69-71 | Cross-namespace list: all namespaces, pagination, comparison |
| `cluster-scoped` | 72-76 | Cluster-scoped resources: CRUD with namespace=null |
| `scope-validation` | 77-81 | Scope validation: reject cluster+ns, default ns, same name diff ns |
| `namespace-watch` | 82-83 | Namespace-aware watch: scoped and cross-namespace |
| `all` | 1-93 | Run all tests |

### CLI Tests (kapi command)

| Area | Tests | Description |
|------|-------|-------------|
| `cli-get` | C1-C8 | Get command: single object, list, label selector, output formats (table/json/yaml), namespace flag, all-namespaces |
| `cli-apply` | C9-C15 | Apply command: create from file, update from file, YAML/JSON parsing, namespace resolution, schema creation |
| `cli-delete` | C16-C18 | Delete command: basic delete, not-found error, namespace handling |
| `cli-watch` | C19-C22 | Watch command: basic watch, label selector filter, namespace scoping, all-namespaces |
| `cli-status` | C23-C26 | Status commands: status get, status apply from file, status with no value |
| `cli-namespace` | C27-C30 | Namespace handling: default namespace, explicit namespace, cluster-scoped warning, namespace resolution |
| `cli-output` | C31-C33 | Output formats: table alignment, JSON validity, YAML validity |
| `cli-all` | C1-C33 | Run all CLI tests |

## Workflow

### 1. Parse User Intent

Determine which test area(s) to run based on user request:
- "test labels" → `labels` (raw API)
- "test watch" → `watch` (raw API)
- "test finalizers" → `finalizers` (raw API)
- "run all tests" → `all` (raw API)
- "test labels and annotations" → `labels`, `annotations` (raw API)
- "test cli" → `cli-all`
- "test cli get" → `cli-get`
- "test cli apply" → `cli-apply`
- "test cli and api" → run both raw API and CLI tests for the relevant areas
- "run all tests including cli" → `all` + `cli-all`

### 2. Clean Up Previous Runs

Always clean up stale state from previous test runs before starting:

```bash
# Kill any existing server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1

# Delete DB files and logs from previous runs
rm -f /tmp/watch-*.log /tmp/kapi-server*.log /tmp/kapi-test.db /tmp/kapi-persist-*.db ./kapi.db
unset KAPI_DB_PATH
```

### 3. Build and Start Server

```bash
# Build server and CLI
cargo build

# Verify CLI binary exists (test scripts auto-detect this path)
ls -la ./target/debug/kapi

# Start server with trace logging
RUST_LOG=kapi=trace cargo run --bin kapi-server > /tmp/kapi-server.log 2>&1 &
sleep 3

# Verify server is up
curl -s http://localhost:8080/apis/kapi.io/v1/Schema
```

### 4. Run Tests

Execute the appropriate test script(s) from the `scripts/` directory:

```bash
# Run specific raw API area
bash .opencode/skills/kapi-e2e-tests/scripts/test-<area>.sh

# Run specific CLI area
bash .opencode/skills/kapi-e2e-tests/scripts/test-cli-<area>.sh

# Or run all
bash .opencode/skills/kapi-e2e-tests/scripts/test-all.sh
bash .opencode/skills/kapi-e2e-tests/scripts/test-cli-all.sh
```

### 5. Present Results

Present results in a clean table format:

```
## Test Results: <area>

| # | Test | Result | Notes |
|---|------|--------|-------|
| 1 | Watch fieldSelector | PASS | Only target delivered |
| 2 | Watch lifecycle | PASS | All 3 event types received |
...

**Summary:** X/Y tests passed
```

For CLI tests, prefix test numbers with `C` to distinguish from raw API tests.

## Important Notes

- Tests are designed to run sequentially within an area (later tests may depend on earlier ones)
- The `persistence` area requires server restarts (kills and restarts with SQLite)
- Each test area is self-contained and can be run independently
- The `all` area runs tests in the correct order to handle dependencies
- Server must be running before tests start
- Tests use a unique `TEST_RUN` timestamp to avoid collisions
- CLI tests require the `kapi` binary to be built (`cargo build`). Scripts auto-detect `./target/debug/kapi` or use the `KAPI` env var if set.
- CLI tests use the same test data as raw API tests where applicable
- CLI tests verify command parsing, output formatting, and error handling
- Namespace objects require a non-empty `spec` field (e.g., `"spec":{"annotations":{}}}`). An empty `"spec":{}` is rejected by the server.

## Example Usage

User: "test the label selection"
→ Run `label-selectors` area (tests 11-17)

User: "run all tests"
→ Run `all` area (tests 1-93)

User: "test finalizers and persistence"
→ Run `finalizers` area (tests 41-49) then `persistence` area (tests 10, 49)

User: "test the cli"
→ Run `cli-all` area (tests C1-C33)

User: "test cli get and apply"
→ Run `cli-get` area (tests C1-C8) then `cli-apply` area (tests C9-C15)

User: "run all tests including cli"
→ Run `all` area (tests 1-93) then `cli-all` area (tests C1-C33)
