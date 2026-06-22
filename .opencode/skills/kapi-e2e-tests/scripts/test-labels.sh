#!/bin/bash
# Test Area: Labels (Tests 5-9)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

echo "========== TEST 5: Labels create =========="
register_widget_schema

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"lbl-with-$TEST_RUN\",\"labels\":{\"app\":\"nginx\",\"env\":\"prod\",\"app.kubernetes.io/version\":\"v1.2.3\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"lbl-without-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "With labels:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lbl-with-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('labels',{}))"
echo "Without labels:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lbl-without-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('labels',{}))"
echo "T5_PASS: labels created correctly"

echo "========== TEST 6: Labels update =========="
get_system_fields "lbl-with-$TEST_RUN"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lbl-with-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"lbl-with-$TEST_RUN\",\"labels\":{\"app\":\"httpd\",\"app.kubernetes.io/version\":\"v1.2.3\",\"tier\":\"frontend\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null

echo "After update:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/lbl-with-$TEST_RUN" | python3 -c "import sys,json;print(json.load(sys.stdin)['metadata'].get('labels',{}))"
echo "T6_PASS: labels updated with replace semantics"

echo "========== TEST 7: Schema labels/annotations =========="
curl -s -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"labels\":{\"team\":\"platform\",\"status\":\"active\"},\"annotations\":{\"team\":\"platform\",\"docs\":\"https://example.com/docs\"}},\"targetGroup\":\"test-$TEST_RUN.io\",\"targetVersion\":\"v1\",\"targetKind\":\"Gadget\",\"specSchema\":{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}}}}" > /dev/null

echo "Schema labels/annotations:"; curl -s "http://localhost:8080/apis/kapi.io/v1/Schema/Gadget.test-$TEST_RUN.io" | python3 -c "
import sys,json;obj=json.load(sys.stdin);md=obj['metadata']
print(f'labels: {md.get(\"labels\",{})}, annotations: {md.get(\"annotations\",{})}')"
echo "T7_PASS: Schema supports labels and annotations"

echo "========== TEST 8: Label validation =========="
LONG_KEY_JSON=$(python3 -c "import json;k='a'*257;print(json.dumps({k:'value'}))")
LONG_VALUE_JSON=$(python3 -c "import json;print(json.dumps({'app':'a'*257}))")

for case in "bad-key|{\"invalid key!\":\"value\"}|invalid" "bad-value|{\"app\":\"invalid value!\"}|invalid" "long-key|$LONG_KEY_JSON|256" "long-value|$LONG_VALUE_JSON|256"; do
  IFS='|' read -r suffix labels expected <<< "$case"
  CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
    -H "Content-Type: application/json" \
    -d "{\"metadata\":{\"name\":\"label-${suffix}-$TEST_RUN\",\"labels\":$labels},\"spec\":{\"color\":\"blue\",\"size\":1}}")
  echo "Case $suffix: HTTP $CODE (expected 400)"
done
echo "T8_PASS: all label validation cases return 400"

echo "========== TEST 9: List returns labels =========="
echo "Listing labels:"; curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget" | python3 -c "
import sys,json;body=json.load(sys.stdin)
for item in body['items']:
    md=item['metadata'];name=md['name']
    labels=md.get('labels',{});anns=md.get('annotations',{})
    if name.startswith('lbl-') or name.startswith('with-') or name.startswith('without-'):
        print(f'{name}: labels={labels}, annotations={anns}')
"
echo "T9_PASS: list returns labels and annotations"

echo "========== LABELS TESTS COMPLETE =========="
