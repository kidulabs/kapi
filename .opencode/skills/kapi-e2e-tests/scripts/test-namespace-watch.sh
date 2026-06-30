#!/bin/bash
# Test Area: Namespace Watch (Tests 82-83)
# Verifies that watch with a namespace in the URL filters events by namespace,
# and cross-namespace watch (no namespace in URL) delivers events from all namespaces.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

GROUP="ns-watch.$TEST_RUN"
KIND="NamespacedWidget"
BASE="http://localhost:8080"
API="$BASE/apis/$GROUP/v1"
NS_API="$BASE/apis/kapi.io/v1/Namespace"

# Pre-create the namespaces the watch tests need
for ns in staging production ns-x; do
  curl -s -X POST "$NS_API" -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"$ns-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null
done

register_namespaced_schema "$GROUP" "v1" "$KIND"

echo "========== TEST 82: Watch Objects in a Specific Namespace =========="
# Watch events in the "staging" namespace only
WATCH_PID=$(curl -s -N "$API/namespaces/staging-$TEST_RUN/$KIND?watch=true" > /tmp/t82-watch.log 2>&1 & echo $!)
sleep 2

# Create object in staging — should be delivered
curl -s -X POST "$API/namespaces/staging-$TEST_RUN/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-test-staging-$TEST_RUN\"},\"spec\":{\"color\":\"cyan\",\"size\":5}}" > /dev/null
sleep 1

# Create object in production — should NOT be delivered to staging watcher
curl -s -X POST "$API/namespaces/production-$TEST_RUN/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"watch-test-prod-$TEST_RUN\"},\"spec\":{\"color\":\"magenta\",\"size\":3}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null

STAGING_COUNT=$(grep -c "watch-test-staging-$TEST_RUN" /tmp/t82-watch.log 2>/dev/null | head -1)
STAGING_COUNT=${STAGING_COUNT:-0}
PROD_COUNT=$(grep -c "watch-test-prod-$TEST_RUN" /tmp/t82-watch.log 2>/dev/null | head -1)
PROD_COUNT=${PROD_COUNT:-0}
echo "  staging events: $STAGING_COUNT, production events (should be 0): $PROD_COUNT"

STAGING_COUNT=$STAGING_COUNT PROD_COUNT=$PROD_COUNT python3 << EOF
import os
sc = int(os.environ["STAGING_COUNT"])
pc = int(os.environ["PROD_COUNT"])
assert sc >= 1, f"Expected at least 1 staging event, got {sc}"
assert pc == 0, f"Expected 0 production events in staging watcher, got {pc}"
print(f"PASS: Watch filtered by namespace (staging={sc}, prod={pc})")
EOF
echo "T82_PASS"

echo "========== TEST 83: Cross-Namespace Watch =========="
# Watch events across all namespaces (no namespace in URL)
WATCH_PID=$(curl -s -N "$API/$KIND?watch=true" > /tmp/t83-watch.log 2>&1 & echo $!)
sleep 2

curl -s -X POST "$API/namespaces/ns-x-$TEST_RUN/$KIND" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cross-ns-test-$TEST_RUN\"},\"spec\":{\"color\":\"magenta\",\"size\":3}}" > /dev/null
sleep 2

kill $WATCH_PID 2>/dev/null

CROSS_COUNT=$(grep -c "cross-ns-test-$TEST_RUN" /tmp/t83-watch.log 2>/dev/null | head -1)
CROSS_COUNT=${CROSS_COUNT:-0}
echo "  cross-ns events: $CROSS_COUNT"

CROSS_COUNT=$CROSS_COUNT python3 << EOF
import os
cc = int(os.environ["CROSS_COUNT"])
assert cc >= 1, f"Expected at least 1 cross-namespace event, got {cc}"
print(f"PASS: Cross-namespace watch received event (count={cc})")
EOF
echo "T83_PASS"

echo "========== NAMESPACE WATCH TESTS COMPLETE =========="

