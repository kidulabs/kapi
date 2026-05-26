# Kapi Watch Semantics — End-to-End Test Prompts

Use these prompts to verify watch semantics by running the server with trace logging and exercising the SSE watch endpoints with curl.

---

## Prerequisites

```bash
# Build
cargo build

# Start server with trace logging (logs go to /tmp/kapi-server.log)
RUST_LOG=kapi=trace cargo run > /tmp/kapi-server.log 2>&1 &
sleep 2

# Verify server is up
curl -s http://localhost:8080/apis/kapi.io/v1/Schema
```

---

## Test 1: Watch with fieldSelector — matching event delivered, non-matching filtered

**Goal:** Verify that `?fieldSelector=metadata.name=<value>` only delivers events for the specified name.

```bash
# 1. Register the Widget schema (if not already done)
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

# 2. Start a watch filtered to "target-widget"
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=target-widget" \
  > /tmp/watch-fieldselector.log 2>&1 &
WATCH_PID=$!
sleep 1

# 3. Create a NON-target widget (should NOT be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{"metadata":{"name":"other-widget"},"color":"blue","size":1}'

sleep 1

# 4. Create the TARGET widget (should be delivered)
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{"metadata":{"name":"target-widget"},"color":"red","size":2}'

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
- Client output contains only `target-widget` Added event (no `other-widget`)
- Server logs show:
  - `sse watch stream started`
  - `watcher subscribed`
  - `event filtered out by watcher filter name=other-widget`
  - `event delivered to watcher name=target-widget`

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
  -d '{"metadata":{"name":"lifecycle-test"},"color":"green","size":5}'

sleep 1

# 3. Update the widget (expect Modified)
# First get the current resourceVersion from the create response or list
RV=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-test | grep -o '"resourceVersion":[0-9]*' | grep -o '[0-9]*')
CREATED_AT=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-test | grep -o '"createdAt":"[^"]*"' | sed 's/"createdAt":"//;s/"//')
UPDATED_AT=$(curl -s http://localhost:8080/apis/example.io/v1/Widget/lifecycle-test | grep -o '"updatedAt":"[^"]*"' | sed 's/"updatedAt":"//;s/"//')

curl -s -X PUT http://localhost:8080/apis/example.io/v1/Widget/lifecycle-test \
  -H "Content-Type: application/json" \
  -d "{
    \"key\":{\"group\":\"example.io\",\"version\":\"v1\",\"kind\":\"Widget\"},
    \"metadata\":{\"name\":\"lifecycle-test\"},
    \"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED_AT\",\"updatedAt\":\"$UPDATED_AT\"},
    \"data\":{\"value\":{\"color\":\"yellow\",\"size\":10}}
  }"

sleep 1

# 4. Delete the widget (expect Deleted)
curl -s -X DELETE http://localhost:8080/apis/example.io/v1/Widget/lifecycle-test

sleep 2

# 5. Kill the watch
kill $WATCH_PID 2>/dev/null

# 6. Verify client received all three event types
echo "=== Client received ==="
cat /tmp/watch-all.log

# 7. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher subscribed|event delivered|watch stream)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Client output contains three SSE events: `Added`, `Modified`, `Deleted`
- Server logs show three `event delivered to watcher name=lifecycle-test` entries

---

## Test 3: Abrupt connection cleanup — dead watcher removed on next publish

**Goal:** Verify that when a client disconnects abruptly (e.g., network drop), the watcher resource is cleaned up lazily on the next publish.

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
  -d '{"metadata":{"name":"cleanup-trigger"},"color":"black","size":99}'

sleep 1

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
# 1. Start filtered watch (name=named-one)
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=named-one" \
  > /tmp/watch-named.log 2>&1 &
NAMED_PID=$!

# 2. Start watch-all
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-sim-all.log 2>&1 &
ALL_PID=$!

sleep 1

# 3. Create "named-one" — both watchers should receive it
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{"metadata":{"name":"named-one"},"color":"green","size":3}'

sleep 1

# 4. Create "other" — only watch-all should receive it
curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
  -H "Content-Type: application/json" \
  -d '{"metadata":{"name":"other"},"color":"yellow","size":4}'

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
- Named watch: only `named-one` event
- All watch: both `named-one` and `other` events

---

## Test 5: Watcher buffer full — slow consumer removed

**Goal:** Verify that a watcher whose channel buffer is full is removed (does not block other watchers).

```bash
# This test requires modifying the watcher capacity to a small value for demonstration.
# In src/event/bus.rs, change DEFAULT_WATCHER_CAPACITY from 256 to 2, rebuild, and restart.

# 1. Start a watch but do not read from it (simulating slow consumer)
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /dev/null 2>&1 &
SLOW_PID=$!

# 2. Start a normal watch that will read events
curl -s -N "http://localhost:8080/apis/example.io/v1/Widget?watch=true" \
  > /tmp/watch-normal.log 2>&1 &
NORMAL_PID=$!

sleep 1

# 3. Create more objects than the buffer capacity (with capacity=2, create 5)
for i in $(seq 1 5); do
  curl -s -X POST http://localhost:8080/apis/example.io/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"buffer-test-$i\"},\"color\":\"red\",\"size\":$i}"
  sleep 0.2
done

sleep 2

# 4. Kill watches
kill $SLOW_PID $NORMAL_PID 2>/dev/null

# 5. Verify trace logs
echo "=== Server trace logs ==="
grep -E "(watcher buffer full|event delivered)" /tmp/kapi-server.log | tail -10
```

**Expected results:**
- Server logs show `watcher buffer full, removing` for the slow consumer
- Normal watcher still receives events (slow consumer does not block)

---

## Cleanup

```bash
# Stop the server
kill $(pgrep -f "target/debug/kapi") 2>/dev/null

# Clean up temp files
rm -f /tmp/watch-*.log /tmp/kapi-server.log
```

---

## Trace Log Reference

| Log Message | Source File | Meaning |
|---|---|---|
| `sse watch stream started` | `src/object/handler.rs` | SSE connection opened |
| `sse watch stream ended` | `src/object/handler.rs` | SSE connection closed (normal or abrupt) |
| `watcher subscribed` | `src/event/bus.rs` | Watcher registered in EventBus |
| `event delivered to watcher` | `src/event/bus.rs` | Event matched filter and sent successfully |
| `event filtered out by watcher filter` | `src/event/bus.rs` | Event did not match watcher's fieldSelector |
| `watcher buffer full, removing` | `src/event/bus.rs` | Slow consumer removed (channel full) |
| `watcher channel closed, removing` | `src/event/bus.rs` | Dead watcher removed (client disconnected) |
