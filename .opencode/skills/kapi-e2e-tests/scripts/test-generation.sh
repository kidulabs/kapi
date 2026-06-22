#!/bin/bash
# Test Area: Generation (Tests 33-37)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

register_widget_schema_with_status

echo "========== TEST 33: Generation starts at 1 =========="
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-create-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")

echo "$CREATE_RESP" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
gen=obj['system']['generation']; rv=obj['system']['resourceVersion']
print(f'generation: {gen}, resourceVersion: {rv}')
assert gen==1,'generation should be 1 on create'
print('PASS: generation starts at 1')
"
echo "T33_PASS"

echo "========== TEST 34: Metadata update does NOT bump generation =========="
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"spec\":{\"color\":\"blue\",\"size\":10}}")
INITIAL_GEN=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
INITIAL_RV=$(echo "$CREATE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
get_system_fields "gen-meta-$TEST_RUN"

echo "Initial: generation=$INITIAL_GEN, resourceVersion=$INITIAL_RV"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"system\":{\"resourceVersion\":$INITIAL_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /tmp/gen-meta-update.json

cat /tmp/gen-meta-update.json | python3 -c "
import sys,json;obj=json.load(sys.stdin)
gen=obj['system']['generation']; rv=obj['system']['resourceVersion']
print(f'generation: {gen}, resourceVersion: {rv}')
assert gen==$INITIAL_GEN,f'generation should stay $INITIAL_GEN, got {gen}'
assert rv>$INITIAL_RV,f'resourceVersion should bump'
print('PASS: generation unchanged, resourceVersion bumped')
"
echo "T34_PASS"

echo "========== TEST 35: Spec change bumps generation =========="
CURRENT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")
get_system_fields "gen-meta-$TEST_RUN"

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"gen-meta-$TEST_RUN\",\"labels\":{\"env\":\"prod\"}},\"system\":{\"resourceVersion\":$BEFORE_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":10}}" > /tmp/gen-spec-update.json

cat /tmp/gen-spec-update.json | python3 -c "
import sys,json;obj=json.load(sys.stdin)
gen=obj['system']['generation']; rv=obj['system']['resourceVersion']
print(f'generation: {gen}, resourceVersion: {rv}')
assert gen==$BEFORE_GEN+1,f'generation should bump to {$BEFORE_GEN+1}'
assert rv>$BEFORE_RV,f'resourceVersion should bump'
print('PASS: both generation and resourceVersion bumped')
"
echo "T35_PASS"

echo "========== TEST 36: Status update does NOT bump generation =========="
CURRENT=$(curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN")
BEFORE_GEN=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['generation'])")
BEFORE_RV=$(echo "$CURRENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['system']['resourceVersion'])")

echo "Before: generation=$BEFORE_GEN, resourceVersion=$BEFORE_RV"

curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-meta-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}' > /tmp/gen-status-update.json

cat /tmp/gen-status-update.json | python3 -c "
import sys,json;obj=json.load(sys.stdin)
gen=obj['system']['generation']; rv=obj['system']['resourceVersion']
print(f'generation: {gen}, resourceVersion: {rv}')
assert gen==$BEFORE_GEN,f'generation should stay $BEFORE_GEN'
assert rv>$BEFORE_RV,f'resourceVersion should bump'
print('PASS: generation unchanged, resourceVersion bumped on status update')
"
echo "T36_PASS"

echo "========== TEST 37: Independent counters =========="
CREATE_RESP=$(curl -s -X POST http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget \
  -H "Content-Type: application/json" \
  -d "{\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\"},\"spec\":{\"color\":\"blue\",\"size\":10}}")
echo "Step 0 CREATE:"
echo "$CREATE_RESP" | python3 -c "import sys,json;obj=json.load(sys.stdin);print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')"

# Step 1: metadata only
get_system_fields "gen-indep-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"blue\",\"size\":10}}" > /dev/null
echo "Step 1 UPDATE labels:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "import sys,json;obj=json.load(sys.stdin);print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')"

# Step 2: spec change
get_system_fields "gen-indep-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"nginx\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
echo "Step 2 UPDATE spec:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "import sys,json;obj=json.load(sys.stdin);print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')"

# Step 3: status update
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN/status" \
  -H "Content-Type: application/json" \
  -d '{"status":{"phase":"Running"}}' > /dev/null
echo "Step 3 UPDATE status:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "import sys,json;obj=json.load(sys.stdin);print(f'  generation={obj[\"system\"][\"generation\"]}, resourceVersion={obj[\"system\"][\"resourceVersion\"]}')"

# Step 4: labels again
get_system_fields "gen-indep-$TEST_RUN"
curl -s -X PUT "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" \
  -H "Content-Type: application/json" \
  -d "{\"key\":{\"group\":\"example.io.$TEST_RUN\",\"version\":\"v1\",\"kind\":\"Widget\"},\"metadata\":{\"name\":\"gen-indep-$TEST_RUN\",\"labels\":{\"app\":\"httpd\",\"env\":\"prod\"}},\"system\":{\"resourceVersion\":$GET_RV,\"createdAt\":\"$GET_CREATED\",\"updatedAt\":\"$GET_UPDATED\"},\"spec\":{\"color\":\"red\",\"size\":20}}" > /dev/null
echo "Step 4 UPDATE labels again:"
curl -s "http://localhost:8080/apis/example.io.$TEST_RUN/v1/Widget/gen-indep-$TEST_RUN" | python3 -c "
import sys,json;obj=json.load(sys.stdin)
gen=obj['system']['generation']; rv=obj['system']['resourceVersion']
print(f'  generation={gen}, resourceVersion={rv}')
assert gen==2,f'generation should be 2 (only 1 spec change), got {gen}'
assert rv==5,f'resourceVersion should be 5 (create + 4 updates), got {rv}'
print('PASS: generation=2 (spec-only), resourceVersion=5 (all changes)')
"
echo "T37_PASS"

echo "========== GENERATION TESTS COMPLETE =========="
