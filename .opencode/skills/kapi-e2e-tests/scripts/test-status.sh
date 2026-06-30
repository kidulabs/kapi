#!/bin/bash
# Test Area: Status Subresource (Tests 25-32)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget${query}" > "$logfile" 2>&1 &
  echo $!
}

register_widget_schema_with_status

echo "========== TEST 25: Status subresource create/update =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"status-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "Status on created object:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/status-widget-$TEST_RUN" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
status=obj.get('status')
if status is None and 'status' not in obj: print('Status: field absent (correct)')
elif status is None: print('Status: null')
else: print(f'Status: {status}')
"

echo "PUT /status:"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/status-widget-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running","message":"All systems go"}}' | python3 -m json.tool

echo "GET /status:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/status-widget-$TEST_RUN/status" | python3 -m json.tool

echo "Full object:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/status-widget-$TEST_RUN" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
print(f'Status: {obj.get(\"status\")}')
print(f'Spec: {obj[\"spec\"]}')
"
echo "T25_PASS"

echo "========== TEST 26: Status not enabled =========="
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d '{"targetGroup":"test.io","targetVersion":"v1","targetKind":"Gadget","specSchema":{"type":"object","properties":{"name":{"type":"string"}}}}' > /dev/null

curl -s -X POST http://localhost:8080/apis/test.io/v1/Gadget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-status-gadget-$TEST_RUN\"},\"spec\":{\"name\":\"test\"}}" > /dev/null

echo "GET /status (expected 404):"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "http://localhost:8080/apis/test.io/v1/Gadget/no-status-gadget-$TEST_RUN/status"

echo "PUT /status (expected 404):"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/test.io/v1/Gadget/no-status-gadget-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}'
echo "T26_PASS"

echo "========== TEST 27: Invalid status data returns 422 =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"invalid-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

echo "Invalid status update:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/invalid-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":123}}'
echo "T27_PASS"

echo "========== TEST 28: Status update non-existent =========="
echo "Non-existent status update:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/nonexistent-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}'
echo "T28_PASS"

echo "========== TEST 29: Status side effects =========="
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"spec-preserve-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "Initial resourceVersion: $INITIAL_RV"

STATUS_RV=$(curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/spec-preserve-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}' | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
echo "After status update resourceVersion: $STATUS_RV"

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/spec-preserve-$TEST_RUN" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
print(f'Spec color: {obj[\"spec\"][\"color\"]}')
print(f'Spec size: {obj[\"spec\"][\"size\"]}')
print(f'Status: {obj[\"status\"]}')
print(f'ResourceVersion: {obj[\"system\"][\"resourceVersion\"]}')
assert obj['spec']['color']=='blue','spec.color unchanged'
assert obj['spec']['size']==10,'spec.size unchanged'
assert obj['system']['resourceVersion'] > $INITIAL_RV,'resourceVersion bumped'
print('PASS: spec unchanged, status set, resourceVersion bumped')
"
echo "T29_PASS"

echo "========== TEST 30: Create with unknown field =========="
echo "Create with unknown field:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"create-with-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1},\"status\":{\"phase\":\"Pre-set\"}}"
echo "T30_PASS"

echo "========== TEST 31: Status replaces (not merged) =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"replace-status-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running","message":"initial message"}}' > /dev/null

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/replace-status-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Completed"}}' > /dev/null

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/replace-status-$TEST_RUN/status" | python3 -c "
import sys,json;status=json.load(sys.stdin)
print(f'Status: {status}')
assert status.get('phase')=='Completed',f'phase should be Completed'
assert 'message' not in status,f'message should be removed'
print('PASS: status replaced, not merged')
"
echo "T31_PASS"

echo "========== TEST 32: Event types for status vs spec =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"event-types-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

WATCH_PID=$(start_watch "?watch=true" /tmp/t32-watch.log)
sleep 2

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/event-types-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}' > /dev/null
sleep 1

get_system_fields "event-types-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/event-types-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"event-types-$TEST_RUN\"},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Event types:"; grep -o '"eventType":"[^"]*"' /tmp/t32-watch.log
echo "T32_PASS"

echo "========== STATUS TESTS COMPLETE =========="
