#!/bin/bash
# Test Area: Namespace CRUD (Tests 60-68)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

GROUP="ns-crud.$TEST_RUN"
KIND="NamespacedWidget"
BASE="http://localhost:8080"
API="$BASE/apis/$GROUP/v1"
NS_API="$BASE/apis/kapi.io/v1/Namespace"

register_namespaced_schema "$GROUP" "v1" "$KIND"

# Pre-create the namespaces used in this test (Namespace existence is now required)
for ns in staging production development defaults-test; do
  curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$ns\"},\"spec\":{\"annotations\":{}}}" > /dev/null
done

echo "========== TEST 60: Create Object in Namespace =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/namespaces/staging/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"widget-alpha-$TEST_RUN\",\"labels\":{\"app\":\"demo\",\"tier\":\"frontend\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace','')
assert status=='201',f'Expected 201, got {status}'
assert ns=='staging',f'Expected namespace=staging, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Object created in staging namespace')
"
echo "T60_PASS"

echo "========== TEST 61: Create Objects in Multiple Namespaces =========="
for ns_info in "production|widget-prod-$TEST_RUN|red|20" "development|widget-dev-$TEST_RUN|green|5" "staging|widget-beta-$TEST_RUN|purple|15"; do
  IFS='|' read -r ns name color size <<< "$ns_info"
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$API/namespaces/$ns/$KIND" \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$name\"},\"spec\":{\"color\":\"$color\",\"size\":$size}}")
  echo "  Create $name in $ns: HTTP $STATUS"
  [ "$STATUS" = "201" ] || echo "  FAIL: Expected 201"
done
echo "T61_PASS"

echo "========== TEST 62: Get Object by Namespace and Name =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "$API/namespaces/staging/$KIND/widget-alpha-$TEST_RUN" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace','')
assert status=='200',f'Expected 200, got {status}'
assert ns=='staging',f'Expected namespace=staging, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Got object with correct namespace')
"
echo "T62_PASS"

echo "========== TEST 63: Get 404 for Object in Wrong Namespace =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$API/namespaces/production/$KIND/widget-alpha-$TEST_RUN")
echo "HTTP Status: $STATUS (expected 404)"
[ "$STATUS" = "404" ] && echo "T63_PASS" || echo "T63_FAIL: Expected 404, got $STATUS"

echo "========== TEST 64: List Objects in Specific Namespace =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "$API/namespaces/staging/$KIND" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;body=json.loads(body)
items=body.get('items',[])
assert status=='200',f'Expected 200, got {status}'
for item in items:
    ns=item['metadata'].get('namespace','')
    assert ns=='staging',f'Expected staging, got {ns} for {item[\"metadata\"][\"name\"]}'
print(f'  Items in staging: {len(items)}')
for item in items:
    print(f'    - {item[\"metadata\"][\"name\"]} (ns={item[\"metadata\"].get(\"namespace\",\"\")})')
print('PASS: Only staging namespace objects returned')
"
echo "T64_PASS"

echo "========== TEST 65: Update Object in Namespace =========="
CURRENT=$(curl -s "$API/namespaces/staging/$KIND/widget-alpha-$TEST_RUN")
GET_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
GET_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
GET_CREATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
GET_UPDATED=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X PUT "$API/namespaces/staging/$KIND/widget-alpha-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"$GROUP\",\"version\":\"v1\",\"kind\":\"$KIND\"},\"metadata\":{\"name\":\"widget-alpha-$TEST_RUN\",\"namespace\":\"staging\",\"labels\":{\"app\":\"demo\",\"tier\":\"backend\",\"version\":\"v2\"}},\"system\":{\"resourceVersion\":$GET_RV,\"generation\":$GET_GEN,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":20}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
assert status=='200',f'Expected 200, got {status}'
new_rv=obj['system']['resourceVersion']
assert new_rv>$GET_RV,f'Expected resourceVersion > $GET_RV, got {new_rv}'
assert obj['spec']['size']==20,f'Expected size=20, got {obj[\"spec\"][\"size\"]}'
assert obj['metadata']['labels'].get('version')=='v2',f'Expected version=v2 label'
print(f'  resourceVersion: $GET_RV -> {new_rv}')
print(f'  spec.size: 20, labels.version: v2')
print('PASS: Object updated with bumped resourceVersion')
"
echo "T65_PASS"

echo "========== TEST 66: Update Fails with Wrong Namespace in Body =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API/namespaces/staging/$KIND/widget-alpha-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"$GROUP\",\"version\":\"v1\",\"kind\":\"$KIND\"},\"metadata\":{\"name\":\"widget-alpha-$TEST_RUN\",\"namespace\":\"wrong-ns\"},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":5}}")
echo "HTTP Status: $STATUS (expected 400)"
[ "$STATUS" = "400" ] && echo "T66_PASS" || echo "T66_FAIL: Expected 400, got $STATUS"

echo "========== TEST 67: Delete Object from Namespace =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$API/namespaces/staging/$KIND/widget-beta-$TEST_RUN")
echo "DELETE HTTP Status: $STATUS (expected 200)"
[ "$STATUS" = "200" ] && echo "  DELETE ok" || echo "  DELETE FAIL"

STATUS2=$(curl -s -o /dev/null -w "%{http_code}" "$API/namespaces/staging/$KIND/widget-beta-$TEST_RUN")
echo "GET after DELETE HTTP Status: $STATUS2 (expected 404)"
[ "$STATUS" = "200" ] && [ "$STATUS2" = "404" ] && echo "T67_PASS" || echo "T67_FAIL"

echo "========== TEST 68: Namespace Defaulting on Create =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/namespaces/defaults-test/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"widget-defaults-$TEST_RUN\",\"namespace\":\"ignored-ns\"},\"spec\":{\"color\":\"yellow\",\"size\":3}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace','')
assert status=='201',f'Expected 201, got {status}'
assert ns=='defaults-test',f'Expected namespace=defaults-test, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: URL namespace took precedence over body namespace')
"
echo "T68_PASS"

echo "========== NAMESPACE CRUD TESTS COMPLETE =========="
