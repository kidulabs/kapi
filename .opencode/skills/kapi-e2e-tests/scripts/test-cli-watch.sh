#!/bin/bash
# Test Area: CLI Watch (Tests C19-C22)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI WATCH TESTS =========="

# Setup: register schema
register_widget_schema

# Create a namespace for scoping tests
curl -s -X POST "http://localhost:8080/apis/kapi.io/v1/Namespace" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-ns-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null

sleep 1

# C19: Basic watch streams events
echo "========== TEST C19: Basic watch =========="
WATCH_LOG=$(mktemp)
$KAPI watch example.io.$TEST_RUN/Widget > "$WATCH_LOG" 2>&1 &
WATCH_PID=$!
sleep 1

# Create an object to trigger an event
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-target-$TEST_RUN\",\"labels\":{\"app\":\"watcher\"}},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

sleep 2
kill $WATCH_PID 2>/dev/null || true
wait $WATCH_PID 2>/dev/null || true

if grep -q "cli-watch-target-$TEST_RUN" "$WATCH_LOG" 2>/dev/null; then
  echo "C19_PASS: watch received event for created object"
else
  echo "C19_FAIL: no event found in watch log"
  cat "$WATCH_LOG" 2>/dev/null
fi
rm -f "$WATCH_LOG"

# C20: Watch with label selector filter
echo "========== TEST C20: Watch with label selector =========="
WATCH_LOG=$(mktemp)
$KAPI watch example.io.$TEST_RUN/Widget -l "app=watcher" > "$WATCH_LOG" 2>&1 &
WATCH_PID=$!
sleep 1

# Create matching object
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-match-$TEST_RUN\",\"labels\":{\"app\":\"watcher\"}},\"spec\":{\"color\":\"blue\",\"size\":3}}" > /dev/null

# Create non-matching object
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-nomatch-$TEST_RUN\",\"labels\":{\"app\":\"other\"}},\"spec\":{\"color\":\"green\",\"size\":7}}" > /dev/null

sleep 2
kill $WATCH_PID 2>/dev/null || true
wait $WATCH_PID 2>/dev/null || true

HAS_MATCH=$(grep -c "cli-watch-match-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_MATCH=${HAS_MATCH:-0}
HAS_NOMATCH=$(grep -c "cli-watch-nomatch-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_NOMATCH=${HAS_NOMATCH:-0}

if [ "$HAS_MATCH" -gt 0 ] && [ "$HAS_NOMATCH" -eq 0 ]; then
  echo "C20_PASS: label selector filtered watch events"
else
  echo "C20_FAIL: match=$HAS_MATCH nomatch=$HAS_NOMATCH"
  cat "$WATCH_LOG" 2>/dev/null
fi
rm -f "$WATCH_LOG"

# C21: Watch with namespace scoping
echo "========== TEST C21: Watch with namespace =========="
WATCH_LOG=$(mktemp)
$KAPI watch example.io.$TEST_RUN/Widget -n "cli-watch-ns-$TEST_RUN" > "$WATCH_LOG" 2>&1 &
WATCH_PID=$!
sleep 1

# Create in watched namespace
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-watch-ns-$TEST_RUN/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-in-ns-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":1}}" > /dev/null

# Create in different namespace (should NOT appear)
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-other-ns-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":2}}" > /dev/null

sleep 2
kill $WATCH_PID 2>/dev/null || true
wait $WATCH_PID 2>/dev/null || true

HAS_IN_NS=$(grep -c "cli-watch-in-ns-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_IN_NS=${HAS_IN_NS:-0}
HAS_OTHER_NS=$(grep -c "cli-watch-other-ns-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_OTHER_NS=${HAS_OTHER_NS:-0}

if [ "$HAS_IN_NS" -gt 0 ] && [ "$HAS_OTHER_NS" -eq 0 ]; then
  echo "C21_PASS: watch scoped to namespace"
else
  echo "C21_FAIL: in_ns=$HAS_IN_NS other_ns=$HAS_OTHER_NS"
  cat "$WATCH_LOG" 2>/dev/null
fi
rm -f "$WATCH_LOG"

# C22: Watch all namespaces
echo "========== TEST C22: Watch all namespaces =========="
WATCH_LOG=$(mktemp)
$KAPI watch example.io.$TEST_RUN/Widget --all-namespaces > "$WATCH_LOG" 2>&1 &
WATCH_PID=$!
sleep 1

# Create in default namespace
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-all-def-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":1}}" > /dev/null

# Create in other namespace
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-watch-ns-$TEST_RUN/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-watch-all-other-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":2}}" > /dev/null

sleep 2
kill $WATCH_PID 2>/dev/null || true
wait $WATCH_PID 2>/dev/null || true

HAS_DEF=$(grep -c "cli-watch-all-def-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_DEF=${HAS_DEF:-0}
HAS_OTHER=$(grep -c "cli-watch-all-other-$TEST_RUN" "$WATCH_LOG" 2>/dev/null || true)
HAS_OTHER=${HAS_OTHER:-0}

if [ "$HAS_DEF" -gt 0 ] && [ "$HAS_OTHER" -gt 0 ]; then
  echo "C22_PASS: all-namespaces watch received events from multiple namespaces"
else
  echo "C22_FAIL: default=$HAS_DEF other=$HAS_OTHER"
  cat "$WATCH_LOG" 2>/dev/null
fi
rm -f "$WATCH_LOG"

echo "========== CLI WATCH TESTS COMPLETE =========="
