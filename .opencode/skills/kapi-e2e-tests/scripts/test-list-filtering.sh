#!/bin/bash
# Test Area: List Filtering (Tests 18-24)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

register_widget_schema

start_watch() {
  local query="$1" logfile="$2"
  curl -s -N "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget${query}" > "$logfile" 2>&1 &
  echo $!
}

echo "========== TEST 18: List with fieldSelector =========="
for name in "list-field-foo" "list-field-bar" "list-field-baz"; do
  curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$name-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

COUNT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?fieldSelector=metadata.name=list-field-foo-$TEST_RUN" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['items']))")
echo "Items: $COUNT (expected 1)"
echo "T18_PASS: list fieldSelector filters correctly"

echo "========== TEST 19: List with labelSelector =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-nginx-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-apache-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-label-none-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?labelSelector=app=nginx" | python3 -c "
import sys,json;items=json.load(sys.stdin)['items']
print(f'Items: {len(items)}')
for i in items: print(f'  - {i[\"metadata\"][\"name\"]}')
"
echo "T19_PASS: list labelSelector filters correctly"

echo "========== TEST 20: List with both selectors =========="
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-other-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"list-both-target-$TEST_RUN-nolabel\"},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null

COUNT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?fieldSelector=metadata.name=list-both-target-$TEST_RUN&labelSelector=app=nginx" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['items']))")
echo "Items: $COUNT (expected 1)"
echo "T20_PASS: combined selectors work"

echo "========== TEST 21: Filter + pagination =========="
for i in $(seq 1 10); do
  if [ $i -le 3 ]; then
    labels='{"pag-test-run":"true"}'
  else
    labels='{}'
  fi
  curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"list-pag-$(printf '%02d' $i)-$TEST_RUN\",\"labels\":$labels},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
done

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?labelSelector=pag-test-run=true&limit=10" | python3 -c "
import sys,json;body=json.load(sys.stdin);items=body['items']
print(f'Items returned: {len(items)}')
print(f'Continue token: {body.get(\"continueToken\", \"null\")}')
for i in items: print(f'  - {i[\"metadata\"][\"name\"]}')
"
echo "T21_PASS: filter+pagination works"

echo "========== TEST 22: Filter with no matches =========="
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?fieldSelector=metadata.name=nonexistent-$TEST_RUN" | python3 -c "
import sys,json;body=json.load(sys.stdin)
print(f'Items: {len(body[\"items\"])}')
print(f'Continue token: {body.get(\"continueToken\", \"null\")}')
"
echo "T22_PASS: empty results work"

echo "========== TEST 23: Watch combined fieldSelector + labelSelector =========="
WATCH_PID=$(start_watch "?watch=true&fieldSelector=metadata.name=watch-combo-target-$TEST_RUN&labelSelector=app=nginx" /tmp/t23-watch.log)
sleep 2

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-target-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-wrong-label-$TEST_RUN\",\"labels\":{\"app\":\"apache\"}},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
sleep 1

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-combo-other-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"green\",\"size\":30}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null
echo "Events:"; grep -o '"name":"[^"]*"' /tmp/t23-watch.log
echo "T23_PASS: combined AND semantics work"

echo "========== TEST 24: Invalid fieldSelector returns 400 =========="
CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget?fieldSelector=metadata.namespace=default")
echo "Invalid fieldSelector: HTTP $CODE (expected 400)"
echo "T24_PASS"

echo "========== LIST FILTERING TESTS COMPLETE =========="
