---
name: skill-creator
description: "Capture domain knowledge, patterns, and workflows from the current session and create a reusable OpenCode skill. Triggers: create skill, capture knowledge, make a skill, save pattern, remember workflow, save this as a skill, create a reusable skill, save domain knowledge, skill from this, capture this workflow, turn this into a skill, save as skill, create skill from conversation"
---

# Skill Creator

> Capture hard-won knowledge from the current session and package it into a reusable OpenCode skill so neither you nor the user repeats the same hoops.

## When to Use

Use when the orchestrator recognizes **any** of these during a session:

| Signal | Example |
|--------|---------|
| **Recurring pattern** | Same debugging steps done twice |
| **Hard-won fix** | Took multiple iterations to get right |
| **Complex workflow** | Multi-step procedure easy to get wrong |
| **Project convention** | Architecture decisions, naming, layout |
| **API integration** | How to use a library/SDK correctly |
| **Test harness** | How to run/verify tests for a subsystem |
| **Debugging procedure** | Diagnostic steps for common failures |
| **Configuration setup** | Tooling, environment, build setup |

**Also use when the user explicitly asks** to save something as a skill.

## Workflow

### Phase 1: Recognize & Get Buy-in

When you detect a capture opportunity, briefly note it and ask:

> "This pattern (describe what) would be worth saving as a reusable skill. Shall I create one?"

If the user says yes or the request is explicit, proceed.

### Phase 2: Extract Essential Knowledge

Distill the session down to durable knowledge — strip session-specific details (timestamps, temp paths, specific test run IDs, etc.):

```
What was the problem?          → Root cause, not symptoms
What steps solved it?          → Abstracted workflow, not literal commands
What made it hard?             → Gotchas, edge cases, silent failures
What patterns emerged?         → Reusable abstractions
What should NOT be done?       → Dead ends, anti-patterns
What context does it need?     → Prerequisites, assumptions
```

### Phase 3: Name & Scope

Choose a name that is:
- **Short** (lowercase, hyphen-separated, < 40 chars)
- **Descriptive** (tells what domain it covers)
- **Specific** (narrow enough to be actionable)

**Good:** `domain-web`, `kapi-e2e-tests`, `m06-error-handling`
**Bad:** `rust-tips`, `general-advice`, `my-utility`

Write the trigger description — list keywords/phrases a user might say that should trigger this skill.

### Phase 4: Generate the Skill

Use this template:

```markdown
---
name: <skill-name>
description: "<1-line description with trigger keywords. Triggers: <keyword>, <keyword>, ...>"
---

# <Skill Name>

> <1-2 sentence elevator pitch — what problem this skill solves>

## When to Use

<Bullet list of scenarios>

## Prerequisites

<What must be true before following this workflow>

## Workflow

### Step 1: <Title>

```
<Commands or instructions>
```

### Step 2: <Title>

...

## Important Notes

<Gotchas, edge cases, things that are easy to get wrong>

## DO NOT

<Anti-patterns, common mistakes>

## Error Handling

| Symptom | Cause | Fix |
|---------|-------|-----|
| <Error> | <Why> | <Solution> |
```

#### Key rules:

- **Frontmatter is required** — OpenCode won't detect the skill without `name:` and `description:` in the YAML frontmatter
- **Triggers go in the description** — list keywords after `Triggers:` so the system matches user intent
- **Use `context: fork`** if the skill needs its own agent context (for complex multi-step skills)
- **Prefer `context: fork` with `agent: general-purpose`** for non-trivial skills — this prevents context pollution
- **Scripts go in `scripts/`** subdirectory when the workflow involves bash commands

### Phase 5: Create Files

```bash
# Create skill directory
SKILL_NAME="<name>"
mkdir -p ".opencode/skills/$SKILL_NAME"
mkdir -p ".opencode/skills/$SKILL_NAME/scripts"

# Write SKILL.md (use the Write tool with content from Phase 4)

# Create scripts (if needed)
# Write helper scripts to .opencode/skills/$SKILL_NAME/scripts/

# Make scripts executable
chmod +x .opencode/skills/$SKILL_NAME/scripts/*.sh
```

### Phase 6: Verify

Check that the skill is valid:

```bash
# 1. File exists
ls .opencode/skills/<name>/SKILL.md

# 2. Frontmatter is valid
head -5 .opencode/skills/<name>/SKILL.md
# Should start with ---, have name: and description:

# 3. Scripts are executable (if any)
ls -la .opencode/skills/<name>/scripts/

# 4. Directory tree is clean
find .opencode/skills/<name>/ -type f | sort
```

## Anatomy of a Good Skill

```
.opencode/skills/<name>/
├── SKILL.md                         # Required — skill definition
└── scripts/                         # Optional — helper scripts
    ├── common.sh                    # Shared helpers
    └── test-<area>.sh              # Workflow automation
```

### SKILL.md frontmatter

```yaml
---
name: <hyphenated-name>             # Required — used as skill identifier
description: "<trigger phrase>"      # Required — used for auto-matching
argument-hint: "[options]"           # Optional — for command-style skills
context: fork                        # Recommended for multi-step skills
agent: general-purpose               # Agent type (general-purpose default)
---
```

### Good triggers (description field)

Include these patterns for better matching:
- **Action phrases:** `"run tests"`, `"create widget"`, `"deploy service"`
- **Domain terms:** `"CRUD"`, `"database"`, `"authentication"`, `"watch semantics"`
- **Problem symptoms:** `"ownership error"`, `"lifetime issue"`, `"compilation error"`
- **Feature names:** `"finalizers"`, `"label selectors"`, `"status subresource"`

## Examples

### Example 1: From a test session → test skill

**Session context:** Ran 51 kapi e2e tests manually, learned how tests depend on each other, discovered server setup/teardown patterns, found which tests need SQLite vs in-memory.

**Captured skill:** `kapi-e2e-tests` — organized test areas, reusable scripts, SKILL.md with triggers for each area.

### Example 2: From debugging a Rust ownership issue

**Session context:** Spent time debugging E0382, traced through multiple functions, identified the exact pattern that causes the issue.

**Captured skill:** Would create a skill documenting the specific ownership pattern, with before/after code, common fixes, and related error codes.

### Example 3: From a complex Cargo workspace setup

**Session context:** Set up workspace members, shared dependencies, feature flags, conditional compilation.

**Captured skill:** Would create a skill with workspace templates, common patterns, dependency management guidelines.

## DO NOT

- Create a skill for one-off tasks that won't repeat
- Include session-specific details (timestamps, PIDs, temp paths)
- Create skills that are too broad to be actionable
- Skip the frontmatter — the skill won't be detected
- Duplicate existing skills — check `.opencode/skills/` and `~/.config/opencode/skills/` first
- Create skills for general programming knowledge that belongs in the LLM's training data
- Nest skills too deeply — flat structure is better

## Error Handling

| Symptom | Cause | Fix |
|---------|-------|-----|
| Skill not detected after restart | Missing frontmatter | Add `name:` and `description:` |
| Skill not auto-triggering | Wrong trigger keywords | Update description with better keywords |
| Scripts fail | Wrong paths in scripts | Use `SCRIPT_DIR` pattern for portability |
| `Argument list too long` | Shell expansion limit | Use stdin or temp files for large payloads |
| Skill too vague | Scope too broad | Narrow to one specific workflow |
