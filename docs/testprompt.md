# Kapi End-to-End Test Prompts

Use these prompts to verify kapi's watch semantics, label validation, and persistence by running the server with trace logging and exercising the endpoints with curl.

---

## Test Index

| # | Area | Tests |
|---|------|-------|
| Watch basics | fieldSelector, lifecycle, cleanup, concurrent | 1, 2, 3, 4 |
| Field selectors | watch + list, validation | 1, 18, 20, 23, 24 |
| Label selectors (watch) | equality, AND, !key, !=, empty, mixed | 11, 12, 13, 14, 16, 17, 23 |
| Label selectors (list) | filter, combined, validation | 19, 20, 21, 22, 23, 24 |
| Label selector validation | 400s on both watch and list | 15 |
| Labels | create (with/without) / update / Schema / matrix / list | 5, 6, 7, 8, 9 |
| Annotations | create (with/without) / update / Schema / matrix / list | 38, 39, 7, 40, 9 |
| List filtering & pagination | fieldSelector, labelSelector, combined, filter+limit, empty | 18, 19, 20, 21, 22 |
| Persistence (SQLite) | labels + annotations across restart | 10 |
| Status subresource | create/update/get, validation, 404, replace, side effects, events | 25, 26, 27, 28, 29, 30, 31, 32 |
| Generation | start, metadata-only, spec, status, independence | 33, 34, 35, 36, 37 |
| Finalizers | create (with/without) / delete / update / validation / watch | 41, 42, 43, 44, 45, 46, 47, 48, 49 |

> **Note:** Some tests are deliberately split to keep operator semantics readable (e.g. one test per `labelSelector` operator). Matrix-style validation tests (label/annotation) merge what would otherwise be near-identical boilerplate.

---

## Prerequisites & Setup

```bash
# Build
cargo build

# Generate a unique suffix for this run to avoid name collisions on re-runs
TEST_RUN=$(date +%s)

# Clean up from previous runs
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1
rm -f /tmp/watch-*.log /tmp/kapi-server*.log /tmp/kapi-test.db /tmp/kapi-persist-*.db ./kapi.db
unset KAPI_DB_PATH

# Start server with trace logging (in-memory store; SQLite is started by Test 10)
RUST_LOG=kapi=trace cargo run > /tmp/kapi-server.log 2>&1 &
sleep 3

# Verify server is up
curl -s http://localhost:8080/apis/kapi.io/v1/Schema
```

> **Re-run safety**: Each test uses `$TEST_RUN` as a suffix in both **object names** (e.g. `target-widget-$TEST_RUN`) and the **schema's `targetGroup`** (e.g. `example.io.$TEST_RUN`). The unique group ensures the Widget schema can be re-registered on every run without colliding with schemas from prior runs in the same in-memory store. Test 10 (SQLite persistence) uses hard-coded object names by design — it must be re-runnable against the same database file, and uses the same `$TEST_RUN` group so the schema registration after restart matches what was stored. Clean its objects in the Cleanup section before re-running.

---

## Shared Helpers

The tests below reference these bash functions. Define them once in your shell after running the Setup section, or paste the block into a sourced script.

```bash
# Register the Widget schema (example.io.$TEST_RUN/v1) — idempotent
register_widget_schema() {
  curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
    -H "Content-Type: application/json" \
    -d '{
      "targetGroup": "example.io.'$TEST_RUN'",
      "targetVersion": "v1",
      "targetKind": "Widget",
      "specSchema": {
        "type": "object",
        "properties": {
          "color": { "type": "string" },
          "size":  { "type": "integer" }
        },
        "required": ["color", "size"]
      }
    }' > /dev/null
}

# Register the Widget schema WITH a statusSchema (for status subresource tests)
register_widget_schema_with_status() {
  curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
    -H "Content-Type: application/json" \
    -d '{
      "targetGroup": "example.io.'$TEST_RUN'",
      "targetVersion": "v1",
      "targetKind": "Widget",
      "specSchema": {
        "type": "object",
        "properties": {
          "color": { "type": "string" },
          "size":  { "type": "integer" }
        },
        "required": ["color", "size"]
      },
      "statusSchema": {
        "type": "object",
        "properties": {
          "phase":   { "type": "string" },
          "message": { "type": "string" }
        }
      }
    }' > /dev/null
}

# Start a watch, capture output to <logfile>, return the PID via stdout.
# Usage: WATCH_PID=$(start_watch '?watch=true&labelSelector=app=nginx' /tmp/x.log)
start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget${query}" \
    > "$logfile" 2>&1 &
  echo $!
}

# Extract resourceVersion, createdAt, updatedAt from GET <name>.
# Sets globals: GET_RV, GET_CREATED, GET_UPDATED.
get_system_fields() {
  local name="$1"
  local body
  body=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${name}")
  GET_RV=$(echo "$body"      | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
  GET_CREATED=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
  GET_UPDATED=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")
}
```

> **Note on `system` fields**: The server uses `resourceVersion` for optimistic concurrency and may ignore `createdAt`/`updatedAt` round-tripped from the client. The tests pass them back anyway to mirror how a real client would behave. If a future server change enforces them, the helpers above are the single place to update.

---

## Test 1: Watch with fieldSelector — matching event delivered, non-matching filtered

**Goal:** Verify that `?fieldSelector=metadata.name=<value>` only delivers events for the specified name.

```bash
# 1. Register the Widget schema (with statusSchema for later status tests)
register_widget_schema_with_status

# 2. Start a watch filtered to "target-widget-$TEST_RUN"
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=target-widget-$TEST_RUN" /tmp/watch-fieldselector.log)
sleep 2

# Verify the watch is still alive
if ! kill -0 $WATCH_PID 2>/dev/null; then echo "ERROR: watch died before events"; fi

# 3. Create a NON-target widget (should NOT be delivered to the watch)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 4. Create the TARGET widget (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"target-widget-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received only the target-widget event
echo "=== Client received ==="
cat /tmp/watch-fieldselector.log

# 7. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher subscribed|event delivered|event filtered|watch stream)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Client output contains only `target-widget-$TEST_RUN` Added event (no `other-widget-$TEST_RUN`)
- Server logs show:
  - `sse watch stream started`
  - `watcher subscribed`
  - `event filtered out by watcher filter name=other-widget-$TEST_RUN`
  - `event delivered to watcher name=target-widget-$TEST_RUN`

---

## Test 2: Watch all events — Added, Modified, Deleted

**Goal:** Verify that `?watch=true` (no fieldSelector) receives all event types.

```bash
# 1. Start a watch-all
WATCH_PID=$(start_watch "?watch=true" /tmp/watch-all.log)
sleep 1

# 2. Create a widget (expect Added)
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":5}}")

# Verify created object has empty finalizers
echo "$CREATE_RESP" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
f = obj['metadata'].get('finalizers', None)
assert f == [], f'Expected finalizers=[], got {f}'
print('PASS: finalizers=[] on create')
"

sleep 1

# 3. Update the widget (expect Modified)
get_system_fields "lifecycle-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lifecycle-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"yellow\",\"size\":10}
  }"

sleep 1

# 4. Delete the widget (expect Deleted)
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lifecycle-$TEST_RUN"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received all three event types
# NOTE: The SSE JSON uses "eventType" (camelCase) as the discriminator field.
echo "=== Client received ==="
cat /tmp/watch-all.log

echo "=== Event counts ==="
grep -c '"eventType":"Added"'    /tmp/watch-all.log && echo " Added events"
grep -c '"eventType":"Modified"' /tmp/watch-all.log && echo " Modified events"
grep -c '"eventType":"Deleted"'  /tmp/watch-all.log && echo " Deleted events"

# 7. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher subscribed|event delivered|watch stream)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Client output contains three SSE events: `"eventType":"Added"`, `"eventType":"Modified"`, `"eventType":"Deleted"`
- All three events reference the same `lifecycle-$TEST_RUN` object
- Server logs show three `event delivered to watcher name=lifecycle-$TEST_RUN` entries

---

## Test 3: Abrupt connection cleanup — dead watcher removed on next publish

**Goal:** Verify that when a client disconnects abruptly (e.g., `SIGKILL`), the watcher resource is cleaned up lazily on the next publish.

```bash
# 1. Start a watch
WATCH_PID=$(start_watch "?watch=true" /tmp/watch-abrupt.log)
sleep 1

# 2. Abruptly kill the curl process (simulates network disconnect / SIGKILL)
kill -9 $WATCH_PID 2>/dev/null
echo "Killed watch process (simulating abrupt disconnect)"
sleep 1

# 3. Trigger a publish — this should detect and clean up the dead watcher
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cleanup-trigger-$TEST_RUN\"},\"spec\":{\"color\":\"black\",\"size\":99}}"

sleep 2

# 4. Verify trace logs show cleanup
echo "=== Server trace logs for cleanup ==="
grep -E "(watcher channel closed|watcher subscribed|event delivered)" /tmp/kapi-server.log | tail -5
```

**Expected results:**
- Server logs show `watcher channel closed, removing` after the publish
- The dead watcher is removed from the EventBus (lazy cleanup via `retain()`)

---

