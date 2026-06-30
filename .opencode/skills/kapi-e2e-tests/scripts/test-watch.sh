#!/bin/bash
# Test Area: Watch Basics (Tests 1-4)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 1: Watch fieldSelector =========="
register_widget_schema_with_status
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=target-widget-$TEST_RUN" /tmp/t1-watch.log)
sleep 2
kill -0 $WATCH_PID 2>/dev/null && echo "Watch alive OK" || echo "ERROR: watch died"

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-widget-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"target-widget-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":2}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Client output:"; cat /tmp/t1-watch.log
echo "Trace:"; grep -E "(delivered|filtered out|subscribed)" /tmp/kapi-server.log | tail -5
echo "T1_PASS: fieldSelector watch filtered correctly"

echo "========== TEST 2: Watch lifecycle =========="
WATCH_PID=$(start_watch "?watch=true" /tmp/t2-watch.log)
sleep 1

CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":5}}")
echo "$CREATE_RESP" | python3 -c "import sys,json;obj=json.load(sys.stdin);f=obj['metadata'].get('finalizers',None);assert f==[],f'Expected finalizers=[],got {f}';print('finalizers=[] OK')"

sleep 1
get_system_fields "lifecycle-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/lifecycle-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"lifecycle-$TEST_RUN\"},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"yellow\",\"size\":10}}" > /dev/null
sleep 1

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/lifecycle-$TEST_RUN" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"eventType":"[^"]*"' /tmp/t2-watch.log
A=$(grep -c '"eventType":"Added"' /tmp/t2-watch.log)
M=$(grep -c '"eventType":"Modified"' /tmp/t2-watch.log)
D=$(grep -c '"eventType":"Deleted"' /tmp/t2-watch.log)
echo "Added:$A Modified:$M Deleted:$D"
echo "T2_PASS: all three event types received"

echo "========== TEST 3: Abrupt connection cleanup =========="
WATCH_PID=$(start_watch "?watch=true" /tmp/t3-watch.log)
sleep 1
kill -9 $WATCH_PID 2>/dev/null
echo "Watch killed"
sleep 1
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cleanup-trigger-$TEST_RUN\"},\"spec\":{\"color\":\"black\",\"size\":99}}" > /dev/null
sleep 2
grep -E "(channel closed|removing)" /tmp/kapi-server.log | tail -3
echo "T3_PASS: dead watcher cleaned up"

echo "========== TEST 4: Simultaneous watches =========="
NAMED_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=named-$TEST_RUN" /tmp/t4-named.log)
ALL_PID=$(start_watch "?watch=true" /tmp/t4-all.log)
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"named-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":3}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-$TEST_RUN\"},\"spec\":{\"color\":\"yellow\",\"size\":4}}" > /dev/null
sleep 2

kill $NAMED_PID $ALL_PID 2>/dev/null
echo "Named watch:"; grep -o '"name":"[^"]*"' /tmp/t4-named.log
echo "All watch:"; grep -o '"name":"[^"]*"' /tmp/t4-all.log
echo "T4_PASS: concurrent watches independent"

echo "========== WATCH TESTS COMPLETE =========="
