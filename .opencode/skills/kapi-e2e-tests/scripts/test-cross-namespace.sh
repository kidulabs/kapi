#!/bin/bash
# Test Area: Cross-Namespace List (Tests 69-71)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

GROUP="cross-ns.$TEST_RUN"
KIND="NamespacedWidget"
BASE="http://localhost:8080"
API="$BASE/apis/$GROUP/v1"

register_namespaced_schema "$GROUP" "v1" "$KIND"

# Pre-create the namespaces (Namespace existence is now required)
NS_API="$BASE/apis/kapi.io/v1/Namespace"
for ns in ns-alpha ns-beta ns-gamma; do
  curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$ns\"},\"spec\":{\"annotations\":{}}}" > /dev/null
done

# Create test objects across multiple namespaces
echo "Setting up test data..."
for ns_info in "ns-alpha|obj-a-$TEST_RUN|blue|1" "ns-alpha|obj-b-$TEST_RUN|red|2" "ns-beta|obj-c-$TEST_RUN|green|3" "ns-gamma|obj-d-$TEST_RUN|yellow|4" "ns-gamma|obj-e-$TEST_RUN|purple|5"; do
  IFS='|' read -r ns name color size <<< "$ns_info"
  curl -s -X POST "$API/namespaces/$ns/$KIND" \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$name\"},\"spec\":{\"color\":\"$color\",\"size\":$size}}" > /dev/null
done
echo "Test data created across ns-alpha, ns-beta, ns-gamma"

echo "========== TEST 69: List All Objects Across Namespaces =========="
curl -s -w "\nHTTP_STATUS: %{http_code}\n" "$API/$KIND" | python3 -c "
import sys;data=sys.stdin.read();body,status=data.rsplit('\nHTTP_STATUS: ',1);status=status.strip()
import json;body=json.loads(body)
items=body.get('items',[])
assert status=='200',f'Expected 200, got {status}'
assert len(items)==5,f'Expected 5 items, got {len(items)}'
namespaces=set(item['metadata'].get('namespace','') for item in items)
assert 'ns-alpha' in namespaces,f'Missing ns-alpha'
assert 'ns-beta' in namespaces,f'Missing ns-beta'
assert 'ns-gamma' in namespaces,f'Missing ns-gamma'
print(f'  Total items: {len(items)}')
print(f'  Namespaces: {sorted(namespaces)}')
print('PASS: All namespaces returned in cross-namespace list')
"
echo "T69_PASS"

echo "========== TEST 70: Paginated Cross-Namespace List =========="
# Get first page (limit=2)
FIRST=$(curl -s -w "\n%{http_code}" "$API/$KIND?limit=2")
FIRST_CODE=$(echo "$FIRST" | tail -1)
FIRST_BODY=$(echo "$FIRST" | sed '$d')
FIRST_ITEMS=$(echo "$FIRST_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(len(body.get('items',[])))" 2>/dev/null || echo "0")
FIRST_TOKEN=$(echo "$FIRST_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(body.get('continue_token','null'))" 2>/dev/null || echo "null")
echo "First page: HTTP $FIRST_CODE, items=$FIRST_ITEMS, token=$FIRST_TOKEN"

# Get second page
SECOND=$(curl -s -w "\n%{http_code}" "$API/$KIND?limit=2&continue=$FIRST_TOKEN")
SECOND_CODE=$(echo "$SECOND" | tail -1)
SECOND_BODY=$(echo "$SECOND" | sed '$d')
SECOND_ITEMS=$(echo "$SECOND_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(len(body.get('items',[])))" 2>/dev/null || echo "0")
SECOND_TOKEN=$(echo "$SECOND_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(body.get('continue_token','null'))" 2>/dev/null || echo "null")
echo "Second page: HTTP $SECOND_CODE, items=$SECOND_ITEMS, token=$SECOND_TOKEN"

# Get third page if continue_token present
THIRD_ITEMS=0
if [ "$SECOND_TOKEN" != "null" ] && [ "$SECOND_TOKEN" != "None" ] && [ -n "$SECOND_TOKEN" ]; then
  THIRD=$(curl -s -w "\n%{http_code}" "$API/$KIND?limit=2&continue=$SECOND_TOKEN")
  THIRD_CODE=$(echo "$THIRD" | tail -1)
  THIRD_BODY=$(echo "$THIRD" | sed '$d')
  THIRD_ITEMS=$(echo "$THIRD_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(len(body.get('items',[])))" 2>/dev/null || echo "0")
  THIRD_TOKEN=$(echo "$THIRD_BODY" | python3 -c "import sys,json;body=json.load(sys.stdin);print(body.get('continue_token','null'))" 2>/dev/null || echo "null")
  echo "Third page: HTTP $THIRD_CODE, items=$THIRD_ITEMS, token=$THIRD_TOKEN"
fi

# Verify totals
python3 << EOF
first=$FIRST_ITEMS; second=$SECOND_ITEMS; third=$THIRD_ITEMS
total=first+second+third
assert total==5, f'Expected 5 total items, got {total}'
assert first==2, f'Expected 2 on first page, got {first}'
assert '$FIRST_TOKEN'!='null' and '$FIRST_TOKEN'!='None' and '$FIRST_TOKEN'!='', f'Expected continue_token on first page'
print(f'PASS: Pagination works: {first}+{second}+{third}={total}')
EOF
echo "T70_PASS"

echo "========== TEST 71: Namespace-Scoped vs Cross-Namespace List Comparison =========="
NS_COUNT=$(curl -s "$API/namespaces/ns-alpha/$KIND" | python3 -c "import sys,json;body=json.load(sys.stdin);print(len(body.get('items',[])))")
ALL_COUNT=$(curl -s "$API/$KIND" | python3 -c "import sys,json;body=json.load(sys.stdin);print(len(body.get('items',[])))")
echo "  Namespace-scoped (ns-alpha) count: $NS_COUNT"
echo "  Cross-namespace count: $ALL_COUNT"

python3 << EOF
assert int($ALL_COUNT) > int($NS_COUNT), f'Cross-ns count ($ALL_COUNT) should be > ns-alpha count ($NS_COUNT)'
assert int($NS_COUNT)==2, f'Expected 2 items in ns-alpha, got $NS_COUNT'
print(f'PASS: Cross-ns count ($ALL_COUNT) > ns-alpha count ($NS_COUNT)')
EOF
echo "T71_PASS"

echo "========== CROSS-NAMESPACE TESTS COMPLETE =========="