## Test 4: Simultaneous watches — fieldSelector + all events

**Goal:** Verify that two concurrent watches (one filtered, one unfiltered) operate independently.

```bash
# 1. Start filtered watch (name=named-$TEST_RUN)
NAMED_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=named-$TEST_RUN" /tmp/watch-named.log)

# 2. Start watch-all
ALL_PID=$(start_watch "?watch=true" /tmp/watch-sim-all.log)

sleep 1

# 3. Create "named-$TEST_RUN" — both watchers should receive it
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"named-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":3}}"

sleep 1

# 4. Create "other-$TEST_RUN" — only watch-all should receive it
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-$TEST_RUN\"},\"spec\":{\"color\":\"yellow\",\"size\":4}}"

sleep 2

# 5. Kill both watches
kill $NAMED_PID $ALL_PID 2>/dev/null

# 6. Verify
echo "=== Named watch received ==="
cat /tmp/watch-named.log
echo ""
echo "=== All watch received ==="
cat /tmp/watch-sim-all.log
```

**Expected results:**
- Named watch: only `named-$TEST_RUN` event
- All watch: both `named-$TEST_RUN` and `other-$TEST_RUN` events

---

## Test 5: Labels — create with/without labels

**Goal:** Verify that `metadata.labels` are persisted when provided on create, and default to `{}` when omitted.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# Each entry: <name-suffix>|<labels-json>
CASES=(
  "with-labels|{\"app\":\"nginx\",\"env\":\"prod\",\"app.kubernetes.io/version\":\"v1.2.3\"}"
  "without-labels|null"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix labels <<< "$case"
  echo "=== Case: $suffix ==="
  if [ "$labels" = "null" ]; then
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{\"metadata\":{\"name\":\"${suffix}-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -m json.tool
  else
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{
        \"metadata\": {
          \"name\": \"${suffix}-widget-$TEST_RUN\",
          \"labels\": $labels
        },
        \"spec\": {
          \"color\": \"blue\",
          \"size\": 10
        }
      }" | python3 -m json.tool
  fi
  echo
done

# 2. Verify via GET
echo "=== GET labels ==="
for suffix in "with-labels" "without-labels"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${suffix}-widget-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'\${suffix}: labels={obj[\"metadata\"].get(\"labels\", {})}')
"
done
```

**Expected results:**
- `with-labels` case: Create and GET responses contain `"labels": { "app": "nginx", "env": "prod", "app.kubernetes.io/version": "v1.2.3" }`
- `without-labels` case: Create and GET responses contain `"labels": {}`
- Prefixed key `app.kubernetes.io/version` is accepted

---

## Test 6: Labels — update with changed labels (replace semantics)

**Goal:** Verify that updating labels applies diff-based changes (add, modify, remove).

```bash
# 1. Get the current labeled widget
echo "=== Fetch current state ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-labels-widget-$TEST_RUN" | python3 -m json.tool

# 2. Update: remove "env", change "app" to "httpd", add "tier" -> "frontend"
get_system_fields "with-labels-widget-$TEST_RUN"

echo "=== Update with changed labels ==="
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-labels-widget-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": {\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\": {
      \"name\": \"with-labels-widget-$TEST_RUN\",
      \"labels\": { \"app\": \"httpd\", \"app.kubernetes.io/version\": \"v1.2.3\", \"tier\": \"frontend\" }
    },
    \"system\": {\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\": {\"color\":\"blue\",\"size\":10}}
  }" | python3 -m json.tool

# 3. GET and verify labels changed
echo "=== GET after update ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-labels-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- `"env"` label removed (was `"prod"`)
- `"app"` changed from `"nginx"` to `"httpd"`
- `"tier": "frontend"` added
- `"app.kubernetes.io/version": "v1.2.3"` unchanged
- `resourceVersion` incremented

---

## Test 7: Schema with labels and annotations

**Goal:** Verify that Schema objects support both labels and annotations.

```bash
# 1. Create a Schema with labels and annotations
echo "=== Create Schema with labels and annotations ==="
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"labels\": { \"team\": \"platform\", \"status\": \"active\" },
      \"annotations\": { \"team\": \"platform\", \"docs\": \"https://example.com/docs\" }
    },
    \"targetGroup\": \"test-$TEST_RUN.io\",
    \"targetVersion\": \"v1\",
    \"targetKind\": \"Gadget\",
    \"specSchema\": {
      \"type\": \"object\",
      \"properties\": {
        \"name\": { \"type\": \"string\" }
      }
    }
  }" | python3 -m json.tool

# 2. GET the Schema and verify labels and annotations
# Schema name format: {targetKind}.{targetGroup}
echo "=== GET Schema with labels and annotations ==="
curl -s "http://localhost:8080/apis/kapi.io/v1/Schema/Gadget.test-$TEST_RUN.io" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
md = obj['metadata']
print(f'labels: {md.get(\"labels\", {})}')
print(f'annotations: {md.get(\"annotations\", {})}')
"
```

**Expected results:**
- Create response contains both `"labels": { "team": "platform", "status": "active" }` and `"annotations": { "team": "platform", "docs": "https://example.com/docs" }`
- GET response contains the same labels and annotations

---

## Test 8: Label validation matrix — invalid key, value, and length limits

**Goal:** Verify that malformed `metadata.labels` are rejected with HTTP 400 and `code: InvalidLabel`. Each case asserts a different validation rule.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# 2. Run the four cases
# Use python3 to generate valid JSON with long keys/values (shell string interpolation
# would produce invalid JSON from bare 257-char strings)
LONG_KEY_JSON=$(python3 -c "import json; k='a'*257; print(json.dumps({k: 'value'}))")
LONG_VALUE_JSON=$(python3 -c "import json; print(json.dumps({'app': 'a'*257}))")

# Each entry: <name-suffix>|<labels-json>|<expected-fragment-in-error>
CASES=(
  "bad-key|{\"invalid key!\":\"value\"}|invalid"
  "bad-value|{\"app\":\"invalid value!\"}|invalid"
  "long-key|$LONG_KEY_JSON|256"
  "long-value|$LONG_VALUE_JSON|256"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix labels expected <<< "$case"
  echo "=== Case: $suffix ==="
  curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{
      \"metadata\": { \"name\": \"label-${suffix}-$TEST_RUN\", \"labels\": $labels },
      \"spec\":     { \"color\": \"blue\", \"size\": 1 }
    }"
  echo
done
```

**Expected results (all four cases):**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions the relevant rule:
  - `bad-key` → invalid characters in key
  - `bad-value` → invalid characters in value
  - `long-key` → maximum length of 256
  - `long-value` → maximum length of 256

---

## Test 9: List returns labels and annotations

**Goal:** Verify that list endpoint returns both labels and annotations for each object.

```bash
# 1. List all widgets
echo "=== List widgets ==="
curl -s http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget | python3 -c "
import sys, json
body = json.load(sys.stdin)
for item in body['items']:
    md = item['metadata']
    name = md['name']
    labels = md.get('labels', {})
    annotations = md.get('annotations', {})
    print(f'{name}: labels={labels}, annotations={annotations}')
"
```

**Expected results:**
- Each item in `items` array has both `metadata.labels` and `metadata.annotations` fields
- `with-labels-widget-$TEST_RUN` has updated labels (`app: httpd`, `tier: frontend`, etc.)
- `without-labels-widget-$TEST_RUN` has `"labels": {}`
- `with-ann-widget-$TEST_RUN` has annotations
- `without-ann-widget-$TEST_RUN` has `"annotations": {}`

---

## Test 10: SQLite persistence — labels and annotations survive restart

**Goal:** Verify that both `metadata.labels` and `metadata.annotations` are persisted by SQLite and survive a full server restart.

> **Note:** This test starts its own server with `KAPI_DB_PATH` set via inline env (not `export`), so it does not leak state to other tests. Object names are hard-coded (`persist-labels-widget`, `persist-ann-widget`) by design — the database itself is the unit under test. Delete those objects (or `rm` the db file) before re-running.

```bash
# 1. Stop the in-memory server started in the Setup section
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1

# 2. Start a fresh server with SQLite storage
PERSIST_DB=/tmp/kapi-persist-test.db
rm -f "$PERSIST_DB"
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

# 3. Register the Widget schema (idempotent)
register_widget_schema

# 4. Create the labels widget, then update its labels
LABELS_NAME="persist-labels-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"$LABELS_NAME\",
      \"labels\": { \"app\": \"nginx\", \"env\": \"prod\", \"app.kubernetes.io/version\": \"v1.2.3\" }
    },
    \"spec\": { \"color\": \"blue\", \"size\": 10 }
  }" > /dev/null

get_system_fields "$LABELS_NAME"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$LABELS_NAME" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": {\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\": {
      \"name\": \"$LABELS_NAME\",
      \"labels\": { \"app\": \"httpd\", \"app.kubernetes.io/version\": \"v1.2.3\", \"tier\": \"frontend\" }
    },
    \"system\": {\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\": {\"color\":\"blue\",\"size\":10}}
  }" > /dev/null

