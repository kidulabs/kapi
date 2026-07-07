#!/bin/bash
# Test Area: CLI Output Formats (Tests C31-C33)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI OUTPUT FORMAT TESTS =========="

# Setup: register schema and create test objects
register_widget_schema

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-out-alpha-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-out-beta-$TEST_RUN\",\"labels\":{\"app\":\"httpd\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

sleep 1

# C31: Table output has correct columns
echo "========== TEST C31: Table columns =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget 2>&1)
EXIT_CODE=$?
# Namespaced objects should have NAME, NAMESPACE, AGE columns
HAS_NAME=$(echo "$OUTPUT" | head -1 | grep -ci "NAME" || true)
HAS_NAME=${HAS_NAME:-0}
HAS_NS=$(echo "$OUTPUT" | head -1 | grep -ci "NAMESPACE" || true)
HAS_NS=${HAS_NS:-0}
HAS_AGE=$(echo "$OUTPUT" | head -1 | grep -ci "AGE" || true)
HAS_AGE=${HAS_AGE:-0}

if [ "$HAS_NAME" -gt 0 ] && [ "$HAS_NS" -gt 0 ] && [ "$HAS_AGE" -gt 0 ]; then
  echo "C31_PASS: table has NAME, NAMESPACE, AGE columns"
else
  echo "C31_FAIL: columns missing. Header: $(echo "$OUTPUT" | head -1)"
fi

# C32: JSON output is valid JSON
echo "========== TEST C32: JSON validity =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget -o json 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | python3 -c "import sys,json; data=json.load(sys.stdin); print(f'items: {len(data.get(\"items\", [data]) if isinstance(data, dict) else data)}')" 2>/dev/null; then
  echo "C32_PASS: JSON output is valid"
else
  echo "C32_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C33: YAML output is valid YAML
echo "========== TEST C33: YAML validity =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget -o yaml 2>&1)
EXIT_CODE=$?
# Check for YAML list markers (starts with "- key:" for list output)
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "^- key:"; then
  echo "C33_PASS: YAML output is valid"
else
  echo "C33_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

echo "========== CLI OUTPUT FORMAT TESTS COMPLETE =========="
