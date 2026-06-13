# Kapi End-to-End Test Prompts

Use these prompts to verify kapi's watch semantics, label validation, and persistence by running the server with trace logging and exercising the endpoints with curl.

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
rm -f /tmp/watch-*.log /tmp/kapi-server.log /tmp/kapi-test.db

# Start server with trace logging
RUST_LOG=kapi=trace cargo run > /tmp/kapi-server.log 2>&1 &
sleep 3

# Verify server is up
curl -s http://localhost:8080/apis/kapi.io/v1/Schema
```

> **Re-run safety**: Each test uses `$TEST_RUN` as a suffix in object names (e.g. `target-widget-$TEST_RUN`) so you can re-run the entire suite without restarting the server. Tests that share objects across runs (Test 14, persistence) use the `$TEST_RUN` from the initial run or hard-coded names with explicit cleanup.

---

## Test 1: Watch with fieldSelector — matching event delivered, non-matching filtered

**Goal:** Verify that `?fieldSelector=metadata.name=<value>` only delivers events for the specified name.

```bash
# 1. Register the Widget schema
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "specSchema": {
      "type": "object",
      "properties": {
        "color": { "type": "string" },
        "size": { "type": "integer" }
      },
      "required": ["color"]
    },
    "statusSchema": {
      "type": "object",
      "properties": {
        "phase": { "type": "string" },
        "message": { "type": "string" }
      }
    }
  }'

# 2. Start a watch filtered to "target-widget-$TEST_RUN"
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=target-widget-$TEST_RUN" \
  > /tmp/watch-fieldselector.log 2>&1 &
WATCH_PID=$!
sleep 2

# Verify the watch is still alive
if ! kill -0 $WATCH_PID 2>/dev/null; then echo "ERROR: watch died before events"; fi

# 3. Create a NON-target widget (should NOT be delivered to the watch)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 4. Create the TARGET widget (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-all.log 2>&1 &
WATCH_PID=$!
sleep 1

# 2. Create a widget (expect Added)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":5}}"

sleep 1

# 3. Update the widget (expect Modified)
# Extract resourceVersion, createdAt, updatedAt from the current object
RV=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"spec\":{\"color\":\"yellow\",\"size\":10}
  }"

sleep 1

# 4. Delete the widget (expect Deleted)
curl -s -X DELETE "http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received all three event types
# NOTE: The SSE JSON uses "eventType" (camelCase) as the discriminator field.
echo "=== Client received ==="
cat /tmp/watch-all.log

echo "=== Event counts ==="
grep -c '"eventType":"Added"' /tmp/watch-all.log && echo " Added events"
grep -c '"eventType":"Modified"' /tmp/watch-all.log && echo " Modified events"
grep -c '"eventType":"Deleted"' /tmp/watch-all.log && echo " Deleted events"

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
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-abrupt.log 2>&1 &
WATCH_PID=$!
sleep 1

# 2. Abruptly kill the curl process (simulates network disconnect / SIGKILL)
kill -9 $WATCH_PID 2>/dev/null
echo "Killed watch process (simulating abrupt disconnect)"
sleep 1

# 3. Trigger a publish — this should detect and clean up the dead watcher
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=named-$TEST_RUN" \
  > /tmp/watch-named.log 2>&1 &
NAMED_PID=$!

# 2. Start watch-all
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-sim-all.log 2>&1 &
ALL_PID=$!

sleep 1

# 3. Create "named-$TEST_RUN" — both watchers should receive it
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"named-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":3}}"

sleep 1

# 4. Create "other-$TEST_RUN" — only watch-all should receive it
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 5: Labels — create object with labels, verify in response and GET

**Goal:** Verify that `metadata.labels` are persisted and returned on create and get.