# 5. Create the annotations widget
ANN_NAME="persist-ann-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"$ANN_NAME\",
      \"annotations\": { \"description\": \"persistent\", \"build\": \"v1.0.0\" }
    },
    \"spec\": { \"color\": \"blue\", \"size\": 10 }
  }" > /dev/null

# 6. Snapshot both fields to disk (order-independent comparison later)
echo "=== Before restart ==="
for name in "$LABELS_NAME" "$ANN_NAME"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$name" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
md = obj['metadata']
print(f'${name}: labels={md.get(\"labels\", {})}, annotations={md.get(\"annotations\", {})}')
"
done
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$LABELS_NAME" | python3 -c "
import sys, json
with open('/tmp/kapi-labels-before.json', 'w') as f:
    json.dump(json.load(sys.stdin)['metadata']['labels'], f, sort_keys=True)
"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$ANN_NAME" | python3 -c "
import sys, json
with open('/tmp/kapi-ann-before.json', 'w') as f:
    json.dump(json.load(sys.stdin)['metadata']['annotations'], f, sort_keys=True)
"

# 7. Stop the server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 2

# 8. Restart with the same database
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

# 9. Snapshot again and compare
echo "=== After restart ==="
for name in "$LABELS_NAME" "$ANN_NAME"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$name" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
md = obj['metadata']
print(f'${name}: labels={md.get(\"labels\", {})}, annotations={md.get(\"annotations\", {})}')
"
done
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$LABELS_NAME" | python3 -c "
import sys, json
with open('/tmp/kapi-labels-after.json', 'w') as f:
    json.dump(json.load(sys.stdin)['metadata']['labels'], f, sort_keys=True)
"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$ANN_NAME" | python3 -c "
import sys, json
with open('/tmp/kapi-ann-after.json', 'w') as f:
    json.dump(json.load(sys.stdin)['metadata']['annotations'], f, sort_keys=True)
"

# 10. Compare semantically
echo "=== Comparison ==="
python3 -c "
import json
for field in ['labels', 'ann']:
    with open(f'/tmp/kapi-{field}-before.json') as f: before = json.load(f)
    with open(f'/tmp/kapi-{field}-after.json')  as f: after  = json.load(f)
    print(f'{field}: before={before}')
    print(f'{field}: after ={after}')
    assert before == after, f'{field} differ: {before} vs {after}'
print('PASS: Labels and annotations survived restart')
"
```

**Expected results:**
- Both widgets visible with their final field values before restart
- After restart, both widgets return identical labels / annotations
- Final line: `PASS: Labels and annotations survived restart`

---

## Test 11: Watch with labelSelector equality — matching event delivered

**Goal:** Verify that `?labelSelector=app=nginx` only delivers events for objects with matching labels.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# 2. Start a watch filtered by label selector
WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx" /tmp/watch-labelselector.log)
sleep 2

# Verify the watch is still alive
if ! kill -0 $WATCH_PID 2>/dev/null; then echo "ERROR: watch died before events"; fi

# 3. Create a widget with NON-matching labels (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-labels-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 4. Create a widget with MATCHING labels (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"matching-labels-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received only the matching event
echo "=== Client received ==="
cat /tmp/watch-labelselector.log

# 7. Verify only matching event arrived
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-labelselector.log
```

**Expected results:**
- Client output contains only `matching-labels-$TEST_RUN` Added event (no `other-labels-$TEST_RUN`)
- Server logs show `event delivered to watcher name=matching-labels-$TEST_RUN`

---

## Test 12: Watch with labelSelector AND combinator — multiple requirements

**Goal:** Verify that comma-separated label selectors require ALL labels to match.

```bash
# 1. Start a watch with AND combinator: app=nginx AND env=prod
WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx,env=prod" /tmp/watch-label-and.log)
sleep 2

# 2. Create widget with only app=nginx (should NOT be delivered — missing env)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget with only env=prod (should NOT be delivered — missing app)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match2-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"green\",\"size\":2}}"

sleep 1

# 4. Create widget with BOTH labels (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"full-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"env\":\"prod\"}},\"spec\":{\"color\":\"red\",\"size\":3}}"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify only the full-match event arrived
echo "=== Client received ==="
cat /tmp/watch-label-and.log
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-label-and.log
```

**Expected results:**
- Client output contains only `full-match-$TEST_RUN` event
- `partial-match-$TEST_RUN` and `partial-match2-$TEST_RUN` are filtered out

---

## Test 13: Watch with labelSelector non-existence (!key)

**Goal:** Verify that `?labelSelector=!experimental` delivers events for objects WITHOUT the specified label.

```bash
# 1. Start a watch for objects without the "experimental" label
WATCH_PID=$(start_watch "?watch=true&labelSelector=!experimental" /tmp/watch-label-notexists.log)
sleep 2

# 2. Create widget WITH experimental label (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"has-experimental-$TEST_RUN\",\"labels\":{\"experimental\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget WITHOUT experimental label (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-experimental-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 2

# 4. Kill the watch
kill $WATCH_PID 2>/dev/null

# 5. Verify only the no-experimental event arrived
echo "=== Client received ==="
cat /tmp/watch-label-notexists.log
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-label-notexists.log
```

**Expected results:**
- Client output contains only `no-experimental-$TEST_RUN` event
- `has-experimental-$TEST_RUN` is filtered out

---

## Test 14: Watch with labelSelector inequality (key!=value)

**Goal:** Verify that `?labelSelector=env!=prod` delivers events for objects where the label has a different value OR is absent.

```bash
# 1. Start a watch for objects where env is NOT prod
WATCH_PID=$(start_watch "?watch=true&labelSelector=env!=prod" /tmp/watch-label-notequals.log)
sleep 2

# 2. Create widget with env=prod (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-prod-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget with env=staging (should be delivered — different value)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-staging-$TEST_RUN\",\"labels\":{\"env\":\"staging\"}},\"spec\":{\"color\":\"green\",\"size\":2}}"

sleep 1

# 4. Create widget without env label (should be delivered — absence satisfies inequality)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-env-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":3}}"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify events
echo "=== Client received ==="
cat /tmp/watch-label-notequals.log
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-label-notequals.log
```

**Expected results:**
- Client output contains `is-staging-$TEST_RUN` and `no-env-$TEST_RUN` events
- `is-prod-$TEST_RUN` is filtered out

---

## Test 15: Invalid labelSelector returns 400 — watch and list contexts

**Goal:** Verify that malformed label selectors are rejected with HTTP 400 in both the watch and list contexts. (Note: a *valid* `labelSelector` on a non-watch list request is allowed and filters results — see Test 19. This test only covers the malformed case.)

```bash
# Each entry: <case-label>|<selector-fragment>|<expected-error-fragment>
CASES=(
  "empty-value-watch|app=|empty value"
  "empty-segment-watch|app=nginx,,env=prod|empty segment"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r label selector expected <<< "$case"
  echo "=== Case: $label ==="
  curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
    "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?watch=true&labelSelector=${selector}"
  echo
done

# Also verify the list-context (non-watch) variant of empty-value fails the same way
echo "=== Case: empty-value-list ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?labelSelector=app="
```

**Expected results (all cases):**
- HTTP 400 status
- Response body contains `"code": "InvalidLabelSelector"`
- Error message mentions the relevant rule (`empty value`, `empty segment`, etc.)

---

## Test 16: Empty labelSelector matches all events

**Goal:** Verify that `?labelSelector=` (empty string) matches all objects.

```bash
# 1. Start a watch with empty labelSelector
WATCH_PID=$(start_watch "?watch=true&labelSelector=" /tmp/watch-label-empty.log)
sleep 2

# 2. Create widgets with various labels — all should be delivered
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-sel-1-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 0.5

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-sel-2-$TEST_RUN\",\"labels\":{}},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 2

# 3. Kill the watch
kill $WATCH_PID 2>/dev/null

# 4. Verify both events arrived
echo "=== Client received ==="
cat /tmp/watch-label-empty.log
echo "=== Event count ==="
grep -c '"eventType":"Added"' /tmp/watch-label-empty.log
```

**Expected results:**
- Both `empty-sel-1-$TEST_RUN` and `empty-sel-2-$TEST_RUN` events are delivered
- Event count is 2

---

## Test 17: Mixed label selector operators

**Goal:** Verify that a single labelSelector can combine different operator types.

```bash
# 1. Start a watch with mixed operators: app=nginx AND !experimental AND gpu (existence)
WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx,!experimental,gpu" /tmp/watch-label-mixed.log)
sleep 2

# 2. Create widget that matches all three requirements
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget that fails !experimental (has experimental=true)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-fail-exp-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\",\"experimental\":\"true\"}},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 1

# 4. Create widget that fails gpu existence (no gpu label)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-fail-gpu-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"green\",\"size\":3}}"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify only mixed-match arrived
echo "=== Client received ==="
cat /tmp/watch-label-mixed.log
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-label-mixed.log
```

**Expected results:**
- Only `mixed-match-$TEST_RUN` event is delivered
- `mixed-fail-exp-$TEST_RUN` and `mixed-fail-gpu-$TEST_RUN` are filtered out

---

## Test 18: List with fieldSelector — filtered results

