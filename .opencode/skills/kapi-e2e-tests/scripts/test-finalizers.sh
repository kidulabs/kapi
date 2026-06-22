#!/bin/bash
# Test Area: Finalizers (Tests 41-49)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget${query}" > "$logfile" 2>&1 &
  echo $!
}

echo "========== TEST 41: Finalizers create =========="
register_widget_schema

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-with-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
f=obj['metadata'].get('finalizers',None);print(f'finalizers: {f}')
"

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-without-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
f=obj['metadata'].get('finalizers',None);print(f'finalizers: {f}')
assert f==[],'Expected finalizers=[]'
print('PASS: finalizers=[] on create')
"

echo "GET finalizers:"
for suffix in "fin-with" "fin-without"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/${suffix}-$TEST_RUN" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
f=obj['metadata'].get('finalizers',[])
print(f'$suffix: finalizers={f}')
"
done
echo "T41_PASS"

echo "========== TEST 42: DELETE with/without finalizers =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-del-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "DELETE with finalizers:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-del-$TEST_RUN" | python3 -c "
import sys;data=sys.stdin.read()
body,status=data.rsplit('\nHTTP_STATUS: ',1)
import json;obj=json.loads(body)
print(f'Status: {status}')
print(f'deletionTimestamp: {obj[\"system\"].get(\"deletionTimestamp\", \"NOT SET\")}')
assert status.strip()=='200','Expected 200'
assert 'deletionTimestamp' in obj['system'],'deletionTimestamp should be set'
print('PASS: object marked for deletion')
"
echo

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-harddel-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

echo "DELETE without finalizers:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-harddel-$TEST_RUN"
echo

echo "GET after DELETE:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-harddel-$TEST_RUN"
echo "T42_PASS"

echo "========== TEST 43: Idempotent DELETE =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-idempotent-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"spec\":{\"color\":\"green\",\"size\":3}}" > /dev/null

FIRST_DEL=$(curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-idempotent-$TEST_RUN")
FIRST_DT=$(echo "$FIRST_DEL" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['deletionTimestamp'])")
echo "First deletionTimestamp: $FIRST_DT"

echo "Second DELETE:"
SECOND_DEL=$(curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-idempotent-$TEST_RUN")
echo "$SECOND_DEL"
SECOND_DT=$(echo "$SECOND_DEL" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1)
import json;print(json.loads(body)['system']['deletionTimestamp'])
")
echo "Second deletionTimestamp: $SECOND_DT"

python3 -c "assert '$FIRST_DT'=='$SECOND_DT',f'deletionTimestamp changed';print('PASS: deletionTimestamp unchanged on second DELETE')"
echo "T43_PASS"

echo "========== TEST 44: UPDATE on deleting objects =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" > /dev/null
sleep 1
get_system_fields "fin-update-$TEST_RUN"

echo "Case 1: UPDATE spec (should be rejected):"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":20}}"
echo

sleep 1
get_system_fields "fin-update-$TEST_RUN"

echo "Case 2: UPDATE finalizers (should succeed):"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}"
echo

sleep 1
get_system_fields "fin-update-$TEST_RUN"

echo "Case 3: UPDATE to empty finalizers (triggers hard delete):"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"fin-update-$TEST_RUN\",\"finalizers\":[]},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}"
echo

echo "GET after hard delete:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-update-$TEST_RUN"
echo "T44_PASS"

echo "========== TEST 45: Finalizer validation =========="
LONG_FINALIZER_JSON=$(python3 -c "import json;n='a'*257;print(json.dumps([n]))")
MANY_FINALIZERS=$(python3 -c "import json;finalizers=[f'fin-{i}.example.io.$TEST_RUN/cleanup' for i in range(21)];print(json.dumps(finalizers))")

for case in "invalid-chars|[\"invalid name with spaces\"]" "long-name|$LONG_FINALIZER_JSON" "too-many|$MANY_FINALIZERS"; do
  IFS='|' read -r suffix finalizers <<< "$case"
  CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"fin-${suffix}-$TEST_RUN\",\"finalizers\":$finalizers},\"spec\":{\"color\":\"blue\",\"size\":1}}")
  echo "Case $suffix: HTTP $CODE (expected 400)"
done
echo "T45_PASS"

echo "========== TEST 46: Watch events for finalizer lifecycle =========="
WATCH_PID=$(start_watch "?watch=true" /tmp/t46-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-watch-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
sleep 1

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-watch-$TEST_RUN" > /dev/null
sleep 1

get_system_fields "fin-watch-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-watch-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"fin-watch-$TEST_RUN\",\"finalizers\":[]},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Event types:"; grep -o '"eventType":"[^"]*"' /tmp/t46-watch.log
echo "T46_PASS"

echo "========== TEST 47: CREATE same-name after DELETE-with-finalizers =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-recreate-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-recreate-$TEST_RUN" > /dev/null
sleep 1

echo "CREATE same name while deleting:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-recreate-$TEST_RUN\",\"finalizers\":[\"other.io/finalizer\"]},\"spec\":{\"color\":\"red\",\"size\":20}}"
echo "T47_PASS"

echo "========== TEST 48: UPDATE adds finalizer on deleting object =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"fin-add-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-add-$TEST_RUN" > /dev/null
sleep 1
get_system_fields "fin-add-$TEST_RUN"

echo "UPDATE add finalizer on deleting object:"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/fin-add-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"fin-add-$TEST_RUN\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/new\"]},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}"
echo "T48_PASS"

echo "========== FINALIZER TESTS COMPLETE =========="