```bash
# 1. Register the Widget schema (no-op if already registered)
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "specSchema": {
      "type": "object",
      "properties": {
        "color": { "type": "string" },
        "size": { "type": "integer" }
      },
      "required": ["color", "size"]
    }
  }' > /dev/null

# 2. Create a widget with labels
echo "=== Create with labels ==="
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"labeled-widget-$TEST_RUN\",
      \"labels\": { \"app\": \"nginx\", \"env\": \"prod\", \"app.kubernetes.io/version\": \"v1.2.3\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 10
    }
  }" | python3 -m json.tool

# 3. GET the widget and verify labels
echo "=== GET labeled widget ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/labeled-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- Create response contains `"labels": { "app": "nginx", "env": "prod", "app.kubernetes.io/version": "v1.2.3" }`
- GET response contains the same labels
- Prefixed key `app.kubernetes.io/version` is accepted

---

## Test 6: Labels — create object without labels, verify empty map

**Goal:** Verify that omitting `metadata.labels` results in `"labels": {}`.

```bash
# 1. Create a widget without labels
echo "=== Create without labels ==="
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-labels-widget-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" | python3 -m json.tool

# 2. GET and verify empty labels
echo "=== GET no-labels widget ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/no-labels-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- Create response contains `"labels": {}`
- GET response contains `"labels": {}`

---

## Test 7: Labels — update with changed labels (replace semantics)

**Goal:** Verify that updating labels applies diff-based changes (add, modify, remove).

```bash
# 1. Get the current labeled widget
echo "=== Fetch current state ==="
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/labeled-widget-$TEST_RUN")
echo "$CURRENT" | python3 -m json.tool

# 2. Update: remove "env", change "app" to "httpd", add "tier" -> "frontend"
RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

echo "=== Update with changed labels ==="
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/labeled-widget-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": {\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\": {
      \"name\": \"labeled-widget-$TEST_RUN\",
      \"labels\": { \"app\": \"httpd\", \"app.kubernetes.io/version\": \"v1.2.3\", \"tier\": \"frontend\" }
    },
    \"system\": {\"resourceVersion\":$RV,\"createdAt\":\"$CREATED\",\"updatedAt\":\"$UPDATED\"},
    \"spec\": {\"color\":\"blue\",\"size\":10}}
  }" | python3 -m json.tool

# 3. GET and verify labels changed
echo "=== GET after update ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/labeled-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- `"env"` label removed (was `"prod"`)
- `"app"` changed from `"nginx"` to `"httpd"`
- `"tier": "frontend"` added
- `"app.kubernetes.io/version": "v1.2.3"` unchanged
- `resourceVersion` incremented

---

## Test 8: Labels — create Schema with labels

**Goal:** Verify that Schema objects support labels.

```bash
# 1. Create a Schema with labels
echo "=== Create Schema with labels ==="
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"labels\": { \"team\": \"platform\", \"status\": \"active\" }
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

# 2. GET the Schema and verify labels
# Schema name format: {targetKind}.{targetGroup}
echo "=== GET Schema with labels ==="
curl -s "http://localhost:8080/apis/kapi.io/v1/Schema/Gadget.test-$TEST_RUN.io" | python3 -m json.tool
```

**Expected results:**
- Create response contains `"labels": { "team": "platform", "status": "active" }`
- GET response contains the same labels

---

## Test 9: Labels — invalid key format returns 400

**Goal:** Verify that invalid label key characters are rejected.

```bash
# 1. Create with invalid key (contains space and !)
echo "=== Invalid label key ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"bad-key-widget-$TEST_RUN\",
      \"labels\": { \"invalid key!\": \"value\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 1
    }
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions invalid characters

---

## Test 10: Labels — invalid value format returns 400

**Goal:** Verify that invalid label value characters are rejected.

```bash
# 1. Create with invalid value (contains space and !)
echo "=== Invalid label value ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"bad-value-widget-$TEST_RUN\",
      \"labels\": { \"app\": \"invalid value!\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 1
    }
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions invalid characters in value

---

## Test 11: Labels — key exceeds length limit returns 400

**Goal:** Verify that label keys exceeding 256 characters are rejected.

```bash
# 1. Generate a 257-char key
LONG_KEY=$(python3 -c "print('a' * 257)")

