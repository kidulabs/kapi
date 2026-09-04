---
name: cmd-openspec-apply
description: "Core workflow command: iteratively implement, review, and fix an OpenSpec change until all CRITICAL issues are resolved. Triggers on: /cmd-openspec-apply, drive change, implement and review, apply with review"
---

# cmd-openspec-apply

Core workflow command that iteratively drives an OpenSpec change to completion through repeated cycles of implementation, code review, spec compliance review, and fixes.

## When to Use

Use this command when you want to:
- Implement a change and ensure it passes both code quality and spec compliance reviews
- Iteratively fix issues until the change is production-ready
- Automate the implement → review → fix loop

## Inputs

| Argument | Description | Default |
|----------|-------------|---------|
| `<change-name>` | Name of the active change to implement | Auto-detect active change |
| `--max-iterations <n>` | Maximum review-fix cycles before stopping | 5 |
| `--skip-code-review` | Skip code quality review | false |
| `--skip-spec-review` | Skip spec compliance review | false |

## Workflow

### Phase 1: Detect Active Change

```bash
openspec list --json
```

If exactly one active change exists, use it. If multiple, ask the user. If none, exit with error.

### Phase 2: Implementation Loop

Repeat the following cycle up to `--max-iterations` times:

#### Step 2a: Apply Remaining Tasks

Invoke the `openspec-apply-change` skill:
```
/openspec-apply-change <change-name>
```

This implements all pending tasks from `tasks.md`.

#### Step 2b: Code Quality Review

If `--skip-code-review` is NOT set:

Invoke the `code-review` skill:
```
/code-review
```

This reviews the code against 17 quality dimensions (correctness, security, performance, etc.).

**Capture the report** and extract:
- List of CRITICAL issues
- List of WARNING issues
- List of SUGGESTION issues

#### Step 2c: Spec Compliance Review

If `--skip-spec-review` is NOT set:

Invoke the `spec-compliance-review` skill:
```
/spec-compliance-review --change <change-name>
```

This reviews the implementation against the change's specs and design.

**Capture the report** and extract:
- List of CRITICAL violations
- List of WARNING violations
- List of SUGGESTION violations

#### Step 2d: Consolidate Findings

Merge both review reports into a unified list:

```markdown
## Consolidated Review Report (Iteration N)

### CRITICAL (Must Fix)
- [code-review] `file.rs:42` — Description
- [spec-review] `file.rs:88` — Description

### WARNING (Should Fix)
- [code-review] `file.rs:120` — Description
- [spec-review] `file.rs:150` — Description

### SUGGESTION (Nice to Fix)
- [code-review] `file.rs:200` — Description
```

#### Step 2e: Fix CRITICAL and WARNING Issues

If there are CRITICAL or WARNING issues:

1. Dispatch fixer subagents to address all CRITICAL and WARNING issues
2. Run `cargo check`, `cargo test`, `cargo clippy` to verify fixes compile and pass
3. **IMPORTANT: Do NOT stop here. The fixer's self-verification is not sufficient.**
4. Continue to Step 2f to re-run reviews and independently verify the fixes

#### Step 2f: Re-Review and Check Loop Condition

**CRITICAL: You MUST re-run both reviews after fixes to verify they actually resolved the issues.**

The fixer's claim that issues are fixed is not independent verification. You must:

1. Re-run the code quality review (Step 2b) on the updated code
2. Re-run the spec compliance review (Step 2c) on the updated code
3. Consolidate the new findings (Step 2d)
4. Check if any CRITICAL issues remain in the NEW review reports

**Stop if:**
- The NEW review reports show zero CRITICAL issues (success!)
- Reached `--max-iterations` (warn user)
- User interrupts (Ctrl+C)

**Continue if:**
- The NEW review reports still show CRITICAL issues AND iterations < max
- Fix the remaining CRITICALs and loop back to Step 2f (re-review again)

**Why re-review is mandatory:**
- The fixer may claim an issue is fixed but the fix could be incomplete or incorrect
- Fixes can introduce new issues or regressions
- Independent verification by a fresh review is the only way to confirm the fixes actually work
- The loop condition ("no CRITICAL issues remain") can only be evaluated by running the reviews, not by trusting the fixer's output

### Phase 3: Final Report

After the loop exits, generate a final report:

```markdown
## cmd-openspec-apply Complete

**Change**: <change-name>
**Iterations**: N
**Final Status**: SUCCESS | MAX_ITERATIONS_REACHED

### Summary
- Tasks completed: X/Y
- Code review issues: A CRITICAL, B WARNING, C SUGGESTION
- Spec compliance issues: D CRITICAL, E WARNING, F SUGGESTION
- Fixes applied: G

### Remaining Issues
[List any remaining WARNING/SUGGESTION issues that were not fixed]

### Next Steps
1. Review remaining WARNING/SUGGESTION issues manually
2. Commit the changes: `git add -A && git commit -m "implement <change-name>"`
3. Sync specs: `/openspec-sync-specs <change-name>`
4. Archive: `/openspec-archive-change <change-name>`
```

## Examples

### Example 1: Basic usage

```bash
/cmd-openspec-apply
```

Auto-detects the active change and runs the full loop (max 5 iterations).

### Example 2: Specify change name

```bash
/cmd-openspec-apply slice-1-2-volume-management
```

Implements the specified change.

### Example 3: Limit iterations

```bash
/cmd-openspec-apply --max-iterations 3
```

Stops after 3 review-fix cycles even if CRITICALs remain.

### Example 4: Skip code review

```bash
/cmd-openspec-apply --skip-code-review
```

Only runs spec compliance review (faster, but less thorough).

### Example 5: Skip both reviews

```bash
/cmd-openspec-apply --skip-code-review --skip-spec-review
```

Just implements tasks without any review (not recommended).

## Integration with Other Skills

This command orchestrates:
- `openspec-apply-change` — implements tasks
- `code-review` — reviews code quality
- `spec-compliance-review` — reviews spec adherence

After this command completes, you can:
- `openspec-sync-specs` — sync delta specs to main specs
- `openspec-archive-change` — archive the completed change

## Notes

- The loop is **iterative**: each cycle implements → reviews → fixes → reviews again
- CRITICAL issues block completion; WARNING/SUGGESTION issues are logged but don't block
- The command is **read-only** for reviews (no code changes) but **writes code** during implementation and fixes
- If a fix introduces new CRITICALs, the next iteration will catch them
- The command respects the OpenSpec workflow: propose → apply → review → sync → archive
