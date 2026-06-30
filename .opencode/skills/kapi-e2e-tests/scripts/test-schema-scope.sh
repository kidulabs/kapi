#!/bin/bash
# Test Area: Schema Scope (Tests 52-59)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 52: Register Namespaced Schema =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{\"targetGroup\":\"schema-scope.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"NamespacedKind\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"color\":{\"type\":\"string\"},\"size\":{\"type\":\"integer\"}},\"required\":[\"color\",\"size\"]},\"scope\":\"Namespaced\"}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
scope=obj['spec'].get('scope','')
ns=obj['metadata'].get('namespace')
assert status=='201',f'Expected 201, got {status}'
assert scope=='Namespaced',f'Expected scope Namespaced, got {scope}'
print(f'  scope: {scope}, namespace: {ns}')
print('PASS: Namespaced schema created')
"
echo "T52_PASS"

echo "========== TEST 53: Register Cluster Schema =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{\"targetGroup\":\"schema-scope.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"ClusterKind\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"},\"replicas\":{\"type\":\"integer\"}},\"required\":[\"name\",\"replicas\"]},\"scope\":\"Cluster\"}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
scope=obj['spec'].get('scope','')
assert status=='201',f'Expected 201, got {status}'
assert scope=='Cluster',f'Expected scope Cluster, got {scope}'
print(f'  scope: {scope}')
print('PASS: Cluster schema created')
"
echo "T53_PASS"

echo "========== TEST 54: Register Schema with statusSchema =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{\"targetGroup\":\"schema-scope.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"StatusKind\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"image\":{\"type\":\"string\"},\"replicas\":{\"type\":\"integer\"}},\"required\":[\"image\"]},\"statusSchema\":{\"type\":\"object\",\"properties\":{\"phase\":{\"type\":\"string\"},\"availableReplicas\":{\"type\":\"integer\"}}},\"scope\":\"Namespaced\"}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
assert status=='201',f'Expected 201, got {status}'
print(f'  statusSchema present: {\"statusSchema\" in obj[\"spec\"]}')
print('PASS: Schema with statusSchema created')
"
echo "T54_PASS"

echo "========== TEST 55: List All Schemas =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" http://localhost:8080/apis/kapi.io/v1/Schema | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;body=json.loads(body)
items=body.get('items',[])
assert status=='200',f'Expected 200, got {status}'
assert len(items)>0,f'Expected non-empty items, got {len(items)}'
print(f'  Schema count: {len(items)}')
print('PASS: List schemas returned items')
"
echo "T55_PASS"

echo "========== TEST 56: Get Schema by Name =========="
SCHEMA_NAME="NamespacedKind.schema-scope.$TEST_RUN.v1"
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "http://localhost:8080/apis/kapi.io/v1/Schema/$SCHEMA_NAME" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace')
assert status=='200',f'Expected 200, got {status}'
assert ns is None or ns=='null',f'Expected namespace=null, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Schema has namespace=null')
"
echo "T56_PASS"

echo "========== TEST 57: Schema via Namespace-Scoped URL =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" http://localhost:8080/apis/kapi.io/v1/namespaces/default/Schema | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;body=json.loads(body)
items=body.get('items',[])
# Namespace-scoped Schema URL returns empty list (Schema is cluster-scoped)
assert status=='200',f'Expected 200, got {status}'
print(f'  Namespace-scoped Schema list returns {len(items)} items')
print('PASS: Schema namespace-scoped URL returns valid response')
"
echo "T57_PASS"

echo "========== TEST 58: Update Schema =========="
SCHEMA_NAME="NamespacedKind.schema-scope.$TEST_RUN.v1"
GET_RESP=$(curl -s "http://localhost:8080/apis/kapi.io/v1/Schema/$SCHEMA_NAME")
GET_RV=$(echo "$GET_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
GET_GEN=$(echo "$GET_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
GET_CREATED=$(echo "$GET_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
GET_UPDATED=$(echo "$GET_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "http://localhost:8080/apis/kapi.io/v1/Schema/$SCHEMA_NAME" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"kapi.io\",\"version\":\"v1\",\"kind\":\"Schema\"},\"metadata\":{\"name\":\"$SCHEMA_NAME\",\"labels\":{}},\"system\":{\"resourceVersion\":$GET_RV,\"generation\":$GET_GEN,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"targetGroup\":\"schema-scope.$TEST_RUN\",\"targetVersion\":\"v1\",\"targetKind\":\"NamespacedKind\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"color\":{\"type\":\"string\"},\"size\":{\"type\":\"integer\"},\"enabled\":{\"type\":\"boolean\"}},\"required\":[\"color\",\"size\"]},\"scope\":\"Namespaced\"}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
assert status=='200',f'Expected 200, got {status}'
new_rv=obj['system']['resourceVersion']
assert new_rv>$GET_RV,f'Expected resourceVersion > $GET_RV, got {new_rv}'
print(f'  resourceVersion: $GET_RV -> {new_rv}')
print('PASS: Schema updated with bumped resourceVersion')
"
echo "T58_PASS"

echo "========== TEST 59: Delete Schema =========="
SCHEMA_NAME="ClusterKind.schema-scope.$TEST_RUN.v1"
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "http://localhost:8080/apis/kapi.io/v1/Schema/$SCHEMA_NAME")
echo "HTTP Status: $STATUS (expected 200)"
[ "$STATUS" = "200" ] && echo "T59_PASS: Schema deleted successfully" || echo "T59_FAIL: Expected 200, got $STATUS"

echo "========== SCHEMA SCOPE TESTS COMPLETE =========="