echo "=== Label key too long ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"long-key-widget-$TEST_RUN\",
      \"labels\": { \"$LONG_KEY\": \"value\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 1
    }
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions maximum length of 256

---

## Test 12: Labels — value exceeds length limit returns 400

**Goal:** Verify that label values exceeding 256 characters are rejected.

```bash
# 1. Generate a 257-char value
LONG_VALUE=$(python3 -c "print('a' * 257)")

echo "=== Label value too long ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"long-value-widget-$TEST_RUN\",
      \"labels\": { \"app\": \"$LONG_VALUE\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 1
    }
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions maximum length of 256

---

## Test 13: Labels — list returns labels for all objects

**Goal:** Verify that list endpoint returns labels for each object.

```bash
# 1. List all widgets
echo "=== List widgets ==="
curl -s http://localhost:8080/apis/example.io/v1/Widget | python3 -m json.tool
```

**Expected results:**
- Each item in `items` array has `metadata.labels` field
- `labeled-widget-$TEST_RUN` has updated labels (`app: httpd`, `tier: frontend`, etc.)
- `no-labels-widget-$TEST_RUN` has `"labels": {}`

---

## Test 14: SQLite persistence survives restart

**Goal:** Verify that labels persist across server restarts via SQLite storage.

```bash
# NOTE: The server now uses the KAPI_DB_PATH env var for database path.
# If unset, it defaults to ./kapi.db. Set it to a temp file for this test.

# 1. Stop the current in-memory server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1

# 2. Start server with SQLite storage
export KAPI_DB_PATH=/tmp/kapi-persist-test.db
rm -f "$KAPI_DB_PATH"
RUST_LOG=kapi=trace cargo run > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

# 3. Register schema and create an object with labels
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "specSchema": {
      "type": "object",
      "properties": { "color": { "type": "string" }, "size": { "type": "integer" } },
      "required": ["color", "size"]
    }
  }' > /dev/null

PERSIST_NAME="persist-widget"

# Create with labels
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{
    \"metadata\": {
      \"name\": \"$PERSIST_NAME\",
      \"labels\": { \"app\": \"nginx\", \"env\": \"prod\", \"app.kubernetes.io/version\": \"v1.2.3\" }
    },
    \"spec\": {
      \"color\": \"blue\",
      \"size\": 10
    }
  }" > /dev/null

# Update labels (remove env, change app, add tier)
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/$PERSIST_NAME")
RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/$PERSIST_NAME" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": {\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\": {
      \"name\": \"$PERSIST_NAME\",
      \"labels\": { \"app\": \"httpd\", \"app.kubernetes.io/version\": \"v1.2.3\", \"tier\": \"frontend\" }
    },
    \"system\": {\"resourceVersion\":$RV,\"createdAt\":\"$CREATED\",\"updatedAt\":\"$UPDATED\"},
    \"spec\": {\"color\":\"blue\",\"size\":10}}
  }" > /dev/null

# 4. Verify labels before restart
echo "=== Before restart ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/$PERSIST_NAME" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
labels = obj['metadata']['labels']
print(f'Labels: {labels}')
# Write labels to a temp file for later comparison
with open('/tmp/kapi-labels-before.json', 'w') as f:
    json.dump(labels, f, sort_keys=True)
"

# 5. Stop the server
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 2

# 6. Restart the server with the same database
RUST_LOG=kapi=trace KAPI_DB_PATH="$KAPI_DB_PATH" cargo run > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

# 7. Verify labels survived restart
echo "=== After restart ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/$PERSIST_NAME" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
labels = obj['metadata']['labels']
print(f'Labels: {labels}')
with open('/tmp/kapi-labels-after.json', 'w') as f:
    json.dump(labels, f, sort_keys=True)
"

# 8. Compare labels semantically (order-independent)
echo "=== Comparison ==="
python3 -c "
import json
with open('/tmp/kapi-labels-before.json') as f:
    before = json.load(f)
with open('/tmp/kapi-labels-after.json') as f:
    after = json.load(f)
