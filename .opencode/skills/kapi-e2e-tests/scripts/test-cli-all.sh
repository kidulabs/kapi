#!/bin/bash
# Test Area: All CLI Tests (C1-C33)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Running all CLI e2e tests..."
echo ""

# Run each CLI area in order
echo "=== Phase 1: CLI Get ==="
bash "$SCRIPT_DIR/test-cli-get.sh"
echo ""

echo "=== Phase 2: CLI Apply ==="
bash "$SCRIPT_DIR/test-cli-apply.sh"
echo ""

echo "=== Phase 3: CLI Delete ==="
bash "$SCRIPT_DIR/test-cli-delete.sh"
echo ""

echo "=== Phase 4: CLI Watch ==="
bash "$SCRIPT_DIR/test-cli-watch.sh"
echo ""

echo "=== Phase 5: CLI Status ==="
bash "$SCRIPT_DIR/test-cli-status.sh"
echo ""

echo "=== Phase 6: CLI Namespace ==="
bash "$SCRIPT_DIR/test-cli-namespace.sh"
echo ""

echo "=== Phase 7: CLI Output ==="
bash "$SCRIPT_DIR/test-cli-output.sh"
echo ""

echo "========== ALL CLI TESTS COMPLETE =========="