**Goal:** Verify that `?fieldSelector=metadata.name=<value>` on a non-watch list request returns only matching objects.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# 2. Create multiple widgets
for name in "list-field-foo" "list-field-bar" "list-field-baz"; do
  curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$name-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

# 3. List with fieldSelector=metadata.name=list-field-foo
echo "=== List with fieldSelector ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.name=list-field-foo-$TEST_RUN" | python3 -m json.tool

# 4. Verify only one item returned
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.name=list-field-foo-$TEST_RUN" | python3 -c "import sys,json; print(f\"Items: {len(json.load(sys.stdin)['items'])}\")"
```

**Expected results:**
- List returns exactly 1 item with name `list-field-foo-$TEST_RUN`
- `list-field-bar-$TEST_RUN` and `list-field-baz-$TEST_RUN` are not in results

---

## Test 19: List with labelSelector — filtered results

**Goal:** Verify that `?labelSelector=app=nginx` on a non-watch list request returns only matching objects.

```bash
# 1. Create widgets with different labels
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-nginx-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-apache-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-none-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

# 2. List with labelSelector=app=nginx
echo "=== List with labelSelector ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?labelSelector=app=nginx" | python3 -m json.tool

# 3. Verify only nginx widget returned
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?labelSelector=app=nginx" | python3 -c "import sys,json; items=json.load(sys.stdin)['items']; print(f\"Items: {len(items)}\"); [print(f\"  - {i['metadata']['name']}\") for i in items]"
```

**Expected results:**
- List returns exactly 1 item: `list-label-nginx-$TEST_RUN`
- Other widgets are not in results

---

## Test 20: List with both fieldSelector and labelSelector

**Goal:** Verify that both selectors are applied together on list requests.

```bash
# 1. Create widgets
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-other-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN-nolabel\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

# 2. List with both selectors
echo "=== List with both selectors ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.name=list-both-target-$TEST_RUN&labelSelector=app=nginx" | python3 -m json.tool

# 3. Verify only one item returned (matches both name AND label)
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.name=list-both-target-$TEST_RUN&labelSelector=app=nginx" | python3 -c "import sys,json; print(f\"Items: {len(json.load(sys.stdin)['items'])}\")"
```

**Expected results:**
- List returns exactly 1 item: `list-both-target-$TEST_RUN`
- `list-both-other-$TEST_RUN` (wrong name) and `list-both-target-$TEST_RUN-nolabel` (no label) are excluded

---

## Test 21: List with filter and pagination

**Goal:** Verify that filtering happens before pagination (correct page sizes). Uses a unique label (`pag-test-run`) to avoid counting objects from earlier tests.

```bash
# 1. Create 10 widgets, only 3 have the target label
for i in $(seq 1 10); do
  if [ $i -le 3 ]; then
    labels='{"pag-test-run":"true"}'
  else
    labels='{}'
  fi
  curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"list-pag-$(printf '%02d' $i)-$TEST_RUN\",\"labels\":$labels},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

# 2. Filter to 3, limit 10 → should return 3 (not 10)
echo "=== Filter + pagination ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?labelSelector=pag-test-run=true&limit=10" | python3 -c "
import sys, json
body = json.load(sys.stdin)
items = body['items']
print(f\"Items returned: {len(items)}\")
print(f\"Continue token: {body.get('continueToken', 'null')}\")
for i in items:
    print(f\"  - {i['metadata']['name']}\")
"
```

**Expected results:**
- Exactly 3 items returned (not 10)
- No continue token (all matching items fit in the page)

---

## Test 22: List with filter that matches no objects

**Goal:** Verify that a filter matching no objects returns an empty list.

```bash
echo "=== Filter with no matches ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.name=nonexistent-$TEST_RUN" | python3 -c "
import sys, json
body = json.load(sys.stdin)
print(f\"Items: {len(body['items'])}\")
print(f\"Continue token: {body.get('continueToken', 'null')}\")
"
```

**Expected results:**
- Empty items array
- No continue token

---

## Test 23: Watch with combined fieldSelector + labelSelector (AND semantics)

**Goal:** Verify that when both selectors are present on a watch request, they are combined with AND semantics.

```bash
# 1. Start a watch with both selectors
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=watch-combo-target-$TEST_RUN&labelSelector=app=nginx" /tmp/watch-combo.log)
sleep 2

# 2. Create widget matching BOTH selectors (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}"

sleep 1

# 3. Create widget with matching name but wrong label (should NOT be delivered — label mismatch)
# NOTE: Uses a different name to avoid conflict; the fieldSelector filters by name, so
# a different name won't match the fieldSelector anyway. This tests label filtering independently.
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-wrong-label-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

sleep 1

# 4. Create widget with matching label but wrong name (should NOT be delivered — name mismatch)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-other-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify only the matching event arrived
echo "=== Client received ==="
cat /tmp/watch-combo.log
echo "=== Event names ==="
grep -o '"name":"[^"]*"' /tmp/watch-combo.log
```

**Expected results:**
- Only `watch-combo-target-$TEST_RUN` with `app=nginx` label is delivered
- `watch-combo-wrong-label-$TEST_RUN` is filtered out (wrong label)
- `watch-combo-other-$TEST_RUN` is filtered out (wrong name)

---

## Test 24: Invalid fieldSelector on list returns 400

**Goal:** Verify that invalid field selectors on list requests return HTTP 400.

```bash
echo "=== Invalid fieldSelector on list ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget?fieldSelector=metadata.namespace=default"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidFieldSelector"`

---

## Test 25: Status subresource — create object, update status via /status

**Goal:** Verify that a Schema with `statusSchema` enables the `/status` endpoint for reading and updating status.

```bash
# 1. Create an object
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"status-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -m json.tool

# 2. Verify status field is absent on created object
echo "=== Status on created object ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/status-widget-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
status = obj.get('status')
if status is None and 'status' not in obj:
    print('Status: field absent (correct)')
elif status is None:
    print('Status: null')
else:
    print(f'Status: {status}')
"

# 3. Update status via PUT /status
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/status-widget-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\",\"message\":\"All systems go\"}}" | python3 -m json.tool

# 4. GET /status to verify
echo "=== GET /status ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/status-widget-$TEST_RUN/status" | python3 -m json.tool

# 5. GET full object to verify status persisted
echo "=== GET full object ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/status-widget-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f\"Status: {obj['status']}\")
print(f\"Spec: {obj['spec']}\")
"
```

**Expected results:**
- Created object has no `status` field (field is omitted from JSON when status is None)
- PUT /status returns 200 with full `StoredObject` including updated status
- GET /status returns the status value (inline JSON: `{"phase":"Running","message":"All systems go"}`)
- Full object GET shows both spec and status

---

## Test 26: Status subresource not enabled — Schema without statusSchema returns 404

**Goal:** Verify that `/status` endpoints return `StatusSubresourceNotEnabled` for kinds without `statusSchema`.

```bash
# 1. Register a Schema WITHOUT statusSchema
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "test.io",
    "targetVersion": "v1",
    "targetKind": "Gadget",
    "specSchema": {
      "type": "object",
      "properties": {
        "name": { "type": "string" }
      }
    }
  }'

# 2. Create an object
curl -s -X POST http://localhost:8080/apis/test.io/v1/Gadget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-status-gadget-$TEST_RUN\"},\"spec\":{\"name\":\"test\"}}" > /dev/null

# 3. GET /status should return 404
echo "=== GET /status (should be 404) ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/test.io/v1/Gadget/no-status-gadget-$TEST_RUN/status"

# 4. PUT /status should return 404
echo "=== PUT /status (should be 404) ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/test.io/v1/Gadget/no-status-gadget-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}"
```

**Expected results:**
- Both GET and PUT /status return HTTP 404
- Response body contains `"code": "StatusSubresourceNotEnabled"`

---

## Test 27: Status update with invalid data returns 422

**Goal:** Verify that status updates are validated against `statusSchema`.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"invalid-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"}}" > /dev/null

# 2. Update status with invalid type (phase should be string, not integer)
echo "=== Invalid status update ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/invalid-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":123}}"
```

**Expected results:**
- HTTP 422 Unprocessable Entity
- Response body contains `"code": "SchemaValidation"` with validation error details

---

## Test 28: Status update for non-existent object returns 404 NotFound

**Goal:** Verify that updating status on a non-existent object returns `NotFound`.

```bash
# 1. PUT /status for non-existent object
echo "=== Status update for non-existent object ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/nonexistent-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}"
```

**Expected results:**
- HTTP 404 Not Found
- Response body contains `"code": "NotFound"`

---

## Test 29: Status update side effects — spec unchanged and resourceVersion bumped

**Goal:** Verify that a status update leaves spec unchanged and bumps `resourceVersion`.

```bash
# 1. Create object and capture resourceVersion
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"spec-preserve-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "Initial resourceVersion: $INITIAL_RV"

# 2. Update status
STATUS_RV=$(curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/spec-preserve-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "After status update resourceVersion: $STATUS_RV"

# 3. Verify spec unchanged and resourceVersion bumped
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/spec-preserve-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
spec = obj['spec']
status = obj['status']
print(f'Spec color: {spec[\"color\"]}')
print(f'Spec size: {spec[\"size\"]}')
print(f'Status: {status}')
print(f'ResourceVersion: {obj[\"system\"][\"resourceVersion\"]}')
assert spec['color'] == 'blue', 'spec.color should be unchanged'
assert spec['size'] == 10, 'spec.size should be unchanged'
assert obj['system']['resourceVersion'] > $INITIAL_RV, 'resourceVersion should be bumped'
print('PASS: spec unchanged, status set, resourceVersion bumped')
"
```

**Expected results:**
- `spec.color` is still `"blue"`, `spec.size` is still `10`
- `status` is `{"phase":"Running"}`
- `resourceVersion` after status update is greater than initial `resourceVersion`

---

## Test 30: Create object with unknown top-level field — rejected with 400

**Goal:** Verify that unknown top-level fields in the create request body are rejected with 400 Bad Request.

```bash
# 1. Create object with unknown top-level field "status"
echo "=== Create with unknown field ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"create-with-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1},\"status\":{\"phase\":\"Pre-set\"}}"
```

**Expected results:**
- HTTP 400 Bad Request
- Response body contains `"code": "InvalidRequestBody"`
- Error message mentions unknown field(s)

---

## Test 31: Status update replaces status (not merged)

**Goal:** Verify that status updates completely replace the status field, not merge.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"replace-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

# 2. Set status with both phase and message
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\",\"message\":\"initial message\"}}" > /dev/null

# 3. Update status with only phase (no message)
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Completed\"}}" > /dev/null

# 4. Verify message is gone (replaced, not merged)
echo "=== Verify status replaced ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/replace-status-$TEST_RUN/status" | python3 -c "
import sys, json
status = json.load(sys.stdin)
print(f'Status: {status}')
assert status.get('phase') == 'Completed', f'phase should be Completed, got {status.get(\"phase\")}'
assert 'message' not in status, f'message should be removed, but got: {status.get(\"message\")}'
print('PASS: status replaced, not merged')
"
```

**Expected results:**
- After second update, `status` contains only `{"phase":"Completed"}` — `message` is gone

---

## Test 32: Event types for status vs spec

**Goal:** Verify that status updates publish `StatusModified` events while spec updates publish `Modified` events.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"event-types-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

# 2. Start watching
WATCH_PID=$(start_watch "?watch=true" /tmp/watch-event-types.log)
sleep 2

# 3. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/event-types-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null

sleep 1

# 4. Update spec
get_system_fields "event-types-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/event-types-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"event-types-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify both event types
echo "=== Watch events ==="
cat /tmp/watch-event-types.log

echo "=== Event types ==="
grep -o '"eventType":"[^"]*"' /tmp/watch-event-types.log
```

**Expected results:**
- Watch log contains both `"eventType":"StatusModified"` (for status update) and `"eventType":"Modified"` (for spec update)
- No `"eventType":"Modified"` for the status update
- No `"eventType":"StatusModified"` for the spec update

---

## Test 33: Generation — starts at 1 on create

**Goal:** Verify that newly created objects have `generation: 1`.

```bash
# 1. Create an object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-create-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")

echo "=== Create response ==="
echo "$CREATE_RESP" | python3 -m json.tool

# 2. Verify generation is 1
echo "$CREATE_RESP" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
gen = obj['system']['generation']
rv = obj['system']['resourceVersion']
print(f'generation: {gen}')
print(f'resourceVersion: {rv}')
assert gen == 1, f'generation should be 1 on create, got {gen}'
print('PASS: generation starts at 1')
"
```

**Expected results:**
- `system.generation` equals `1`
- `system.resourceVersion` equals `1`

---

## Test 34: Generation — metadata-only update does NOT bump generation

**Goal:** Verify that updating only labels (no spec change) increments `resourceVersion` but leaves `generation` unchanged.

```bash
# 1. Create object with labels
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_GEN=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
get_system_fields "gen-meta-$TEST_RUN"

echo "Initial: generation=$INITIAL_GEN, resourceVersion=$INITIAL_RV"

# 2. Update with same spec but different labels (remove app, add env)
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$INITIAL_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  }" > /tmp/gen-meta-update.json

echo "=== After metadata-only update ==="
cat /tmp/gen-meta-update.json | python3 -m json.tool

# 3. Verify generation unchanged, resourceVersion bumped
cat /tmp/gen-meta-update.json | python3 -c "
import sys, json
obj = json.load(sys.stdin)
gen = obj['system']['generation']
rv = obj['system']['resourceVersion']
print(f'generation: {gen}')
print(f'resourceVersion: {rv}')
assert gen == $INITIAL_GEN, f'generation should stay {INITIAL_GEN}, got {gen}'
assert rv > $INITIAL_RV, f'resourceVersion should bump, {rv} > {INITIAL_RV}'
print('PASS: generation unchanged, resourceVersion bumped')
"
```

**Expected results:**
- `generation` stays at `1` (unchanged)
- `resourceVersion` increments (e.g., `1` → `2`)
- Labels changed from `{"app":"nginx"}` to `{"env":"prod"}`

---

## Test 35: Generation — spec change bumps generation

**Goal:** Verify that updating the spec increments both `generation` and `resourceVersion`.

```bash
# 1. Get current state
CURRENT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT"  | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
get_system_fields "gen-meta-$TEST_RUN"

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

# 2. Update spec (change color)
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$BEFORE_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":10}}
  }" > /tmp/gen-spec-update.json

echo "=== After spec update ==="
cat /tmp/gen-spec-update.json | python3 -m json.tool

# 3. Verify both counters bumped
cat /tmp/gen-spec-update.json | python3 -c "
import sys, json
obj = json.load(sys.stdin)
gen = obj['system']['generation']
rv = obj['system']['resourceVersion']
print(f'generation: {gen}')
print(f'resourceVersion: {rv}')
assert gen == $BEFORE_GEN + 1, f'generation should bump to {$BEFORE_GEN + 1}, got {gen}'
assert rv > $BEFORE_RV, f'resourceVersion should bump, {rv} > {BEFORE_RV}'
print('PASS: both generation and resourceVersion bumped')
"
```

**Expected results:**
- `generation` increments (e.g., `1` → `2`)
- `resourceVersion` increments
- Spec changed from `{"color":"blue","size":10}` to `{"color":"red","size":10}`

---

## Test 36: Generation — status update does NOT bump generation

**Goal:** Verify that updating status via `/status` increments `resourceVersion` but leaves `generation` unchanged.

```bash
# 1. Get current state
CURRENT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

# 2. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /tmp/gen-status-update.json

echo "=== After status update ==="
cat /tmp/gen-status-update.json | python3 -m json.tool

# 3. Verify generation unchanged, resourceVersion bumped
cat /tmp/gen-status-update.json | python3 -c "
import sys, json
obj = json.load(sys.stdin)
gen = obj['system']['generation']
rv = obj['system']['resourceVersion']
print(f'generation: {gen}')
print(f'resourceVersion: {rv}')
assert gen == $BEFORE_GEN, f'generation should stay {BEFORE_GEN}, got {gen}'
assert rv > $BEFORE_RV, f'resourceVersion should bump, {rv} > {BEFORE_RV}'
print('PASS: generation unchanged, resourceVersion bumped on status update')
"
```

**Expected results:**
- `generation` stays unchanged
- `resourceVersion` increments
- Status set to `{"phase":"Running"}`

---

## Test 37: Generation — generation and resourceVersion are independent counters

**Goal:** Verify that after a sequence of mixed updates, `generation` reflects only spec changes while `resourceVersion` reflects all changes.

```bash
# 1. Start fresh — create a new object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
echo "=== Step 0: CREATE ==="
echo "$CREATE_RESP" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 2. Update labels only (metadata change)
get_system_fields "gen-indep-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  }" > /dev/null
echo "=== Step 1: UPDATE labels only ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 3. Update spec
get_system_fields "gen-indep-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null
echo "=== Step 2: UPDATE spec ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 4. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null
echo "=== Step 3: UPDATE status ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 5. Update labels again (metadata change)
get_system_fields "gen-indep-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"httpd\",\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null
echo "=== Step 4: UPDATE labels again ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
gen = obj['system']['generation']
rv = obj['system']['resourceVersion']
print(f'  generation={gen}, resourceVersion={rv}')
assert gen == 2, f'generation should be 2 (only 1 spec change), got {gen}'
assert rv == 5, f'resourceVersion should be 5 (create + 4 updates), got {rv}'
print('PASS: generation=2 (spec-only), resourceVersion=5 (all changes)')
"
```

**Expected results:**

| Step | Action | generation | resourceVersion |
|------|--------|------------|-----------------|
| 0 | CREATE | 1 | 1 |
| 1 | UPDATE labels only | 1 | 2 |
| 2 | UPDATE spec | 2 | 3 |
| 3 | UPDATE status | 2 | 4 |
| 4 | UPDATE labels again | 2 | 5 |

Final assertion: `generation == 2`, `resourceVersion == 5`


---

## Test 38: Annotations — create with/without annotations

**Goal:** Verify that `metadata.annotations` are persisted when provided on create, and default to `{}` when omitted.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# Each entry: <name-suffix>|<annotations-json>
CASES=(
  "with-ann|{\"description\":\"my widget\",\"owner\":\"team-platform\"}"
  "without-ann|null"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix annotations <<< "$case"
  echo "=== Case: $suffix ==="
  if [ "$annotations" = "null" ]; then
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{\"metadata\":{\"name\":\"${suffix}-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -m json.tool
  else
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{
        \"metadata\": {
          \"name\": \"${suffix}-widget-$TEST_RUN\",
          \"annotations\": $annotations
        },
        \"spec\": {
          \"color\": \"blue\",
          \"size\": 10
        }
      }" | python3 -m json.tool
  fi
  echo
done

# 2. Verify via GET
echo "=== GET annotations ==="
for suffix in "with-ann" "without-ann"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${suffix}-widget-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'\${suffix}: annotations={obj[\"metadata\"].get(\"annotations\", {})}')
"
done
```

**Expected results:**
- `with-ann` case: Create and GET responses contain `"annotations": { "description": "my widget", "owner": "team-platform" }`
- `without-ann` case: Create and GET responses contain `"annotations": {}`

---

## Test 39: Annotations — update with changed annotations

**Goal:** Verify that updating annotations applies full replacement semantics.

```bash
# 1. Get the current annotated widget
echo "=== Fetch current state ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-ann-widget-$TEST_RUN" | python3 -m json.tool

# 2. Update: change "description" and add "owner"
get_system_fields "with-ann-widget-$TEST_RUN"

echo "=== Update with changed annotations ==="
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-ann-widget-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": {\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\": {
      \"name\": \"with-ann-widget-$TEST_RUN\",
      \"annotations\": { \"description\": \"new widget\", \"owner\": \"team\" }
    },
    \"system\": {\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\": {\"color\":\"blue\",\"size\":10}}
  }" | python3 -m json.tool

# 3. GET and verify annotations changed
echo "=== GET after update ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/with-ann-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- `"description"` changed from `"my widget"` to `"new widget"`
- `"owner"` changed from `"team-platform"` to `"team"`
- `resourceVersion` incremented

---

## Test 40: Annotation validation matrix — invalid key, empty key, and size limit

**Goal:** Verify that malformed `metadata.annotations` are rejected with HTTP 400 and `code: InvalidAnnotation`.  
**Note:** Unlike labels, annotation **values** have no individual length limit — only the total serialized annotations size is capped at 256KB. Therefore this test covers empty keys, keys exceeding the 256-character limit, and total size exceeding 256KB.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# 2. Run the cases
# Use python3 to generate valid JSON with a long key (shell string interpolation
# would produce invalid JSON from a bare 257-char string)
LONG_KEY_JSON=$(python3 -c "import json; k='a'*257; print(json.dumps({k: 'value'}))")

# Each entry: <name-suffix>|<annotations-json>|<expected-fragment-in-error>
CASES=(
  "empty-key|{\"\":\"value\"}|empty"
  "long-key|$LONG_KEY_JSON|256"
  "size-limit|$(python3 -c "import json; print(json.dumps({'key': 'x' * 262145}))")|256"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix annotations expected <<< "$case"
  echo "=== Case: $suffix ==="
  curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{
      \"metadata\": { \"name\": \"ann-${suffix}-$TEST_RUN\", \"annotations\": $annotations },
      \"spec\":     { \"color\": \"blue\", \"size\": 1 }
    }"
  echo
done
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidAnnotation"`
- Error message mentions the relevant rule:
  - `empty-key` → empty key
  - `long-key` → maximum length of 256
  - `size-limit` → maximum total size of 256KB

---

## Test 41: Finalizers — create with/without finalizers

**Goal:** Verify that `metadata.finalizers` are persisted when provided on create, and default to `[]` when omitted.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# Each entry: <name-suffix>|<finalizers-json>
CASES=(
  "with-fin|[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]"
  "without-fin|null"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix finalizers <<< "$case"
  echo "=== Case: $suffix ==="
  if [ "$finalizers" = "null" ]; then
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{\"metadata\":{\"name\":\"${suffix}-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
f = obj['metadata'].get('finalizers', None)
print(f'finalizers: {f}')
assert f == [], f'Expected finalizers=[], got {f}'
print('PASS: finalizers=[] on create')
"
  else
    curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
      -H "Content-Type: application/json" \
      -d "{
        \"metadata\": {
          \"name\": \"${suffix}-$TEST_RUN\",
          \"finalizers\": $finalizers
        },
        \"spec\": {
          \"color\": \"blue\",
          \"size\": 10
        }
      }" | python3 -m json.tool
  fi
  echo
done

# 3. Verify via GET
echo "=== GET finalizers ==="
for suffix in "with-fin" "without-fin"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${suffix}-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
f = obj['metadata'].get('finalizers', [])
print(f'\${suffix}: finalizers={f}')
"
done
```

**Expected results:**
- `with-fin` case: Create and GET responses contain `"finalizers": ["example.io.$TEST_RUN/cleanup", "kapi.io/finalizer"]`
- `without-fin` case: Create and GET responses contain `"finalizers": []`

---

## Test 42: DELETE with/without finalizers

**Goal:** Verify that deleting an object with finalizers marks it for deletion (sets `deletionTimestamp`), while deleting without finalizers performs a hard delete.

```bash
# 1. Create a widget with finalizers
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-del-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup"] },
    "spec": { "color": "blue", "size": 10 }
  }' > /dev/null