print(f'Before: {before}')
print(f'After:  {after}')
assert before == after, f'Labels differ: {before} vs {after}'
print('PASS: Labels survived restart')
"
```

**Expected results:**
- Step 4 (`=== Before restart ===`) shows labels: `app: httpd`, `tier: frontend`, `app.kubernetes.io/version: v1.2.3`
- Step 7 (`=== After restart ===`) shows the same labels
- Step 8 (`=== Comparison ===`) prints `PASS: Labels survived restart`

---

## Test 15: Watch with labelSelector equality — matching event delivered

**Goal:** Verify that `?labelSelector=app=nginx` only delivers events for objects with matching labels.

```bash
# 1. Register the Widget schema (no-op if already registered)
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "specSchema": {
      "type": "object",
      "properties": {
        "color": { "type": "string" },
        "size": { "type": "integer" }
      },
      "required": ["color", "size"]
    }
  }' > /dev/null

# 2. Start a watch filtered by label selector
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx" \
  > /tmp/watch-labelselector.log 2>&1 &
WATCH_PID=$!
sleep 2

# Verify the watch is still alive
if ! kill -0 $WATCH_PID 2>/dev/null; then echo "ERROR: watch died before events"; fi

# 3. Create a widget with NON-matching labels (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-labels-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 4. Create a widget with MATCHING labels (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 16: Watch with labelSelector AND combinator — multiple requirements

**Goal:** Verify that comma-separated label selectors require ALL labels to match.

```bash
# 1. Start a watch with AND combinator: app=nginx AND env=prod
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,env=prod" \
  > /tmp/watch-label-and.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widget with only app=nginx (should NOT be delivered — missing env)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget with only env=prod (should NOT be delivered — missing app)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match2-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"green\",\"size\":2}}"

sleep 1

# 4. Create widget with BOTH labels (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 17: Watch with labelSelector non-existence (!key)

**Goal:** Verify that `?labelSelector=!experimental` delivers events for objects WITHOUT the specified label.

```bash
# 1. Start a watch for objects without the "experimental" label
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=!experimental" \
  > /tmp/watch-label-notexists.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widget WITH experimental label (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"has-experimental-$TEST_RUN\",\"labels\":{\"experimental\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget WITHOUT experimental label (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 18: Watch with labelSelector inequality (key!=value)

**Goal:** Verify that `?labelSelector=env!=prod` delivers events for objects where the label has a different value OR is absent.

```bash
# 1. Start a watch for objects where env is NOT prod
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=env!=prod" \
  > /tmp/watch-label-notequals.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widget with env=prod (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-prod-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget with env=staging (should be delivered — different value)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-staging-$TEST_RUN\",\"labels\":{\"env\":\"staging\"}},\"spec\":{\"color\":\"green\",\"size\":2}}"

sleep 1

# 4. Create widget without env label (should be delivered — absence satisfies inequality)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 19: Invalid labelSelector returns 400

**Goal:** Verify that malformed label selectors are rejected with HTTP 400.

```bash
# 1. Empty value in equality selector
echo "=== Empty value ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=app="

# 2. Empty segment (double comma)
echo "=== Empty segment ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,,env=prod"
```

**Expected results:**
- Both requests return HTTP 400
- Response body contains `"code": "InvalidLabelSelector"`
- Error messages describe the specific issue

> **Note:** `labelSelector` on non-watch list requests is now valid and returns filtered results (not 400). See Test 23.

---

## Test 20: Empty labelSelector matches all events

**Goal:** Verify that `?labelSelector=` (empty string) matches all objects.

```bash
# 1. Start a watch with empty labelSelector
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=" \
  > /tmp/watch-label-empty.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widgets with various labels — all should be delivered
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-sel-1-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 0.5

curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 21: Mixed label selector operators

**Goal:** Verify that a single labelSelector can combine different operator types.

