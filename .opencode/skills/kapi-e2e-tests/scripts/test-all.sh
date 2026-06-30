#!/bin/bash
# Test Area: All Tests (1-93)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Running all kapi e2e tests..."
echo ""

# Run each area in order
echo "=== Phase 1: Watch Basics ==="
bash "$SCRIPT_DIR/test-watch.sh"
echo ""

echo "=== Phase 2: Labels ==="
bash "$SCRIPT_DIR/test-labels.sh"
echo ""

echo "=== Phase 3: Annotations ==="
bash "$SCRIPT_DIR/test-annotations.sh"
echo ""

echo "=== Phase 4: Label Selectors ==="
bash "$SCRIPT_DIR/test-label-selectors.sh"
echo ""

echo "=== Phase 5: List Filtering ==="
bash "$SCRIPT_DIR/test-list-filtering.sh"
echo ""

echo "=== Phase 6: Status Subresource ==="
bash "$SCRIPT_DIR/test-status.sh"
echo ""

echo "=== Phase 7: Generation ==="
bash "$SCRIPT_DIR/test-generation.sh"
echo ""

echo "=== Phase 8: Finalizers ==="
bash "$SCRIPT_DIR/test-finalizers.sh"
echo ""

echo "=== Phase 9: Persistence ==="
bash "$SCRIPT_DIR/test-persistence.sh"
echo ""

echo "=== Phase 10: Concurrent & Failure ==="
bash "$SCRIPT_DIR/test-concurrent.sh"
echo ""

echo "=== Phase 11: Schema Scope ==="
bash "$SCRIPT_DIR/test-schema-scope.sh"
echo ""

echo "=== Phase 12: Namespace Resource (NEW) ==="
bash "$SCRIPT_DIR/test-namespace-resource.sh"
echo ""

echo "=== Phase 13: Namespace CRUD ==="
bash "$SCRIPT_DIR/test-namespace-crud.sh"
echo ""

echo "=== Phase 14: Cross-Namespace List ==="
bash "$SCRIPT_DIR/test-cross-namespace.sh"
echo ""

echo "=== Phase 15: Cluster-Scoped Resources ==="
bash "$SCRIPT_DIR/test-cluster-scoped.sh"
echo ""

echo "=== Phase 16: Scope Validation ==="
bash "$SCRIPT_DIR/test-scope-validation.sh"
echo ""

echo "=== Phase 17: Namespace Watch ==="
bash "$SCRIPT_DIR/test-namespace-watch.sh"
echo ""

echo "========== ALL TESTS COMPLETE =========="