# 2. DELETE it (mark for deletion)
echo "=== Case: DELETE with finalizers ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-del-$TEST_RUN" | python3 -c "
import sys, json
data = sys.stdin.read()
body, status = data.rsplit('\nHTTP_STATUS: ', 1)
obj = json.loads(body)
print(f'Status: {status}')
print(f'deletionTimestamp: {obj[\"system\"].get(\"deletionTimestamp\", \"NOT SET\")}')
assert status == '200', f'Expected 200, got {status}'
assert 'deletionTimestamp' in obj['system'], 'deletionTimestamp should be set'
print('PASS: object marked for deletion, still exists')
"
echo

# 3. Create a widget without finalizers
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-harddel-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

# 4. DELETE it (hard delete)
echo "=== Case: DELETE without finalizers ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-harddel-$TEST_RUN"
echo

# 5. Verify it no longer exists
echo "=== GET after DELETE without finalizers ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-harddel-$TEST_RUN"
```

**Expected results:**
- Case 1 (with finalizers): DELETE returns HTTP 200, `system.deletionTimestamp` set, GET still returns the object
- Case 2 (without finalizers): DELETE returns HTTP 200, GET returns HTTP 404 (object gone)

---

## Test 43: Idempotent DELETE on already-deleting object

**Goal:** Verify that deleting an already-deleting object returns 200 and `deletionTimestamp` is unchanged.

```bash
# 1. Create a widget with finalizers
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-idempotent-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup"] },
    "spec": { "color": "green", "size": 3 }
  }' > /dev/null

