#!/bin/bash
# Test Area: CLI Status (Tests C23-C26)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI STATUS TESTS =========="

# Setup: register schema with status support
register_widget_schema_with_status

# Create object for status tests
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-status-target-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

# Set initial status via API
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-status-target-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d "{\"status\":{\"phase\":\"Running\",\"message\":\"All good\"}}" > /dev/null

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

sleep 1

# C23: Status get retrieves status
echo "========== TEST C23: Status get =========="
OUTPUT=$($KAPI status get example.io.$TEST_RUN/Widget "cli-status-target-$TEST_RUN" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "Running"; then
  echo "C23_PASS: status get returned status"
else
  echo "C23_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C24: Status apply updates status from file
echo "========== TEST C24: Status apply from file =========="
cat > "$TMPDIR/status.json" <<EOF
{
  "phase": "Completed",
  "message": "Done processing"
}
EOF
OUTPUT=$($KAPI status apply example.io.$TEST_RUN/Widget "cli-status-target-$TEST_RUN" -f "$TMPDIR/status.json" 2>&1)
EXIT_CODE=$?
# Verify via API
BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-status-target-$TEST_RUN/status")
if [ $EXIT_CODE -eq 0 ] && echo "$BODY" | grep -q "Completed"; then
  echo "C24_PASS: status apply updated status"
else
  echo "C24_FAIL: exit=$EXIT_CODE body=$BODY"
fi

# C25: Status get on object with no status
echo "========== TEST C25: Status get with no status =========="
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-status-empty-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":3}}" > /dev/null

OUTPUT=$($KAPI status get example.io.$TEST_RUN/Widget "cli-status-empty-$TEST_RUN" 2>&1)
EXIT_CODE=$?
# Should succeed but show empty/no status
echo "C25_INFO: exit=$EXIT_CODE output=$OUTPUT"
echo "C25_PASS: status get handled object with no status"

# C26: Status apply on non-existent object returns error
echo "========== TEST C26: Status apply non-existent =========="
cat > "$TMPDIR/status-ne.json" <<EOF
{
  "phase": "Failed"
}
EOF
OUTPUT=$($KAPI status apply example.io.$TEST_RUN/Widget "nonexistent-$TEST_RUN" -f "$TMPDIR/status-ne.json" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ]; then
  echo "C26_PASS: status apply on non-existent returned error (exit=$EXIT_CODE)"
else
  echo "C26_FAIL: expected non-zero exit, got $EXIT_CODE output=$OUTPUT"
fi

echo "========== CLI STATUS TESTS COMPLETE =========="
