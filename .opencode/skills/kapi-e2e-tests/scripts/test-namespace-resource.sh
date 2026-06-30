#!/bin/bash
# Test Area: Namespace Resource (Tests 84-93)
# Tests Namespace as a first-class cluster-scoped resource:
#   - CRUD on Namespace objects
#   - "default" namespace protection
#   - Namespace existence validation on object creation
#   - Namespace deletion blocking when non-empty
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

BASE="http://localhost:8080"
NS_API="$BASE/apis/kapi.io/v1/Namespace"
NS_LIST="$BASE/apis/kapi.io/v1/Namespace"

# Register a namespaced schema for the deletion-blocking tests
GROUP="ns-resource.$TEST_RUN"
KIND="NamespacedWidget"
OBJ_API="$BASE/apis/$GROUP/v1"
register_namespaced_schema "$GROUP" "v1" "$KIND"

PASS_COUNT=0
FAIL_COUNT=0
record() {
  local name="$1"
  local result="$2"
  if [ "$result" = "PASS" ]; then
    echo "  PASS: $name"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    echo "  FAIL: $name"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

echo "========== TEST 84: Create Namespace via API =========="
RESP=$(curl -s -w "\n%{http_code}" -X POST "$NS_API" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"production-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}")
BODY=$(echo "$RESP" | head -n -1)
STATUS=$(echo "$RESP" | tail -n 1)
if [ "$STATUS" = "201" ]; then
  NS_NAME=$(echo "$BODY" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata']['name'])" 2>/dev/null)
  NS_NS=$(echo "$BODY" | python3 -c "import sys,json;d=json.load(sys.stdin);print(d['metadata'].get('namespace','<none>'))" 2>/dev/null)
  if [ "$NS_NAME" = "production-$TEST_RUN" ] && [ "$NS_NS" = "<none>" ]; then
    record "Create Namespace via API" PASS
  else
    record "Create Namespace via API (name=$NS_NAME ns=$NS_NS)" FAIL
  fi
else
  record "Create Namespace via API (HTTP $STATUS)" FAIL
fi

echo "========== TEST 85: Get Namespace =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$NS_API/production-$TEST_RUN")
[ "$STATUS" = "200" ] && record "Get Namespace (HTTP $STATUS)" PASS || record "Get Namespace (HTTP $STATUS)" FAIL

echo "========== TEST 86: Get Non-existent Namespace Returns 404 =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$NS_API/nonexistent-$TEST_RUN")
[ "$STATUS" = "404" ] && record "Get non-existent Namespace (HTTP $STATUS)" PASS || record "Get non-existent Namespace (HTTP $STATUS)" FAIL

echo "========== TEST 87: List Namespaces =========="
LIST=$(curl -s "$NS_LIST")
COUNT=$(echo "$LIST" | python3 -c "import sys,json;print(len(json.load(sys.stdin).get('items',[])))" 2>/dev/null)
if [ "$COUNT" -ge 1 ]; then
  record "List Namespaces (count=$COUNT, default must be present)" PASS
else
  record "List Namespaces (count=$COUNT)" FAIL
fi

# Verify "default" is in the list
DEFAULT_PRESENT=$(echo "$LIST" | python3 -c "import sys,json;names=[i['metadata']['name'] for i in json.load(sys.stdin).get('items',[])];print('yes' if 'default' in names else 'no')" 2>/dev/null)
[ "$DEFAULT_PRESENT" = "yes" ] && record "default namespace is in list" PASS || record "default namespace missing from list" FAIL

echo "========== TEST 88: Update Namespace Labels =========="
CURRENT=$(curl -s "$NS_API/production-$TEST_RUN")
RV=$(echo "$CURRENT" | python3 -c "import sys,json;print(json.load(sys.stdin)['system']['resourceVersion'])" 2>/dev/null)
CREATED=$(echo "$CURRENT" | python3 -c "import sys,json;print(json.load(sys.stdin)['system']['createdAt'])" 2>/dev/null)
UPDATED=$(echo "$CURRENT" | python3 -c "import sys,json;print(json.load(sys.stdin)['system']['updatedAt'])" 2>/dev/null)

