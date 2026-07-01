#!/bin/bash
# Test Area: Persistence (Tests 10, 49)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 10: SQLite persistence (labels/annotations) =========="
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1
PERSIST_DB=/tmp/kapi-persist-test.db
rm -f "$PERSIST_DB"
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run --bin kapi-server > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

register_widget_schema

# Create labels widget
LABELS_NAME="persist-labels-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"$LABELS_NAME\",\"labels\":{\"app\":\"nginx\",\"env\":\"prod\",\"app.kubernetes.io/version\":\"v1.2.3\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

get_system_fields "$LABELS_NAME"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$LABELS_NAME" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"$LABELS_NAME\",\"labels\":{\"app\":\"httpd\",\"app.kubernetes.io/version\":\"v1.2.3\",\"tier\":\"frontend\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

ANN_NAME="persist-ann-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"$ANN_NAME\",\"annotations\":{\"description\":\"persistent\",\"build\":\"v1.0.0\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "Before restart:"
for name in "$LABELS_NAME" "$ANN_NAME"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$name" | python3 -c "
import sys,json;obj=json.load(sys.stdin);md=obj['metadata']
print(f'$name: labels={md.get(\"labels\",{})}, annotations={md.get(\"annotations\",{})}')
"
done

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$LABELS_NAME" | python3 -c "
import sys,json
with open('/tmp/kapi-labels-before.json','w') as f: json.dump(json.load(sys.stdin)['metadata']['labels'],f,sort_keys=True)
"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$ANN_NAME" | python3 -c "
import sys,json
with open('/tmp/kapi-ann-before.json','w') as f: json.dump(json.load(sys.stdin)['metadata']['annotations'],f,sort_keys=True)
"

# Stop and restart
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 2

RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run --bin kapi-server > /tmp/kapi-server-persist.log 2>&1 &
sleep 3

echo "After restart:"
for name in "$LABELS_NAME" "$ANN_NAME"; do
  curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$name" | python3 -c "
import sys,json;obj=json.load(sys.stdin);md=obj['metadata']
print(f'$name: labels={md.get(\"labels\",{})}, annotations={md.get(\"annotations\",{})}')
"
done

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$LABELS_NAME" | python3 -c "
import sys,json
with open('/tmp/kapi-labels-after.json','w') as f: json.dump(json.load(sys.stdin)['metadata']['labels'],f,sort_keys=True)
"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$ANN_NAME" | python3 -c "
import sys,json
with open('/tmp/kapi-ann-after.json','w') as f: json.dump(json.load(sys.stdin)['metadata']['annotations'],f,sort_keys=True)
"

python3 -c "
import json
for field in ['labels','ann']:
    with open(f'/tmp/kapi-{field}-before.json') as f: before=json.load(f)
    with open(f'/tmp/kapi-{field}-after.json') as f: after=json.load(f)
    print(f'{field}: before={before}')
    print(f'{field}: after={after}')
    assert before==after, f'{field} differ'
print('PASS: Labels and annotations survived restart')
"
echo "T10_DONE"

echo "========== TEST 49: SQLite persistence (finalizers) =========="
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1
PERSIST_DB=/tmp/kapi-persist-fin.db
rm -f "$PERSIST_DB"
RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run --bin kapi-server > /tmp/kapi-server-persist-fin.log 2>&1 &
sleep 3

register_widget_schema

FIN_NAME="persist-fin-widget"
curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"$FIN_NAME\",\"finalizers\":[\"example.io.$TEST_RUN/cleanup\",\"kapi.io/finalizer\"]},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X DELETE "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$FIN_NAME" > /dev/null

echo "Before restart:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$FIN_NAME" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
md=obj['metadata'];sysf=obj['system']
print(f'finalizers: {md.get(\"finalizers\",[])}')
print(f'deletionTimestamp: {sysf.get(\"deletionTimestamp\",\"NOT SET\")}')
"

curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$FIN_NAME" > /tmp/kapi-fin-before.json

kill $(lsof -ti :8080) 2>/dev/null || true
sleep 2

RUST_LOG=kapi=trace KAPI_DB_PATH="$PERSIST_DB" cargo run --bin kapi-server > /tmp/kapi-server-persist-fin.log 2>&1 &
sleep 3

echo "After restart:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$FIN_NAME" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
md=obj['metadata'];sysf=obj['system']
print(f'finalizers: {md.get(\"finalizers\",[])}')
print(f'deletionTimestamp: {sysf.get(\"deletionTimestamp\",\"NOT SET\")}')
"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/$FIN_NAME" > /tmp/kapi-fin-after.json

python3 -c "
import json
with open('/tmp/kapi-fin-before.json') as f:before=json.load(f)
with open('/tmp/kapi-fin-after.json') as f:after=json.load(f)
bf=before['metadata'].get('finalizers',[]);af=after['metadata'].get('finalizers',[])
bd=before['system'].get('deletionTimestamp');ad=after['system'].get('deletionTimestamp')
print(f'finalizers before: {bf}')
print(f'finalizers after: {af}')
print(f'deletionTimestamp before: {bd}')
print(f'deletionTimestamp after: {ad}')
assert bf==af,'finalizers differ';assert bd==ad,'deletionTimestamp differ'
print('PASS: finalizers and deletionTimestamp survived restart')
"
echo "T49_PASS"

# Restart in-memory server for remaining tests
kill $(lsof -ti :8080) 2>/dev/null || true
sleep 1
RUST_LOG=kapi=trace cargo run --bin kapi-server > /tmp/kapi-server.log 2>&1 &
sleep 3

echo "========== PERSISTENCE TESTS COMPLETE =========="