# 2. First DELETE — marks for deletion
FIRST_DEL=$(curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-idempotent-$TEST_RUN")
FIRST_DT=$(echo "$FIRST_DEL" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['deletionTimestamp'])")
echo "First deletionTimestamp: $FIRST_DT"

# 3. Second DELETE — should be idempotent
echo "=== Second DELETE ==="
SECOND_DEL=$(curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-idempotent-$TEST_RUN")
echo "$SECOND_DEL"
SECOND_DT=$(echo "$SECOND_DEL" | python3 -c "
import sys, json
data = sys.stdin.read()
body, status = data.rsplit('\nHTTP_STATUS: ', 1)
print(json.loads(body)['system']['deletionTimestamp'])
")
echo "Second deletionTimestamp: $SECOND_DT"

# 4. Verify timestamps match
python3 -c "
assert '$FIRST_DT' == '$SECOND_DT', f'deletionTimestamp changed: $FIRST_DT vs $SECOND_DT'
print('PASS: deletionTimestamp unchanged on second DELETE')
"
```

**Expected results:**
- Both DELETE requests return HTTP 200
- `deletionTimestamp` has the same value in both responses

---

## Test 44: UPDATE on deleting objects

**Goal:** Verify the three different UPDATE behaviors on deleting objects: spec update rejected, finalizer removal allowed, and empty finalizers trigger hard delete.

```bash
# 1. Create a widget with two finalizers (shared across cases)
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-update-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup", "kapi.io/finalizer"] },
    "spec": { "color": "blue", "size": 10 }
  }' > /dev/null

# 2. DELETE it (mark for deletion)
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" > /dev/null
sleep 1

get_system_fields "fin-update-$TEST_RUN"

# 3. Case 1: UPDATE spec on deleting object (rejected)
echo "=== Case 1: UPDATE spec (should be rejected) ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  "
echo

# 4. Case 2: UPDATE finalizers on deleting object (allowed)
echo "=== Case 2: UPDATE finalizers (should succeed) ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  " | python3 -c "
import sys, json
body = json.loads(sys.stdin.read())
print(f'Status: {body.get(\"code\", \"200\")}')
print(f'finalizers: {body[\"metadata\"][\"finalizers\"]}')
print(f'deletionTimestamp: {body[\"system\"].get(\"deletionTimestamp\", \"NOT SET\")}')
assert body['metadata']['finalizers'] == ['example.io.$TEST_RUN/cleanup'], 'finalizers not updated'
assert 'deletionTimestamp' in body['system'], 'deletionTimestamp should still be set'
print('PASS: finalizers updated, deletionTimestamp preserved')
"
echo

# 5. Case 3: UPDATE to empty finalizers triggers hard delete
get_system_fields "fin-update-$TEST_RUN"
echo "=== Case 3: UPDATE to empty finalizers (triggers hard delete) ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[]},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  "
echo
# Verify object no longer exists
echo "=== GET after hard delete ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN"
```

**Expected results:**
- Case 1: HTTP 409 Conflict with `"code": "ObjectBeingDeleted"` — spec NOT updated
- Case 2: HTTP 200 — finalizers updated to `["example.io.$TEST_RUN/cleanup"]`, `deletionTimestamp` still set
- Case 3: HTTP 200 — object hard-deleted, subsequent GET returns 404

---

## Test 45: Finalizer validation

**Goal:** Verify that finalizer names with invalid characters, exceeding the 256-character limit, or exceeding the maximum count of 20 are rejected with 400.

```bash
# 1. Register the Widget schema (idempotent)
register_widget_schema

# 2. Run the cases
# Use python3 to generate valid JSON with long names / many names
LONG_FINALIZER_JSON=$(python3 -c "import json; n='a'*257; print(json.dumps([n]))")
MANY_FINALIZERS=$(python3 -c "
import json
finalizers = [f'fin-{i}.example.io.$TEST_RUN/cleanup' for i in range(21)]
print(json.dumps(finalizers))
")

# Each entry: <name-suffix>|<finalizers-json>|<expected-fragment-in-error>
CASES=(
  "invalid-chars|[\"invalid name with spaces\"]|invalid characters"
  "long-name|$LONG_FINALIZER_JSON|256"
  "too-many|$MANY_FINALIZERS|max 20"
)

for case in "${CASES[@]}"; do
  IFS='|' read -r suffix finalizers expected <<< "$case"
  echo "=== Case: $suffix ==="
  curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{
      \"metadata\": { \"name\": \"fin-${suffix}-$TEST_RUN\", \"finalizers\": $finalizers },
      \"spec\": { \"color\": \"blue\", \"size\": 1 }
    }"
  echo
done
```

**Expected results (all cases):**
- HTTP 400 Bad Request
- Response body contains `"code": "InvalidFinalizer"`
- Error message mentions the relevant rule:
  - `invalid-chars` → invalid characters in name
  - `long-name` → maximum length of 256
  - `too-many` → max 20

---

## Test 46: Watch events for finalizer lifecycle

**Goal:** Verify that the finalizer lifecycle produces the correct watch events: Added → Modified (deletionTimestamp set) → Deleted (hard delete after finalizers removed).

```bash
# 1. Start a watch-all
WATCH_PID=$(start_watch "?watch=true" /tmp/watch-finalizer-lifecycle.log)
sleep 2

# 2. Create object with finalizers
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-watch-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup"] },
    "spec": { "color": "blue", "size": 10 }
  }' > /dev/null

sleep 1

# 3. DELETE it (mark for deletion → Modified event)
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-watch-$TEST_RUN" > /dev/null

sleep 1

# 4. UPDATE to remove finalizers (triggers hard delete → Deleted event)
get_system_fields "fin-watch-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-watch-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"fin-watch-$TEST_RUN\",\"finalizers\":[]},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  " > /dev/null

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify event types
echo "=== Watch events ==="
cat /tmp/watch-finalizer-lifecycle.log

echo "=== Event types ==="
grep -o '"eventType":"[^"]*"' /tmp/watch-finalizer-lifecycle.log
```

**Expected results:**
- Watch log contains three events: `Added`, `Modified` (deletionTimestamp set), `Deleted` (hard delete)
- All three events reference `fin-watch-$TEST_RUN`
- The Modified event has `deletionTimestamp` set in the object

---

## Test 47: CREATE same-name after DELETE-with-finalizers

**Goal:** Verify that creating an object with the same name as a deleting (not yet deleted) object is rejected.

```bash
# 1. Create a widget with finalizers
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-recreate-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup"] },
    "spec": { "color": "blue", "size": 10 }
  }' > /dev/null

# 2. DELETE it (mark for deletion)
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-recreate-$TEST_RUN" > /dev/null

sleep 1

# 3. Try to CREATE with same name
echo "=== CREATE same name while deleting ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": { \"name\": \"fin-recreate-$TEST_RUN\", \"finalizers\": [\"other.io/finalizer\"] },
    \"spec\": { \"color\": \"red\", \"size\": 20 }
  }"
```

**Expected results:**
- HTTP 409 Conflict
- Response body contains `"code": "AlreadyExists"`

---

## Test 48: UPDATE adds finalizer on deleting object (rejected)

**Goal:** Verify that adding a new finalizer to a deleting object is rejected.

```bash
# 1. Create a widget with one finalizer
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": { "name": "fin-add-'"$TEST_RUN"'", "finalizers": ["example.io.$TEST_RUN/cleanup"] },
    "spec": { "color": "blue", "size": 10 }
  }' > /dev/null

