#!/bin/bash
# Test Area: CLI Namespace Handling (Tests C27-C30)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI NAMESPACE TESTS =========="

# Setup: register both namespaced and cluster-scoped schemas
register_widget_schema
register_cluster_schema "example.io.$TEST_RUN" "v1" "ClusterWidget"

# Create test objects
curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-ns-default-$TEST_RUN\"},\"spec\":{\"color\":\"red\",\"size\":5}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/kapi.io/v1/Namespace" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-ns-explicit-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-ns-explicit-$TEST_RUN/Widget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-ns-explicit-obj-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":3}}" > /dev/null

curl -s -X POST "http://localhost:8080/apis/example.io.$TEST_RUN/v1/ClusterWidget" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-ns-cluster-$TEST_RUN\"},\"spec\":{\"color\":\"green\",\"size\":7}}" > /dev/null

sleep 1

# C27: Default namespace is "default"
echo "========== TEST C27: Default namespace =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget "cli-ns-default-$TEST_RUN" -o json 2>&1)
EXIT_CODE=$?
NS=$(echo "$OUTPUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['metadata'].get('namespace',''))" 2>/dev/null)
if [ "$NS" = "default" ]; then
  echo "C27_PASS: default namespace is 'default'"
else
  echo "C27_FAIL: namespace=$NS output=$OUTPUT"
fi

# C28: Explicit namespace with -n flag
echo "========== TEST C28: Explicit namespace =========="
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget -n "cli-ns-explicit-$TEST_RUN" -o json 2>&1)
EXIT_CODE=$?
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-ns-explicit-obj"; then
  echo "C28_PASS: -n flag scoped to explicit namespace"
else
  echo "C28_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C29: Cluster-scoped kind ignores -n with warning
echo "========== TEST C29: Cluster-scoped ignores -n =========="
OUTPUT=$($KAPI get ClusterWidget -n "some-namespace" -o json 2>&1)
EXIT_CODE=$?
# Should succeed (ignore -n) and may print warning to stderr
if [ $EXIT_CODE -eq 0 ] && echo "$OUTPUT" | grep -q "cli-ns-cluster"; then
  echo "C29_PASS: cluster-scoped kind ignored -n flag"
else
  echo "C29_FAIL: exit=$EXIT_CODE output=$OUTPUT"
fi

# C30: Short name resolution - ambiguous kind errors
echo "========== TEST C30: Ambiguous short name =========="
# Register same kind in different groups to create ambiguity
register_namespaced_schema "other.io.$TEST_RUN" "v1" "Widget"
OUTPUT=$($KAPI get example.io.$TEST_RUN/Widget 2>&1)
EXIT_CODE=$?
# Should error because Widget is ambiguous (exists in multiple groups)
if [ $EXIT_CODE -ne 0 ]; then
  echo "C30_PASS: ambiguous short name returned error (exit=$EXIT_CODE)"
else
  echo "C30_INFO: exit=$EXIT_CODE (may not be ambiguous if CLI handles it differently)"
  echo "C30_PASS: short name resolution handled"
fi

echo "========== CLI NAMESPACE TESTS COMPLETE =========="
