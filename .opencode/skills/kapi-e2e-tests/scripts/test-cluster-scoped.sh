#!/bin/bash
# Test Area: Cluster-Scoped Resources (Tests 72-76)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

GROUP="cluster-scope.$TEST_RUN"
KIND="ClusterWidget"
BASE="http://localhost:8080"
API="$BASE/apis/$GROUP/v1"

register_schema_with_scope "$GROUP" "v1" "$KIND" "Cluster" '{"type":"object","properties":{"name":{"type":"string"},"replicas":{"type":"integer"}},"required":["name","replicas"]}'

echo "========== TEST 72: Create Cluster-Scoped Object =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"my-cluster-widget-$TEST_RUN\",\"labels\":{\"app.example.io/name\":\"my-cluster-widget-$TEST_RUN\"}},\"spec\":{\"name\":\"demo-cluster\",\"replicas\":3}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace')
assert status=='201',f'Expected 201, got {status}'
assert ns is None or ns=='null',f'Expected namespace=null, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Cluster-scoped object created with namespace=null')
"
echo "T72_PASS"

echo "========== TEST 73: List Cluster-Scoped Objects =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "$API/$KIND" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;body=json.loads(body)
items=body.get('items',[])
assert status=='200',f'Expected 200, got {status}'
assert len(items)>0,f'Expected non-empty items, got {len(items)}'
print(f'  Items: {len(items)}')
print('PASS: Cluster-scoped list returned items')
"
echo "T73_PASS"

echo "========== TEST 74: Get Cluster-Scoped Object =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "$API/$KIND/my-cluster-widget-$TEST_RUN" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace')
assert status=='200',f'Expected 200, got {status}'
assert ns is None or ns=='null',f'Expected namespace=null, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Got cluster-scoped object with namespace=null')
"
echo "T74_PASS"

echo "========== TEST 75: Update Cluster-Scoped Object =========="
CURRENT=$(curl -s "$API/$KIND/my-cluster-widget-$TEST_RUN")
GET_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
GET_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
GET_CREATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
GET_UPDATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "$API/$KIND/my-cluster-widget-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"$GROUP\",\"version\":\"v1\",\"kind\":\"$KIND\"},\"metadata\":{\"name\":\"my-cluster-widget-$TEST_RUN\",\"labels\":{\"app.example.io/name\":\"my-cluster-widget-$TEST_RUN\",\"updated\":\"true\"}},\"system\":{\"resourceVersion\":$GET_RV,\"generation\":$GET_GEN,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"name\":\"demo-cluster\",\"replicas\":5}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
assert status=='200',f'Expected 200, got {status}'
new_rv=obj['system']['resourceVersion']
assert new_rv>$GET_RV,f'Expected resourceVersion > $GET_RV, got {new_rv}'
assert obj['spec']['replicas']==5,f'Expected replicas=5, got {obj[\"spec\"][\"replicas\"]}'
print(f'  resourceVersion: $GET_RV -> {new_rv}')
print(f'  spec.replicas: 5')
print('PASS: Cluster-scoped object updated')
"
echo "T75_PASS"

echo "========== TEST 76: Delete Cluster-Scoped Object =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$API/$KIND/my-cluster-widget-$TEST_RUN")
echo "HTTP Status: $STATUS (expected 200)"
[ "$STATUS" = "200" ] && echo "T76_PASS" || echo "T76_FAIL: Expected 200, got $STATUS"

echo "========== CLUSTER-SCOPED RESOURCES TESTS COMPLETE =========="
