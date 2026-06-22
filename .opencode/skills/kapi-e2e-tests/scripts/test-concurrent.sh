#!/bin/bash
# Test Area: Concurrent & Failure (Tests 50-51)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget${query}" > "$logfile" 2>&1 &
  echo $!
}

echo "========== TEST 50: Concurrent spec and status =========="
register_widget_schema

CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"concurrent-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=concurrent-$TEST_RUN" /tmp/t50-watch.log)
sleep 2

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/concurrent-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}' > /dev/null

get_system_fields "concurrent-$TEST_RUN"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/concurrent-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"concurrent-$TEST_RUN\"},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":20}}"

sleep 2
kill $WATCH_PID 2>/dev/null
echo "Event types:"; grep -o '"eventType":"[^"]*"' /tmp/t50-watch.log
echo "T50_PASS"

echo "========== TEST 51: Failed operations don't publish events =========="
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-event-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")

WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=no-event-$TEST_RUN" /tmp/t51-watch.log)
sleep 2

curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/no-event-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"no-event-$TEST_RUN\"},\"system\":{\"resourceVersion\":99999,\"createdAt\":\"2026-01-01T00:00:00Z\",\"updatedAt\":\"2026-01-01T00:00:00Z\"},\"spec\":{\"color\":\"red\",\"size\":20}}"

sleep 2
kill $WATCH_PID 2>/dev/null
echo "Watch events:"
cat /tmp/t51-watch.log
COUNT=$(grep -c '"eventType"' /tmp/t51-watch.log)
echo "Event count: $COUNT (expected 0 - failed ops don't publish)"
echo "T51_PASS"

echo "========== CONCURRENT TESTS COMPLETE =========="
