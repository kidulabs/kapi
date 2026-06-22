---
name: kapi-e2e-tests
description: "Run kapi end-to-end tests from docs/testprompt.md. Triggers: test kapi, run tests, e2e, verify kapi, test labels, test watch, test finalizers, test status, test generation, test annotations, test persistence, test label selection, test label selector, test list filtering, run all tests"
---

# kapi-e2e-tests

Run kapi end-to-end tests from `docs/testprompt.md`. Supports running all tests or specific test areas.

## When to Use

Use this skill when the user asks to:
- Run kapi tests / e2e tests
- Test specific areas (labels, annotations, finalizers, watch, status, etc.)
- Verify kapi functionality
- Run the test suite from testprompt.md

## Test Areas

The tests are organized into these areas:

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
| `all` | 1-51 | Run all tests |

## Workflow

### 1. Parse User Intent

Determine which test area(s) to run based on user request:
- "test labels" → `labels`
- "test watch" → `watch`
- "test finalizers" → `finalizers`
- "run all tests" → `all`
- "test labels and annotations" → `labels`, `annotations`

### 2. Check Server State

```bash
# Check if server is running
lsof -ti :8080 2>/dev/null && echo "RUNNING" || echo "NOT_RUNNING"
```

### 3. Setup (if needed)

If server is not running:

```bash
# Build
cargo build

# Clean up from previous runs
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1
rm -f /tmp/watch-*.log /tmp/kapi-server*.log /tmp/kapi-test.db /tmp/kapi-persist-*.db ./kapi.db
unset KAPI_DB_PATH

# Start server with trace logging
RUST_LOG=kapi=trace cargo run > /tmp/kapi-server.log 2>&1 &
sleep 3

# Verify server is up
curl -s http://localhost:8080/apis/kapi.io/v1/Schema
```

### 4. Run Tests

Execute the appropriate test script(s) from the `scripts/` directory:

```bash
# Run specific area
bash .opencode/skills/kapi-e2e-tests/scripts/test-<area>.sh

# Or run all
bash .opencode/skills/kapi-e2e-tests/scripts/test-all.sh
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

## Important Notes

- Tests are designed to run sequentially within an area (later tests may depend on earlier ones)
- The `persistence` area requires server restarts (kills and restarts with SQLite)
- Each test area is self-contained and can be run independently
- The `all` area runs tests in the correct order to handle dependencies
- Server must be running before tests start
- Tests use a unique `TEST_RUN` timestamp to avoid collisions

## Example Usage

User: "test the label selection"
→ Run `label-selectors` area (tests 11-17)

User: "run all tests"
→ Run `all` area (tests 1-51)

User: "test finalizers and persistence"
→ Run `finalizers` area (tests 41-49) then `persistence` area (tests 10, 49)