```bash
# 1. Start a watch with mixed operators: app=nginx AND !experimental AND gpu (existence)
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,!experimental,gpu" \
  > /tmp/watch-label-mixed.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widget that matches all three requirements
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}"

sleep 1

# 3. Create widget that fails !experimental (has experimental=true)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-fail-exp-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\",\"experimental\":\"true\"}},\"spec\":{\"color\":\"red\",\"size\":2}}"

sleep 1

# 4. Create widget that fails gpu existence (no gpu label)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 22: List with fieldSelector — filtered results

**Goal:** Verify that `?fieldSelector=metadata.name=<value>` on a non-watch list request returns only matching objects.

```bash
# 1. Register the Widget schema (no-op if already registered)
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "specSchema": {
      "type": "object",
      "properties": {
        "color": { "type": "string" },
        "size": { "type": "integer" }
      },
      "required": ["color", "size"]
    }
  }' > /dev/null

# 2. Create multiple widgets
for name in "list-field-foo" "list-field-bar" "list-field-baz"; do
  curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$name-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

# 3. List with fieldSelector=metadata.name=list-field-foo
echo "=== List with fieldSelector ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.name=list-field-foo-$TEST_RUN" | python3 -m json.tool

# 4. Verify only one item returned
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.name=list-field-foo-$TEST_RUN" | python3 -c "import sys,json; print(f\"Items: {len(json.load(sys.stdin)['items'])}\")"
```

**Expected results:**
- List returns exactly 1 item with name `list-field-foo-$TEST_RUN`
- `list-field-bar-$TEST_RUN` and `list-field-baz-$TEST_RUN` are not in results

---

## Test 23: List with labelSelector — filtered results

**Goal:** Verify that `?labelSelector=app=nginx` on a non-watch list request returns only matching objects.

```bash
# 1. Create widgets with different labels
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-nginx-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-apache-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-none-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

# 2. List with labelSelector=app=nginx
echo "=== List with labelSelector ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?labelSelector=app=nginx" | python3 -m json.tool

# 3. Verify only nginx widget returned
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?labelSelector=app=nginx" | python3 -c "import sys,json; items=json.load(sys.stdin)['items']; print(f\"Items: {len(items)}\"); [print(f\"  - {i['metadata']['name']}\") for i in items]"
```

**Expected results:**
- List returns exactly 1 item: `list-label-nginx-$TEST_RUN`
- Other widgets are not in results

---

## Test 24: List with both fieldSelector and labelSelector

**Goal:** Verify that both selectors are applied together on list requests.

```bash
# 1. Create widgets
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-other-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN-nolabel\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

# 2. List with both selectors
echo "=== List with both selectors ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.name=list-both-target-$TEST_RUN&labelSelector=app=nginx" | python3 -m json.tool

# 3. Verify only one item returned (matches both name AND label)
echo "=== Item count ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.name=list-both-target-$TEST_RUN&labelSelector=app=nginx" | python3 -c "import sys,json; print(f\"Items: {len(json.load(sys.stdin)['items'])}\")"
```

**Expected results:**
- List returns exactly 1 item: `list-both-target-$TEST_RUN`
- `list-both-other-$TEST_RUN` (wrong name) and `list-both-target-$TEST_RUN-nolabel` (no label) are excluded

---

## Test 25: List with filter and pagination

**Goal:** Verify that filtering happens before pagination (correct page sizes).

```bash
# 1. Create 10 widgets, only 3 have the target label
for i in $(seq 1 10); do
  if [ $i -le 3 ]; then
    labels='{"app":"nginx"}'
  else
    labels='{}'
  fi
  curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"list-pag-$(printf '%02d' $i)-$TEST_RUN\",\"labels\":$labels},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

# 2. Filter to 3, limit 10 → should return 3 (not 10)
echo "=== Filter + pagination ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?labelSelector=app=nginx&limit=10" | python3 -c "
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

## Test 26: List with filter that matches no objects

**Goal:** Verify that a filter matching no objects returns an empty list.

```bash
echo "=== Filter with no matches ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.name=nonexistent-$TEST_RUN" | python3 -c "
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

## Test 27: Watch with combined fieldSelector + labelSelector (AND semantics)

**Goal:** Verify that when both selectors are present on a watch request, they are combined with AND semantics.

```bash
# 1. Start a watch with both selectors
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=watch-combo-target-$TEST_RUN&labelSelector=app=nginx" \
  > /tmp/watch-combo.log 2>&1 &
WATCH_PID=$!
sleep 2

# 2. Create widget matching BOTH selectors (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}"

sleep 1

# 3. Create widget with matching name but wrong label (should NOT be delivered — label mismatch)
# NOTE: Uses a different name to avoid conflict; the fieldSelector filters by name, so
# a different name won't match the fieldSelector anyway. This tests label filtering independently.
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-wrong-label-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null

sleep 1

# 4. Create widget with matching label but wrong name (should NOT be delivered — name mismatch)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 28: Invalid fieldSelector on list returns 400

**Goal:** Verify that invalid field selectors on list requests return HTTP 400.

```bash
echo "=== Invalid fieldSelector on list ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io/v1/Widget?fieldSelector=metadata.namespace=default"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidFieldSelector"`

---

## Test 29: Invalid labelSelector on list returns 400

**Goal:** Verify that invalid label selectors on list requests return HTTP 400.

```bash
echo "=== Invalid labelSelector on list ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" \
  "http://localhost:8080/apis/example.io/v1/Widget?labelSelector=app="
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabelSelector"`

---

## Test 30: Status subresource — create object, update status via /status

**Goal:** Verify that a Schema with `statusSchema` enables the `/status` endpoint for reading and updating status.

```bash
# 1. Create an object
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"status-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -m json.tool

# 2. Verify status field is absent on created object
echo "=== Status on created object ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/status-widget-$TEST_RUN" | python3 -c "
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
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/status-widget-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\",\"message\":\"All systems go\"}}" | python3 -m json.tool

# 4. GET /status to verify
echo "=== GET /status ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/status-widget-$TEST_RUN/status" | python3 -m json.tool

# 5. GET full object to verify status persisted
echo "=== GET full object ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/status-widget-$TEST_RUN" | python3 -c "
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

## Test 31: Status subresource not enabled — Schema without statusSchema returns 404

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

## Test 32: Status update with invalid data returns 422

**Goal:** Verify that status updates are validated against `statusSchema`.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"invalid-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"}}" > /dev/null

# 2. Update status with invalid type (phase should be string, not integer)
echo "=== Invalid status update ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io/v1/Widget/invalid-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":123}}"
```

**Expected results:**
- HTTP 422 Unprocessable Entity
- Response body contains `"code": "SchemaValidation"` with validation error details

---

## Test 33: Status update for non-existent object returns 404 NotFound

**Goal:** Verify that updating status on a non-existent object returns `NotFound`.

```bash
# 1. PUT /status for non-existent object
echo "=== Status update for non-existent object ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT \
  "http://localhost:8080/apis/example.io/v1/Widget/nonexistent-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}"
```

**Expected results:**
- HTTP 404 Not Found
- Response body contains `"code": "NotFound"`

---

## Test 34: Status update does not modify spec

**Goal:** Verify that updating status leaves the spec field unchanged.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"spec-preserve-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

# 2. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/spec-preserve-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null

# 3. Verify spec is unchanged
echo "=== Verify spec unchanged ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/spec-preserve-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
spec = obj['spec']
status = obj['status']
print(f\"Spec color: {spec['color']}\")
print(f\"Spec size: {spec['size']}\")
print(f\"Status: {status}\")
assert spec['color'] == 'blue', 'spec.color should be unchanged'
assert spec['size'] == 10, 'spec.size should be unchanged'
print('PASS: spec unchanged, status set')
"
```

**Expected results:**
- `spec.color` is still `"blue"`, `spec.size` is still `10`
- `status` is `{"phase":"Running"}`

---

## Test 35: Status update bumps resourceVersion

**Goal:** Verify that status updates increment `resourceVersion`.

```bash
# 1. Create object and capture resourceVersion
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"rv-bump-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"}}")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "Initial resourceVersion: $INITIAL_RV"

# 2. Update status
STATUS_RESP=$(curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/rv-bump-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}")
STATUS_RV=$(echo "$STATUS_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "After status update resourceVersion: $STATUS_RV"

# 3. Verify bumped
python3 -c "
initial = $INITIAL_RV
after = $STATUS_RV
assert after > initial, f'resourceVersion should be bumped: {after} > {initial}'
print(f'PASS: resourceVersion bumped from {initial} to {after}')
"
```

**Expected results:**
- `resourceVersion` after status update is greater than initial `resourceVersion`

---

## Test 36: Create object with unknown top-level field — rejected with 400

**Goal:** Verify that unknown top-level fields in the create request body are rejected with 400 Bad Request.

```bash
# 1. Create object with unknown top-level field "status"
echo "=== Create with unknown field ==="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"create-with-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"},\"status\":{\"phase\":\"Pre-set\"}}"
```

**Expected results:**
- HTTP 400 Bad Request
- Response body contains `"code": "InvalidRequestBody"`
- Error message mentions unknown field(s)

---

## Test 37: Status update replaces status (not merged)

**Goal:** Verify that status updates completely replace the status field, not merge.

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"replace-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"}}" > /dev/null

# 2. Set status with both phase and message
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\",\"message\":\"initial message\"}}" > /dev/null

# 3. Update status with only phase (no message)
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Completed\"}}" > /dev/null

# 4. Verify message is gone (replaced, not merged)
echo "=== Verify status replaced ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/replace-status-$TEST_RUN/status" | python3 -c "
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

## Test 38: StatusModified watch event published on status update

**Goal:** Verify that status updates publish a `StatusModified` watch event (not `Modified`).

```bash
# 1. Create object
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"status-event-$TEST_RUN\"},\"spec\":{\"color\":\"blue\"}}" > /dev/null

# 2. Start watching BEFORE status update
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-status-event.log 2>&1 &
WATCH_PID=$!
sleep 2

# 3. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/status-event-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null

sleep 2

# 4. Kill the watch
kill $WATCH_PID 2>/dev/null

# 5. Verify StatusModified event received
echo "=== Watch events ==="
cat /tmp/watch-status-event.log

echo "=== Event types ==="
grep -o '"eventType":"[^"]*"' /tmp/watch-status-event.log
```

**Expected results:**
- Watch log contains `"eventType":"StatusModified"` event
- Event object includes full `StoredObject` (both spec and status)
- No `"eventType":"Modified"` for the status update

---

## Test 39: Spec update publishes Modified event (not StatusModified)

**Goal:** Verify that regular spec updates still publish `Modified` events (unchanged behavior).

```bash
# 1. Create object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"spec-event-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

# 2. Start watching
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-spec-event.log 2>&1 &
WATCH_PID=$!
sleep 2

# 3. Update spec (regular PUT)
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/spec-event-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"spec-event-$TEST_RUN\"},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null

sleep 2

# 4. Kill the watch
kill $WATCH_PID 2>/dev/null

# 5. Verify Modified event (not StatusModified)
echo "=== Watch events ==="
cat /tmp/watch-spec-event.log

echo "=== Event types ==="
grep -o '"eventType":"[^"]*"' /tmp/watch-spec-event.log
```

**Expected results:**
- Watch log contains `"eventType":"Modified"` for the spec update
- No `"eventType":"StatusModified"` for the spec update

---

## Test 40: Generation — starts at 1 on create

**Goal:** Verify that newly created objects have `generation: 1`.

```bash
# 1. Create an object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
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

## Test 41: Generation — metadata-only update does NOT bump generation

**Goal:** Verify that updating only labels (no spec change) increments `resourceVersion` but leaves `generation` unchanged.

```bash
# 1. Create object with labels
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_GEN=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

echo "Initial: generation=$INITIAL_GEN, resourceVersion=$INITIAL_RV"

# 2. Update with same spec but different labels (remove app, add env)
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$INITIAL_RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
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

## Test 42: Generation — spec change bumps generation

**Goal:** Verify that updating the spec increments both `generation` and `resourceVersion`.

```bash
# 1. Get current state
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

# 2. Update spec (change color)
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$BEFORE_RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
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

## Test 43: Generation — status update does NOT bump generation

**Goal:** Verify that updating status via `/status` increments `resourceVersion` but leaves `generation` unchanged.

```bash
# 1. Get current state
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

# 2. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-meta-$TEST_RUN/status" \
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

## Test 44: Generation — generation and resourceVersion are independent counters

**Goal:** Verify that after a sequence of mixed updates, `generation` reflects only spec changes while `resourceVersion` reflects all changes.

```bash
# 1. Start fresh — create a new object
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
echo "=== Step 0: CREATE ==="
echo "$CREATE_RESP" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 2. Update labels only (metadata change)
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN")
RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"spec\":{\"color\":\"blue\",\"size\":10}}
  }" > /dev/null
echo "=== Step 1: UPDATE labels only ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 3. Update spec
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN")
RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null
echo "=== Step 2: UPDATE spec ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 4. Update status
curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\"}}" > /dev/null
echo "=== Step 3: UPDATE status ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')
"

# 5. Update labels again (metadata change)
CURRENT=$(curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN")
RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED_AT=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"httpd\",\"env\":\"prod\"}},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"spec\":{\"color\":\"red\",\"size\":20}}
  }" > /dev/null
echo "=== Step 4: UPDATE labels again ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
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

## Cleanup

```bash
# Stop the server
kill $(lsof -ti :8080) 2>/dev/null || true

# Clean up temp files
rm -f /tmp/watch-*.log /tmp/kapi-server.log /tmp/kapi-server-persist.log
rm -f /tmp/kapi-persist-test.db /tmp/kapi-test.db
```

---

## Status Subresource Test Summary

| Test | Goal |
|---|---|
| 30 | Create object, update status via `/status`, verify status is set (schema includes `statusSchema` from Test 1) |
| 31 | Schema without statusSchema → GET/PUT `/status` returns 404 `StatusSubresourceNotEnabled` |
| 32 | Update status with invalid data → 422 `SchemaValidation` |
| 33 | Update status for non-existent object → 404 `NotFound` |
| 34 | Status update does not modify spec field |
| 35 | Status update bumps `resourceVersion` |
| 36 | Create object with unknown top-level field → rejected with 400 `InvalidRequestBody` |
| 37 | Status update replaces status completely (not merged) |
| 38 | Status update publishes `StatusModified` watch event |
| 39 | Spec update publishes `Modified` event (unchanged behavior) |
| 40 | Generation starts at 1 on create |
| 41 | Metadata-only update (labels change) does NOT bump generation |
| 42 | Spec change bumps generation |
| 43 | Status update does NOT bump generation |
| 44 | Generation and resourceVersion are independent counters (full lifecycle) |

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

---

## Running All Tests at Once

For convenience, you can use the test runner script at `/tmp/kapi_test_v2.sh` (generated from this document). Or create a one-liner that sources sections sequentially:

```bash
# Set up once per session
export KAPI_BASE="http://localhost:8080"
export TEST_RUN=$(date +%s)

# Run each test block in order (Tests 1–13 on the same server)
# Tests 15–21 (label selector watch) can be run after Test 13 on the same server
# Tests 22–29 (list filtering + combined watch selectors) can be run after Test 21 on the same server
# Tests 30–39 (status subresource) can be run after Test 29 on the same server
# Tests 40–44 (generation field) can be run after Test 39 on the same server
# Test 14 requires a server restart with KAPI_DB_PATH, so run it separately.
```

---

## Integration Test Binary

The project also includes a Rust integration test binary that tests against both memory and SQLite stores without requiring a running server:

```bash
cargo run --package kapi-tests
cargo test --lib
```

These cover additional scenarios including optimistic concurrency, schema deletion, validation edge cases, and full CRUD flows across both store backends.
