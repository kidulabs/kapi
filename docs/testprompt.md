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

> **Re-run safety**: Each test uses `$TEST_RUN` as a suffix in object names (e.g. `target-widget-$TEST_RUN`) so you can re-run the entire suite without restarting the server. Tests that share objects across runs (Test 15, persistence) use the `$TEST_RUN` from the initial run or hard-coded names with explicit cleanup.

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
    "jsonSchema": {
      "type": "object",
      "properties": {
        "color": { "type": "string" },
        "size": { "type": "integer" }
      },
      "required": ["color", "size"]
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
  -d "{\"metadata\":{\"name\":\"other-widget-$TEST_RUN\"},\"color\":\"blue\",\"size\":1}"

sleep 1

# 4. Create the TARGET widget (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"target-widget-$TEST_RUN\"},\"color\":\"red\",\"size\":2}"

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
  -d "{\"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},\"color\":\"green\",\"size\":5}"

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
    \"data\":{\"value\":{\"color\":\"yellow\",\"size\":10}}
  }"

sleep 1

# 4. Delete the widget (expect Deleted)
curl -s -X DELETE "http://localhost:8080/apis/example.io/v1/Widget/lifecycle-$TEST_RUN"

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received all three event types
# NOTE: The SSE JSON uses "event_type" (not "type") as the discriminator field.
echo "=== Client received ==="
cat /tmp/watch-all.log

echo "=== Event counts ==="
grep -c '"event_type":"Added"' /tmp/watch-all.log && echo " Added events"
grep -c '"event_type":"Modified"' /tmp/watch-all.log && echo " Modified events"
grep -c '"event_type":"Deleted"' /tmp/watch-all.log && echo " Deleted events"

# 7. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher subscribed|event delivered|watch stream)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Client output contains three SSE events: `"event_type":"Added"`, `"event_type":"Modified"`, `"event_type":"Deleted"`
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
  -d "{\"metadata\":{\"name\":\"cleanup-trigger-$TEST_RUN\"},\"color\":\"black\",\"size\":99}"

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
  -d "{\"metadata\":{\"name\":\"named-$TEST_RUN\"},\"color\":\"green\",\"size\":3}"

sleep 1

# 4. Create "other-$TEST_RUN" — only watch-all should receive it
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-$TEST_RUN\"},\"color\":\"yellow\",\"size\":4}"

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

## Test 5: Watcher buffer full — slow consumer removed

**Goal:** Verify that a watcher whose channel buffer is full is removed (does not block other watchers).

