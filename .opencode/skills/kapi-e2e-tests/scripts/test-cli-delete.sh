#!/bin/bash
# Test Area: CLI Delete (Tests C16-C18)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI DELETE TESTS =========="

# Setup: register schema and create test objects
register_widget_schema

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-del-target-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

# Create namespace-scoped object
curl -s -X POST "http://localhost:8080/apis/kapi.io/v1/Namespace" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-del-ns-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-del-ns-$TEST_RUN/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-del-namespaced-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":3}}" > /dev/null

sleep 1

# C16: Delete existing object
echo "========== TEST C16: Delete existing object =========="
OUTPUT=$($KAPI delete example.io.$TEST_RUN/Widget "cli-del-target-$TEST_RUN" 2>&1)
EXIT_CODE=$?
# Verify object is gone
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-del-target-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && [ "$HTTP_CODE" = "404" ]; then
  echo "C16_PASS: delete removed object successfully"
else
  echo "C16_FAIL: exit=$EXIT_CODE http_after_delete=$HTTP_CODE"
fi

# C17: Delete non-existent object returns error
echo "========== TEST C17: Delete non-existent object =========="
OUTPUT=$($KAPI delete example.io.$TEST_RUN/Widget "nonexistent-$TEST_RUN" 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ]; then
  echo "C17_PASS: delete non-existent returned error (exit=$EXIT_CODE)"
else
  echo "C17_FAIL: expected non-zero exit, got $EXIT_CODE output=$OUTPUT"
fi

# C18: Delete with namespace flag
echo "========== TEST C18: Delete with namespace =========="
OUTPUT=$($KAPI delete example.io.$TEST_RUN/Widget "cli-del-namespaced-$TEST_RUN" -n "cli-del-ns-$TEST_RUN" 2>&1)
EXIT_CODE=$?
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-del-ns-$TEST_RUN/Widget/cli-del-namespaced-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && [ "$HTTP_CODE" = "404" ]; then
  echo "C18_PASS: delete with namespace removed object"
else
  echo "C18_FAIL: exit=$EXIT_CODE http_after_delete=$HTTP_CODE"
fi

echo "========== CLI DELETE TESTS COMPLETE =========="
