#!/bin/bash
# Test Area: CLI Get (Tests C1-C8)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI GET TESTS =========="

# Setup: register schema and create test objects via API
register_widget_schema

# Create objects in default namespace
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-get-alpha-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"env\":\"prod\"}},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-get-beta-$TEST_RUN\",\"labels\":{\"app\":\"httpd\",\"env\":\"staging\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

# Create a namespace and object in it
curl -s -X POST "http://localhost:8080/apis/kapi.io/v1/Namespace" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-test-ns-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-test-ns-$TEST_RUN/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-get-gamma-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"green\",\"size\":3}}" > /dev/null

sleep 1

# C1: Get single object by name
echo "========== TEST C1: Get single object =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget "cli-get-alpha-$TEST_RUN" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-get-alpha-$TEST_RUN"; then
  echo "C1_PASS: get single object returned successfully"
else
  echo "C1_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C2: Get list of objects
echo "========== TEST C2: Get list =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-get-alpha" && echo "$OUTPUT" | grep -q "cli-get-beta"; then
  echo "C2_PASS: get list returned multiple objects"
else
  echo "C2_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C3: Get with label selector
echo "========== TEST C3: Get with label selector =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget -l "app=nginx" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-get-alpha" && ! echo "$OUTPUT" | grep -q "cli-get-beta"; then
  echo "C3_PASS: label selector filtered correctly"
else
  echo "C3_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C4: Get with JSON output
echo "========== TEST C4: Get with JSON output =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget "cli-get-alpha-$TEST_RUN" -o json 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
  echo "C4_PASS: JSON output is valid"
else
  echo "C4_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C5: Get with YAML output
echo "========== TEST C5: Get with YAML output =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget "cli-get-alpha-$TEST_RUN" -o yaml 2>&1)
EXIT_CODE=$?
# Check for YAML markers (key: value pairs, no JSON braces at start)
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "^key:" && ! echo "$OUTPUT" | grep -q "^{"; then
  echo "C5_PASS: YAML output is valid"
else
  echo "C5_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C6: Get with table output (default)
echo "========== TEST C6: Get with table output =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget 2>&1)
EXIT_CODE=$?
# Table should have NAME and NAMESPACE columns for namespaced objects
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -qi "NAME"; then
  echo "C6_PASS: table output has headers"
else
  echo "C6_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C7: Get with namespace flag
echo "========== TEST C7: Get with namespace flag =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget -n "cli-test-ns-$TEST_RUN" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-get-gamma" && ! echo "$OUTPUT" | grep -q "cli-get-alpha"; then
  echo "C7_PASS: namespace flag scoped correctly"
else
  echo "C7_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C8: Get all namespaces
echo "========== TEST C8: Get all namespaces =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget --all-namespaces 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-get-alpha" && echo "$OUTPUT" | grep -q "cli-get-gamma"; then
  echo "C8_PASS: all-namespaces returned objects from multiple namespaces"
else
  echo "C8_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

echo "========== CLI GET TESTS COMPLETE =========="
