#!/bin/bash
# Test Area: Annotations (Tests 38-40)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 38: Annotations create =========="
register_widget_schema

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"ann-with-$TEST_RUN\",\"annotations\":{\"description\":\"my widget\",\"owner\":\"team-platform\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"ann-without-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "With annotations:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/ann-with-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('annotations',{}))"
echo "Without annotations:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/ann-without-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('annotations',{}))"
echo "T38_PASS"

echo "========== TEST 39: Annotations update =========="
get_system_fields "ann-with-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/ann-with-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"ann-with-$TEST_RUN\",\"annotations\":{\"description\":\"new widget\",\"owner\":\"team\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "After update:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/ann-with-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('annotations',{}))"
echo "T39_PASS"

echo "========== TEST 40: Annotation validation =========="
LONG_KEY_JSON=$(python3 -c "import json;k='a'*257;print(json.dumps({k:'value'}))")

for case in "empty-key|{\"\":\"value\"}" "long-key|$LONG_KEY_JSON"; do
  IFS='|' read -r suffix annotations <<< "$case"
  CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"ann-${suffix}-$TEST_RUN\",\"annotations\":$annotations},\"spec\":{\"color\":\"blue\",\"size\":1}}")
  echo "Case $suffix: HTTP $CODE (expected 400)"
done
echo "T40_PASS"

echo "========== ANNOTATIONS TESTS COMPLETE =========="