# 2. DELETE it (mark for deletion)
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-add-$TEST_RUN" > /dev/null

sleep 1

# 3. Try to UPDATE adding a finalizer
get_system_fields "fin-add-$TEST_RUN"

echo "=== UPDATE add finalizer on deleting object ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-add-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"fin-add-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/new\"]},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  "
```

**Expected results:**
- HTTP 409 Conflict
- Response body contains `"code": "ObjectBeingDeleted"`
- Finalizers are NOT updated (new finalizer not added)

---

## Test 49: SQLite persistence with finalizers

**Goal:** Verify that finalizers and deletionTimestamp survive a server restart with SQLite storage.

```bash
# 1. Stop the in-memory server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1

# 2. Start a fresh server with SQLite storage
PERSIST_DB=/tmp/kapi-persist-fin.db
rm -f "$PERSIST_DB"
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run > /tmp/kapi-server-persist-fin.log 2>&1 &
sleep 3

# 3. Register the Widget schema (idempotent)
register_widget_schema

# 4. Create object with finalizers
FIN_NAME="persist-fin-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": { \"name\": \"$FIN_NAME\", \"finalizers\": [\"example.io.$TEST_RUN/cleanup\", \"kapi.io/finalizer\"] },
    \"spec\": { \"color\": \"blue\", \"size\": 10 }
  }" > /dev/null

# 5. DELETE it (mark for deletion)
echo "=== DELETE (mark for deletion) ==="
curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$FIN_NAME" > /dev/null

# 6. Snapshot state before restart
echo "=== Before restart ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$FIN_NAME" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
md = obj['metadata']
sys = obj['system']
print(f'finalizers: {md.get(\"finalizers\", [])}')
print(f'deletionTimestamp: {sys.get(\"deletionTimestamp\", \"NOT SET\")}')
"

# Save state for comparison
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$FIN_NAME" > /tmp/kapi-fin-before.json

# 7. Stop the server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 2

# 8. Restart with the same database
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run > /tmp/kapi-server-persist-fin.log 2>&1 &
sleep 3

# 9. Verify state after restart
echo "=== After restart ==="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$FIN_NAME" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
md = obj['metadata']
sys = obj['system']
print(f'finalizers: {md.get(\"finalizers\", [])}')
print(f'deletionTimestamp: {sys.get(\"deletionTimestamp\", \"NOT SET\")}')
"

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/$FIN_NAME" > /tmp/kapi-fin-after.json

# 10. Compare
echo "=== Comparison ==="
python3 -c "
import json
with open('/tmp/kapi-fin-before.json') as f: before = json.load(f)
with open('/tmp/kapi-fin-after.json')  as f: after  = json.load(f)
bf = before['metadata'].get('finalizers', [])
af = after['metadata'].get('finalizers', [])
bd = before['system'].get('deletionTimestamp')
ad = after['system'].get('deletionTimestamp')
print(f'finalizers before: {bf}')
print(f'finalizers after:  {af}')
print(f'deletionTimestamp before: {bd}')
print(f'deletionTimestamp after:  {ad}')
assert bf == af, f'finalizers differ: {bf} vs {af}'
assert bd == ad, f'deletionTimestamp differ: {bd} vs {ad}'
print('PASS: finalizers and deletionTimestamp survived restart')
"
```

**Expected results:**
- Before restart: finalizers present, `deletionTimestamp` set
- After restart: same finalizers, same `deletionTimestamp`
- Final line: `PASS: finalizers and deletionTimestamp survived restart`

---

## Test 50: Concurrent spec and status updates don't conflict

**Goal:** Verify that updating spec and status concurrently doesn't cause conflicts (OCC on spec, no OCC on status).

```bash
# 1. Create object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"concurrent-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

# 2. Start a watch to capture events
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=concurrent-$TEST_RUN" /tmp/watch-concurrent.log)
sleep 2

# 3. Update status (no OCC required)
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/concurrent-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null

