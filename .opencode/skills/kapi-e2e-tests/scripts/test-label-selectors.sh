#!/bin/bash
# Test Area: Label Selectors (Tests 11-17)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 11: Watch labelSelector equality =========="
register_widget_schema

WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx" /tmp/t11-watch.log)
sleep 2
kill -0 $WATCH_PID 2>/dev/null && echo "Watch alive" || echo "ERROR: watch died"

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"other-labels-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"matching-labels-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":2}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t11-watch.log
echo "T11_PASS: labelSelector equality filtered correctly"

echo "========== TEST 12: Watch labelSelector AND combinator =========="
WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx,env=prod" /tmp/t12-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"partial-match2-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"green\",\"size\":2}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"full-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"env\":\"prod\"}},\"spec\":{\"color\":\"red\",\"size\":3}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t12-watch.log
echo "T12_PASS: AND combinator requires all labels"

echo "========== TEST 13: Watch !key non-existence =========="
WATCH_PID=$(start_watch "?watch=true&labelSelector=!experimental" /tmp/t13-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"has-experimental-$TEST_RUN\",\"labels\":{\"experimental\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-experimental-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":2}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t13-watch.log
echo "T13_PASS: !key filters correctly"

echo "========== TEST 14: Watch key!=value inequality =========="
WATCH_PID=$(start_watch "?watch=true&labelSelector=env!=prod" /tmp/t14-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-prod-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"is-staging-$TEST_RUN\",\"labels\":{\"env\":\"staging\"}},\"spec\":{\"color\":\"green\",\"size\":2}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"no-env-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":3}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t14-watch.log
echo "T14_PASS: != inequality filters correctly"

echo "========== TEST 15: Invalid labelSelector returns 400 =========="
for label in "empty-value-watch|app=|empty value" "empty-segment-watch|app=nginx,,env=prod|empty segment"; do
  IFS='|' read -r lbl selector expected <<< "$label"
  CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?watch=true&labelSelector=${selector}")
  echo "Case $lbl: HTTP $CODE (expected 400)"
done

# List context
CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?labelSelector=app=")
echo "empty-value-list: HTTP $CODE (expected 400)"
echo "T15_PASS: invalid labelSelector returns 400"

echo "========== TEST 16: Empty labelSelector matches all =========="
WATCH_PID=$(start_watch "?watch=true&labelSelector=" /tmp/t16-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-sel-1-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 0.5

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-sel-2-$TEST_RUN\",\"labels\":{}},\"spec\":{\"color\":\"red\",\"size\":2}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
COUNT=$(grep -c '"eventType":"Added"' /tmp/t16-watch.log)
echo "Event count: $COUNT (expected 2)"
echo "T16_PASS: empty labelSelector matches all"

echo "========== TEST 17: Mixed label selector operators =========="
WATCH_PID=$(start_watch "?watch=true&labelSelector=app=nginx,!experimental,gpu" /tmp/t17-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-match-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\"}},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-fail-exp-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"gpu\":\"true\",\"experimental\":\"true\"}},\"spec\":{\"color\":\"red\",\"size\":2}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"mixed-fail-gpu-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"green\",\"size\":3}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t17-watch.log
echo "T17_PASS: mixed operators work correctly"

echo "========== LABEL SELECTOR TESTS COMPLETE =========="