RESP=$(curl -s -w "\n%{http_code}" -X PUT "$NS_API/production-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"kapi.io\",\"version\":\"v1\",\"kind\":\"Namespace\"},\"metadata\":{\"name\":\"production-$TEST_RUN\",\"labels\":{\"env\":\"prod\",\"team\":\"platform\"}},\"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED\",\"updatedAt\":\"$UPDATED\"},\"spec\":{\"annotations\":{}}}")
STATUS=$(echo "$RESP" | tail -n 1)
if [ "$STATUS" = "200" ]; then
  BODY=$(echo "$RESP" | head -n -1)
  LABEL=$(echo "$BODY" | python3 -c "import sys,json;d=json.load(sys.stdin);print(d['metadata'].get('labels',{}).get('env',''))" 2>/dev/null)
  if [ "$LABEL" = "prod" ]; then
    record "Update Namespace labels (env=prod)" PASS
  else
    record "Update Namespace labels (got env=$LABEL)" FAIL
  fi
else
  record "Update Namespace labels (HTTP $STATUS)" FAIL
fi

echo "========== TEST 89: Delete 'default' Namespace Rejected (403) =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$NS_API/default")
[ "$STATUS" = "403" ] && record "DELETE default rejected (HTTP 403)" PASS || record "DELETE default returned HTTP $STATUS, expected 403" FAIL

# Verify default still exists
STATUS2=$(curl -s -o /dev/null -w "%{http_code}" "$NS_API/default")
[ "$STATUS2" = "200" ] && record "default namespace still exists after failed delete" PASS || record "default namespace missing after failed delete (HTTP $STATUS2)" FAIL

echo "========== TEST 90: Object Creation in Non-Existent Namespace Returns 404 =========="
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$OBJ_API/namespaces/ghost-ns-$TEST_RUN/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"widget-ghost\"},\"spec\":{\"color\":\"red\",\"size\":1}}")
[ "$STATUS" = "404" ] && record "Create in non-existent namespace (HTTP 404)" PASS || record "Create in non-existent namespace (HTTP $STATUS, expected 404)" FAIL

echo "========== TEST 91: Namespace Deletion Blocked When Non-Empty (409) =========="
# Create a new namespace, then an object in it
curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"to-fill-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null

curl -s -X POST "$OBJ_API/namespaces/to-fill-$TEST_RUN/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"widget-1\"},\"spec\":{\"color\":\"blue\",\"size\":1}}" > /dev/null

# Try to delete the namespace
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$NS_API/to-fill-$TEST_RUN")
[ "$STATUS" = "409" ] && record "Delete non-empty namespace (HTTP 409)" PASS || record "Delete non-empty namespace (HTTP $STATUS, expected 409)" FAIL

echo "========== TEST 92: Namespace Deletion Succeeds After Emptying =========="
# Delete the object
curl -s -X DELETE "$OBJ_API/namespaces/to-fill-$TEST_RUN/$KIND/widget-1" > /dev/null
# Now delete the namespace
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$NS_API/to-fill-$TEST_RUN")
[ "$STATUS" = "200" ] && record "Delete empty namespace (HTTP 200)" PASS || record "Delete empty namespace (HTTP $STATUS, expected 200)" FAIL

echo "========== TEST 93: Empty Namespace Deletion Succeeds =========="
# Create a fresh empty namespace
curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"empty-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$NS_API/empty-$TEST_RUN")
[ "$STATUS" = "200" ] && record "Delete empty namespace (HTTP 200)" PASS || record "Delete empty namespace (HTTP $STATUS, expected 200)" FAIL

echo
echo "========== NAMESPACE RESOURCE TESTS COMPLETE =========="
echo "Passed: $PASS_COUNT, Failed: $FAIL_COUNT"
[ "$FAIL_COUNT" -eq 0 ] && exit 0 || exit 1
