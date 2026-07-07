#!/bin/bash
# Test Area: CLI Apply (Tests C9-C15)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

KAPI=${KAPI:-kapi}

echo "========== CLI APPLY TESTS =========="

# Setup: register schema
register_widget_schema

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

# C9: Apply creates new object from JSON file
echo "========== TEST C9: Apply creates from JSON =========="
cat > "$TMPDIR/create.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-new-$TEST_RUN"},
  "spec": {"color": "red", "size": 5}
}
EOF
OUTPUT=$($KAPI apply -f "$TMPDIR/create.json" 2>&1)
EXIT_CODE=$?
# Verify object was created
BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && echo "$BODY" | python3 -c "import sys,json; obj=json.load(sys.stdin); assert obj['spec']['color']=='red'" 2>/dev/null; then
  echo "C9_PASS: apply created object from JSON"
else
  echo "C9_FAIL: exit=$EXIT_CODE body=$BODY"
fi

# C10: Apply updates existing object from JSON file
echo "========== TEST C10: Apply updates existing =========="
cat > "$TMPDIR/update.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-new-$TEST_RUN"},
  "spec": {"color": "blue", "size": 20}
}
EOF
OUTPUT=$($KAPI apply -f "$TMPDIR/update.json" 2>&1)
EXIT_CODE=$?
BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && echo "$BODY" | python3 -c "import sys,json; obj=json.load(sys.stdin); assert obj['spec']['color']=='blue' and obj['spec']['size']==20" 2>/dev/null; then
  echo "C10_PASS: apply updated existing object"
else
  echo "C10_FAIL: exit=$EXIT_CODE body=$BODY"
fi

# C11: Apply from YAML file
echo "========== TEST C11: Apply from YAML =========="
cat > "$TMPDIR/yaml-apply.yaml" <<EOF
kind: Widget
apiVersion: example.io.$TEST_RUN/v1
metadata:
  name: cli-apply-yaml-$TEST_RUN
spec:
  color: green
  size: 7
EOF
OUTPUT=$($KAPI apply -f "$TMPDIR/yaml-apply.yaml" 2>&1)
EXIT_CODE=$?
BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-yaml-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && echo "$BODY" | python3 -c "import sys,json; obj=json.load(sys.stdin); assert obj['spec']['color']=='green'" 2>/dev/null; then
  echo "C11_PASS: apply created object from YAML"
else
  echo "C11_FAIL: exit=$EXIT_CODE body=$BODY"
fi

# C12: Apply preserves system fields
echo "========== TEST C12: Apply preserves system fields =========="
# Get current resourceVersion before apply
RV_BEFORE=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED_BEFORE=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")

cat > "$TMPDIR/preserve.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-new-$TEST_RUN"},
  "spec": {"color": "yellow", "size": 15}
}
EOF
$KAPI apply -f "$TMPDIR/preserve.json" > /dev/null 2>&1

BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN")
CREATED_AFTER=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
RV_AFTER=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

if [ "$CREATED_BEFORE" = "$CREATED_AFTER" ] && [ "$RV_AFTER" -gt "$RV_BEFORE" ] 2>/dev/null; then
  echo "C12_PASS: createdAt preserved, resourceVersion bumped"
else
  echo "C12_FAIL: created_before=$CREATED_BEFORE created_after=$CREATED_AFTER rv_before=$RV_BEFORE rv_after=$RV_AFTER"
fi

# C13: Apply merges labels additively
echo "========== TEST C13: Apply merges labels =========="
# First, set initial labels via API
RV=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
CREATED=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['createdAt'])")
UPDATED=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['updatedAt'])")

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"cli-apply-new-$TEST_RUN\",\"labels\":{\"existing\":\"keep\",\"app\":\"old\"}},\"system\":{\"resourceVersion\":$RV,\"createdAt\":\"$CREATED\",\"updatedAt\":\"$UPDATED\"},\"spec\":{\"color\":\"yellow\",\"size\":15}}" > /dev/null

cat > "$TMPDIR/labels.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-new-$TEST_RUN", "labels": {"new-label": "added"}},
  "spec": {"color": "yellow", "size": 15}
}
EOF
$KAPI apply -f "$TMPDIR/labels.json" > /dev/null 2>&1

BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN")
LABELS=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['metadata'].get('labels',{}))")
if echo "$LABELS" | grep -q "new-label" && echo "$LABELS" | grep -q "existing"; then
  echo "C13_PASS: labels merged additively"
else
  echo "C13_FAIL: labels=$LABELS"
fi

# C14: Apply merges annotations additively
echo "========== TEST C14: Apply merges annotations =========="
cat > "$TMPDIR/annotations.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-new-$TEST_RUN", "annotations": {"note": "from-apply"}},
  "spec": {"color": "yellow", "size": 15}
}
EOF
$KAPI apply -f "$TMPDIR/annotations.json" > /dev/null 2>&1

BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/default/Widget/cli-apply-new-$TEST_RUN")
ANNS=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin)['metadata'].get('annotations',{}))")
if echo "$ANNS" | grep -q "note"; then
  echo "C14_PASS: annotations merged additively"
else
  echo "C14_FAIL: annotations=$ANNS"
fi

# C15: Apply with namespace flag
echo "========== TEST C15: Apply with namespace =========="
# Create namespace first
curl -s -X POST "http://localhost:8080/apis/kapi.io/v1/Namespace" \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"cli-test-ns-$TEST_RUN\"},\"spec\":{\"annotations\":{}}}" > /dev/null
sleep 1
cat > "$TMPDIR/ns-apply.json" <<EOF
{
  "kind": "Widget",
  "apiVersion": "example.io.$TEST_RUN/v1",
  "metadata": {"name": "cli-apply-ns-$TEST_RUN"},
  "spec": {"color": "purple", "size": 1}
}
EOF
OUTPUT=$($KAPI apply -f "$TMPDIR/ns-apply.json" -n "cli-test-ns-$TEST_RUN" 2>&1)
EXIT_CODE=$?
BODY=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/namespaces/cli-test-ns-$TEST_RUN/Widget/cli-apply-ns-$TEST_RUN")
if [ $EXIT_CODE -eq 0 ] && echo "$BODY" | python3 -c "import sys,json; obj=json.load(sys.stdin); assert obj['metadata']['namespace']=='cli-test-ns-$TEST_RUN'" 2>/dev/null; then
  echo "C15_PASS: apply with namespace flag created in correct namespace"
else
  echo "C15_FAIL: exit=$EXIT_CODE body=$BODY"
fi

echo "========== CLI APPLY TESTS COMPLETE =========="