# 4. Update spec using INITIAL_RV (should succeed even though status update changed RV)
get_system_fields "concurrent-$TEST_RUN"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/concurrent-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"concurrent-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }"

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify both updates succeeded
echo "=== Watch events ==="
cat /tmp/watch-concurrent.log

echo "=== Event types ==="
grep -o '"eventType":"[^"]*"' /tmp/watch-concurrent.log
```

**Expected results:**
- Both updates succeed (spec update returns 200)
- Watch log contains both `StatusModified` and `Modified` events
- No conflict errors

---

## Test 51: Failed operations don't publish events

**Goal:** Verify that failed operations (e.g., OCC mismatch) don't publish watch events.

```bash
# 1. Create object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-event-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")

# 2. Start a watch
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=no-event-$TEST_RUN" /tmp/watch-no-event.log)
sleep 2

# 3. Attempt update with WRONG resourceVersion (should fail with 409)
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/no-event-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"no-event-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":99999,\"createdAt\":\"2026-01-01T00:00:00Z\",\"updatedAt\":\"2026-01-01T00:00:00Z\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }"

sleep 2

# 4. Kill the watch
kill $WATCH_PID 2>/dev/null

# 5. Verify NO events were published for the failed update
echo "=== Watch events (should only have Added) ==="
cat /tmp/watch-no-event.log

echo "=== Event count ==="
grep -c '"eventType"' /tmp/watch-no-event.log
```

**Expected results:**
- Failed update returns HTTP 409 Conflict
- Watch log contains only the `Added` event from creation
- No `Modified` event for the failed update
- Event count is 1


---

## Cleanup

```bash
# Stop the server
kill $(lsof -ti :8080) 2>/dev/null || true

# Clear the KAPI_DB_PATH so the next run starts in-memory by default
unset KAPI_DB_PATH

# Remove Test 10's hard-coded persistence objects from the database
# (only needed if you want to re-run Test 10 against the same db file)
# curl -s -X DELETE http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/persist-labels-widget
# curl -s -X DELETE http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/persist-ann-widget

# Clean up temp files
rm -f /tmp/watch-*.log
rm -f /tmp/kapi-server.log /tmp/kapi-server-persist*.log
rm -f /tmp/kapi-labels-*.json /tmp/kapi-ann-*.json /tmp/kapi-fin-*.json
rm -f /tmp/kapi-persist-*.db /tmp/kapi-test.db
rm -f ./kapi.db
```

---

## Test Index (full)

| # | Goal |
|---|------|
| 1 | Watch with fieldSelector — matching event delivered, non-matching filtered |
| 2 | Watch all events — Added, Modified, Deleted |
| 3 | Abrupt connection cleanup — dead watcher removed on next publish |
| 4 | Simultaneous watches — fieldSelector + all events |
| 5 | Labels — create with/without labels |
| 6 | Labels — update with changed labels (replace semantics) |
| 7 | Schema with labels and annotations |
| 8 | Label validation matrix — invalid key, value, and length limits |
| 9 | List returns labels and annotations |
| 10 | SQLite persistence — labels and annotations survive restart |
| 11 | Watch with labelSelector equality — matching event delivered |
| 12 | Watch with labelSelector AND combinator — multiple requirements |
| 13 | Watch with labelSelector non-existence (!key) |
| 14 | Watch with labelSelector inequality (key!=value) |
| 15 | Invalid labelSelector returns 400 — watch and list contexts |
| 16 | Empty labelSelector matches all events |
| 17 | Mixed label selector operators |
| 18 | List with fieldSelector — filtered results |
| 19 | List with labelSelector — filtered results |
| 20 | List with both fieldSelector and labelSelector |
| 21 | List with filter and pagination |
| 22 | List with filter that matches no objects |
| 23 | Watch with combined fieldSelector + labelSelector (AND semantics) |
| 24 | Invalid fieldSelector on list returns 400 |
| 25 | Status subresource — create object, update status via /status |
| 26 | Status subresource not enabled — Schema without statusSchema returns 404 |
| 27 | Status update with invalid data returns 422 |
| 28 | Status update for non-existent object returns 404 NotFound |
| 29 | Status update side effects — spec unchanged and resourceVersion bumped |
| 30 | Create object with unknown top-level field — rejected with 400 |
| 31 | Status update replaces status (not merged) |
| 32 | Event types for status vs spec |
| 33 | Generation — starts at 1 on create |
| 34 | Generation — metadata-only update does NOT bump generation |
| 35 | Generation — spec change bumps generation |
| 36 | Generation — status update does NOT bump generation |
| 37 | Generation — generation and resourceVersion are independent counters |
| 38 | Annotations — create with/without annotations |
| 39 | Annotations — update with changed annotations |
| 40 | Annotation validation matrix — invalid key, empty key, and size limit |
| 41 | Finalizers — create with/without finalizers |
| 42 | DELETE with/without finalizers |
| 43 | Idempotent DELETE on already-deleting object |
| 44 | UPDATE on deleting objects |
| 45 | Finalizer validation |
| 46 | Watch events for finalizer lifecycle |
| 47 | CREATE same-name after DELETE-with-finalizers |
| 48 | UPDATE adds finalizer on deleting object (rejected) |
| 49 | SQLite persistence with finalizers |
| 50 | Concurrent spec and status updates don't conflict |
| 51 | Failed operations don't publish events |

---

## Trace Log Reference

| Log Message | Source File | Meaning |
|---|---|---|
| `sse watch stream started` | `src/object/handler.rs` | SSE connection opened |
| `sse watch stream ended` | `src/object/handler.rs` | SSE stream wrapper initialized (logged by `stream::once` before events flow; does **not** mean the connection closed) |
| `watcher subscribed` | `src/event/bus.rs` | Watcher registered in EventBus |
| `event delivered to watcher` | `src/event/bus.rs` | Event matched filter and sent successfully |
| `event filtered out by watcher filter` | `src/event/bus.rs` | Event did not match watcher's fieldSelector or labelSelector |
| `watcher buffer full, removing` | `src/event/bus.rs` | Slow consumer removed (channel full) |
| `watcher channel closed, removing` | `src/event/bus.rs` | Dead watcher removed (client disconnected) |
| `StatusModified` | `src/event/bus.rs` | Emitted when a `/status` subresource update publishes an event (see Tests 32) |

> The label/annotation validation rules themselves (e.g. `InvalidLabel`, `InvalidAnnotation`) are returned as structured error responses, not as separate log lines. To assert *why* a 400 happened, look at the response body — the tests in this doc grep the body, not the log.

---

## Running All Tests at Once

For convenience, you can use the test runner script at `/tmp/kapi_test_v2.sh` (generated from this document). Or create a one-liner that sources sections sequentially:

```bash
# Set up once per session
export KAPI_BASE="http://localhost:8080"
export TEST_RUN=$(date +%s)

# Phase A — in-memory server (Tests 1–40)
#   1–4   : watch basics (fieldSelector, lifecycle, cleanup, concurrent)
#   5–9   : labels & annotations integration
#   10    : SQLite persistence (kills Phase A server, restarts with KAPI_DB_PATH)
#   11–17 : labelSelector on watch
#   18–24 : list filtering
#   15    : labelSelector validation
#   25–32 : status subresource
#   33–37 : generation
#   38–40 : annotations

# Phase B — SQLite server (Test 10 only — kills Phase A's server, restarts with KAPI_DB_PATH)
#   Run Test 10 body, then re-run the Setup to return to the in-memory server.

# Phase C — finalizers (Tests 41–49, reuses Phase A's in-memory server if not interrupted)
#   41–42 : create/delete with finalizers
#   43    : idempotent delete
#   44    : update on deleting objects
#   45    : finalizer validation (name format + length + max count)
#   46    : watch lifecycle
#   47    : create after delete-with-finalizers
#   48    : add finalizer on deleting object (rejected)
#   49    : SQLite persistence (kills Phase A/C server, restarts with KAPI_DB_PATH, then requires re-setup)

# Phase D — concurrent and failure tests (Tests 50–51)
#   50    : concurrent spec/status updates
#   51    : failed operations don't publish events

# Re-run the Setup section's cleanup line, then run Test 10/49's body, then re-run the Setup to
# return to the in-memory server for any post-mortem.
```

---

## Integration Test Binary

The project also includes a Rust integration test binary that tests against both memory and SQLite stores without requiring a running server:

```bash
cargo run --package kapi-tests
cargo test --lib
```

These cover additional scenarios that are awkward to express as curl, including optimistic concurrency at the protocol level, schema deletion, validation edge cases, and full CRUD flows across both store backends.

### Coverage overlap (this doc ↔ Rust tests)

| Area | curl tests (this doc) | Rust tests |
|------|-----------------------|------------|
| CRUD basics | scattered | yes |
| Watch semantics | full | partial (event publish) |
| Label / annotation validation | matrix (8, 40) | yes |
| fieldSelector / labelSelector | full | partial |
| Status subresource | full | full |
| Generation | full | full |
| Optimistic concurrency (RV mismatch) | implicit (round-trip) | explicit |
| Schema deletion | — | yes |
| SQLite store behavior | 10, 49 (round-trip) | full |