> **Note**: This scenario is difficult to trigger on localhost because the TCP stack absorbs events faster than the mpsc buffer fills. On localhost the SSE stream drains the mpsc channel into the TCP send buffer before backpressure is felt. The buffer-full mechanism (`TrySendError::Full` → remove watcher) is validated by the `dead_watcher_cleanup` and `dropped_subscriber_does_not_block` unit tests in `src/event/bus.rs`.
>
> To reproduce this in E2E:
> 1. Reduce `DEFAULT_WATCHER_CAPACITY` in `src/event/bus.rs` from 256 to 2.
> 2. Use a slow-consumer client that opens the SSE connection but stops reading the response (causing the server's TCP send buffer to fill and backpressure to propagate to the mpsc channel). A simple `curl > /dev/null` reads fast enough on localhost to avoid triggering this.
> 3. Rebuild and restart the server, then run the steps below.

```bash
# Prerequisites (do once before this test):
#   - Edit src/event/bus.rs: change DEFAULT_WATCHER_CAPACITY from 256 to 2
#   - cargo build && restart server

# Slow consumer utility (save as /tmp/slow_consumer.py):
cat > /tmp/slow_consumer.py << 'PYEOF'
import socket, sys, time
HOST, PORT = "localhost", 8080
PATH = sys.argv[1] if len(sys.argv) > 1 else "/apis/example.io/v1/Widget?watch=true"
req = f"GET {PATH} HTTP/1.1\r\nHost: {HOST}:{PORT}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect((HOST, PORT))
sock.sendall(req.encode())
# Read just enough to get past headers, then stop reading
data = b""
while b"\r\n\r\n" not in data:
    chunk = sock.recv(4096)
    if not chunk: break
    data += chunk
print("Connected, holding connection open without reading further...", flush=True)
try:
    while True: time.sleep(10)
except KeyboardInterrupt:
    pass
sock.close()
PYEOF

# 1. Start a slow consumer (opens connection, then stops reading)
python3 /tmp/slow_consumer.py "/apis/example.io/v1/Widget?watch=true" &
SLOW_PID=$!
sleep 1

# 2. Start a normal watch that will read events
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-normal.log 2>&1 &
NORMAL_PID=$!

sleep 1

# 3. Create more objects than the buffer capacity (with capacity=2, create 5+)
for i in $(seq 1 5); do
  curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"buffer-test-$TEST_RUN-$i\"},\"color\":\"red\",\"size\":$i}"
  sleep 0.2
done

sleep 3

# 4. Kill watchers
kill $SLOW_PID $NORMAL_PID 2>/dev/null || true

# 5. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher buffer full|event delivered)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Server logs show `watcher buffer full, removing` for the slow consumer
- Normal watcher still receives events (slow consumer does not block)
- **If no buffer-full trace appears**: the TCP layer on localhost is absorbing events before backpressure reaches the mpsc channel. The mechanism itself is validated by unit tests — see `dead_watcher_cleanup` and `dropped_subscriber_does_not_block` in `src/event/bus.rs`.

---

## Test 6: Labels — create object with labels, verify in response and GET

**Goal:** Verify that `metadata.labels` are persisted and returned on create and get.

```bash
# 1. Register the Widget schema (no-op if already registered)
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "jsonSchema": {
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
    \"color\": \"blue\",
    \"size\": 10
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

## Test 7: Labels — create object without labels, verify empty map

**Goal:** Verify that omitting `metadata.labels` results in `"labels": {}`.

```bash
# 1. Create a widget without labels
echo "=== Create without labels ==="
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-labels-widget-$TEST_RUN\"},\"color\":\"red\",\"size\":5}" | python3 -m json.tool

# 2. GET and verify empty labels
echo "=== GET no-labels widget ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/no-labels-widget-$TEST_RUN" | python3 -m json.tool
```

**Expected results:**
- Create response contains `"labels": {}`
- GET response contains `"labels": {}`

---

## Test 8: Labels — update with changed labels (diff-based)

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
    \"data\": {\"value\":{\"color\":\"blue\",\"size\":10}}
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

## Test 9: Labels — create Schema with labels

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
    \"jsonSchema\": {
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

## Test 10: Labels — invalid key format returns 400

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
    \"color\": \"blue\",
    \"size\": 1
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions invalid characters

---

## Test 11: Labels — invalid value format returns 400

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
    \"color\": \"blue\",
    \"size\": 1
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions invalid characters in value

---

## Test 12: Labels — key exceeds length limit returns 400

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
    \"color\": \"blue\",
    \"size\": 1
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions maximum length of 256

---

## Test 13: Labels — value exceeds length limit returns 400

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
    \"color\": \"blue\",
    \"size\": 1
  }"
```

**Expected results:**
- HTTP 400 status
- Response body contains `"code": "InvalidLabel"`
- Error message mentions maximum length of 256

---

## Test 14: Labels — list returns labels for all objects

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

## Test 15: SQLite persistence survives restart

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
    "jsonSchema": {
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
    \"color\": \"blue\",
    \"size\": 10
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
    \"data\": {\"value\":{\"color\":\"blue\",\"size\":10}}
  }" > /dev/null

# 4. Verify labels before restart
echo "=== Before restart ==="
curl -s "http://localhost:8080/apis/example.io/v1/Widget/$PERSIST_NAME" | python3 -c "
import sys, json
obj = json.load(sys.stdin)
print(f\"Labels: {obj['metadata']['labels']}\")
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
print(f\"Labels: {obj['metadata']['labels']}\")
"
```

**Expected results:**
- Labels are identical before and after restart
- `app: httpd`, `tier: frontend`, `app.kubernetes.io/version: v1.2.3` all present
- `env` label still absent (was removed in update)

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

## Trace Log Reference

| Log Message | Source File | Meaning |
|---|---|---|
| `sse watch stream started` | `src/object/handler.rs` | SSE connection opened |
| `sse watch stream ended` | `src/object/handler.rs` | SSE stream wrapper initialized (logged by `stream::once` before events flow; does **not** mean the connection closed) |
| `watcher subscribed` | `src/event/bus.rs` | Watcher registered in EventBus |
| `event delivered to watcher` | `src/event/bus.rs` | Event matched filter and sent successfully |
| `event filtered out by watcher filter` | `src/event/bus.rs` | Event did not match watcher's fieldSelector |
| `watcher buffer full, removing` | `src/event/bus.rs` | Slow consumer removed (channel full) |
| `watcher channel closed, removing` | `src/event/bus.rs` | Dead watcher removed (client disconnected) |

---

## Running All Tests at Once

For convenience, you can use the test runner script at `/tmp/kapi_test_v2.sh` (generated from this document). Or create a one-liner that sources sections sequentially:

```bash
# Set up once per session
export KAPI_BASE="http://localhost:8080"
export TEST_RUN=$(date +%s)

# Run each test block in order (Tests 1–14 on the same server)
# Test 15 requires a server restart with KAPI_DB_PATH, so run it separately.
```

---

## Integration Test Binary

The project also includes a Rust integration test binary that tests against both memory and SQLite stores without requiring a running server:

```bash
cargo run --package kapi-tests
cargo test --lib
```

These cover additional scenarios including optimistic concurrency, schema deletion, validation edge cases, and full CRUD flows across both store backends.
