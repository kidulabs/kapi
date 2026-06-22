#!/bin/bash
# Scaffold a new skill directory in the project's .opencode/skills/
# Usage: bash scaffold.sh <skill-name> "<description with triggers>"
#
# Example:
#   bash scaffold.sh my-domain "Work with MyDomain. Triggers: mydomain, my domain, md"

set -euo pipefail

SKILL_NAME="${1:?"Usage: $0 <skill-name> \"<description>\""}"
DESCRIPTION="${2:?"Usage: $0 <skill-name> \"<description>\""}"

# Determine project root (look for .opencode/skills relative to this script)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$SCRIPT_DIR/.."
PROJECT_ROOT="$(cd "$SKILLS_DIR/../.." && pwd)"

TARGET="$PROJECT_ROOT/.opencode/skills/$SKILL_NAME"

if [ -d "$TARGET" ]; then
  echo "ERROR: Skill '$SKILL_NAME' already exists at $TARGET"
  exit 1
fi

mkdir -p "$TARGET/scripts"

cat > "$TARGET/SKILL.md" << SKILLEOF
---
name: $SKILL_NAME
description: "$DESCRIPTION"
---

# ${SKILL_NAME}

> TODO: Add elevator pitch

## When to Use

- TODO

## Prerequisites

- TODO

## Workflow

### Step 1: TODO

\`\`\`
TODO
\`\`\`

## Important Notes

- TODO

## DO NOT

- TODO

## Error Handling

| Symptom | Cause | Fix |
|---------|-------|-----|
| TODO | TODO | TODO |
SKILLEOF

cat > "$TARGET/scripts/common.sh" << COMMONEOF
#!/bin/bash
# Common helpers for $SKILL_NAME
# Source this at the start of each script:
# SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
# source "\$SCRIPT_DIR/common.sh"

check_prerequisites() {
  echo "TODO: implement prerequisites check"
}
COMMONEOF

chmod +x "$TARGET/scripts/common.sh"

echo "Created skill: $TARGET"
echo "  SKILL.md         - Skill definition (edit this)"
echo "  scripts/common.sh - Helper script template"
echo ""
echo "Next steps:"
echo "  1. Edit SKILL.md with the actual workflow"
echo "  2. Add scripts to scripts/ if needed"
echo "  3. Verify: head -5 $TARGET/SKILL.md"
