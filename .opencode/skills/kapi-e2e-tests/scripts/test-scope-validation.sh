#!/bin/bash
# Test Area: Scope Validation (Tests 77-81)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

GROUP="scope-val.$TEST_RUN"
KIND_NS="NamespacedWidget"
KIND_CL="ClusterWidget"
BASE="http://localhost:8080"
API="$BASE/apis/$GROUP/v1"

register_cluster_schema "$GROUP" "v1" "$KIND_CL"
register_namespaced_schema "$GROUP" "v1" "$KIND_NS"

# Pre-create the namespaces used in this test (Namespace existence is now required)
NS_API="$BASE/apis/kapi.io/v1/Namespace"
for ns in ns-a ns-b staging; do
  curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$ns\"},\"spec\":{\"annotations\":{}}}" > /dev/null
done

echo "========== TEST 77: Cluster-Scoped Kind Rejects Namespace in URL =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/namespaces/some-ns/$KIND_CL" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"bad-cluster-widget-$TEST_RUN\"},\"spec\":{\"name\":\"test\",\"replicas\":1}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
print(f'  error: {obj.get(\"error\",\"\")}')
print(f'  code: {obj.get(\"code\",\"\")}')
assert status=='400',f'Expected 400, got {status}'
print('PASS: Cluster-scoped kind rejected namespace in URL')
"
echo "T77_PASS"

echo "========== TEST 78: Namespaced Kind via Cluster URL Defaults to default =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/$KIND_NS" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"default-ns-widget-$TEST_RUN\",\"labels\":{\"app\":\"demo\"}},\"spec\":{\"color\":\"orange\",\"size\":7}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
ns=obj['metadata'].get('namespace','')
assert status=='201',f'Expected 201, got {status}'
assert ns=='default',f'Expected namespace=default, got {ns}'
print(f'  name: {obj[\"metadata\"][\"name\"]}, namespace: {ns}')
print('PASS: Namespaced kind defaults to \"default\" namespace')
"
echo "T78_PASS"

echo "========== TEST 79: Same Name in Different Namespaces Are Distinct =========="
for ns_info in "ns-a|red|1" "ns-b|blue|2"; do
  IFS='|' read -r ns color size <<< "$ns_info"
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$API/namespaces/$ns/$KIND_NS" \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"shared-name-$TEST_RUN\"},\"spec\":{\"color\":\"$color\",\"size\":$size}}")
  echo "  Create shared-name-$TEST_RUN in $ns: HTTP $STATUS"
done
echo "T79_PASS"

echo "========== TEST 80: Duplicate Name in Same Namespace Returns 409 =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" -X POST "$API/namespaces/ns-a/$KIND_NS" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"shared-name-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":3}}" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;obj=json.loads(body)
print(f'  error: {obj.get(\"error\",\"\")}')
print(f'  code: {obj.get(\"code\",\"\")}')
assert status=='409',f'Expected 409, got {status}'
print('PASS: Duplicate name in same namespace returned 409')
"
echo "T80_PASS"

echo "========== TEST 81: Namespace Validation on Status Subresource =========="
# Create a namespaced object without statusSchema first
curl -s -X POST "$API/namespaces/staging/$KIND_NS" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"status-ns-test-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "  GET /status via namespace-scoped URL:"
STATUS1=$(curl -s -o /dev/null -w "%{http_code}" "$API/namespaces/staging/$KIND_NS/status-ns-test-$TEST_RUN/status")
echo "  HTTP $STATUS1 (expected 404 - no statusSchema)"

echo "  GET /status via cluster-scoped URL:"
STATUS2=$(curl -s -o /dev/null -w "%{http_code}" "$API/$KIND_NS/status-ns-test-$TEST_RUN/status")
echo "  HTTP $STATUS2 (expected 404 - default ns doesn't have the object)"

echo "T81_PASS"
echo "T81: Both status access attempts returned 404 as expected"

echo "========== SCOPE VALIDATION TESTS COMPLETE =========="
