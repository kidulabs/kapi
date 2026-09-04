---
name: spec-compliance-review
description: "Review code changes against OpenSpec specs and designs. Triggers on: spec compliance, compliance review, check against spec, verify implementation, /spec-compliance-review, review against specs, check spec adherence"
---

# Spec Compliance Review

Review code changes against OpenSpec specs and designs to verify implementation correctness.

## When to Use

Use when you want to verify that code changes comply with the OpenSpec specifications:

- After implementing a change: `/spec-compliance-review`
- Review a specific commit range: `/spec-compliance-review HEAD~3..HEAD`
- Review branch vs main: `/spec-compliance-review main..HEAD`
- Review uncommitted changes (default): `/spec-compliance-review`
- Review against a specific change's specs: `/spec-compliance-review --change slice-1-2-volume-management`

## OpenSpec Directory Layout

```
openspec/
├── specs/                              # MAIN specs (source of truth)
│   └── <capability>/spec.md            # e.g., volume-management/spec.md
└── changes/
    ├── <active-change>/                # Active (in-progress) changes
    │   ├── specs/<capability>/spec.md  # DELTA specs (what this change adds/modifies)
    │   ├── design.md                   # Architecture decisions
    │   ├── proposal.md                 # Why this change
    │   └── tasks.md                    # Implementation checklist
    └── archive/                        # Archived (completed) changes
        └── YYYY-MM-DD-<name>/
            └── ...
```

**Key concepts:**
- **Main specs** (`openspec/specs/`): The source of truth. Updated when a change is synced.
- **Delta specs** (`openspec/changes/<name>/specs/`): Incremental — describe what the change adds/modifies/removes.
- **Active change**: Lives at `openspec/changes/<name>/` (not in `archive/`).
- **Archived change**: Lives at `openspec/changes/archive/YYYY-MM-DD-<name>/`. Read-only reference.

## Inputs

| Argument | Description | Default |
|----------|-------------|---------|
| `<scope>` | Git diff scope (commit range, branch..branch) | Uncommitted changes |
| `--change <name>` | Load specs from this change directory | Auto-detect active change |
| `--specs <list>` | Comma-separated list of spec capabilities to check | Auto-detect from changed files |

## Workflow

### Step 1: Resolve the Scope

Determine what code to review:

```bash
# No scope → uncommitted changes
git diff --name-only

# Commit range
git diff --name-only HEAD~3..HEAD

# Branch vs main
git diff --name-only main..HEAD
```

Store the list of changed files and the full diff content.

### Step 2: Detect Active Change

If `--change` is not provided, auto-detect the active change:

```bash
# List active changes (not archived)
openspec list --json

# Or check the filesystem directly
ls openspec/changes/ | grep -v archive
```

If exactly one active change exists, use it. If multiple, ask the user. If none, fall back to main specs only.

### Step 3: Map Files to Specs

For each changed file, determine which spec capability it relates to:

**Convention-based mapping:**

| File Pattern | Spec Capability |
|--------------|-----------------|
| `kcloud/src/operator/<resource>.rs` | `<resource>-management` |
| `kcloud/src/storage/directory.rs` | `storage-pool-management`, `volume-management` |
| `kcloud-api/src/api/kcloud.io/v1/<resource>.rs` | `<resource>-management` |
| `kcloud/src/core/traits.rs`, `types.rs` | Check all specs for trait usage |
| `openspec/changes/<name>/specs/<capability>/spec.md` | `<capability>` |

**Fallback:** If a file doesn't match any pattern, skip it or ask the user.

**Override:** If `--specs` is provided, use only those specs.

### Step 4: Load Specs and Design

For each relevant capability:

1. **Always load main spec**: `openspec/specs/<capability>/spec.md` (the source of truth)
2. **If active change exists**: Also load `openspec/changes/<name>/specs/<capability>/spec.md` (delta spec — what this change adds/modifies)
3. **If active change exists**: Load `openspec/changes/<name>/design.md` (architecture decisions)
4. **If active change exists**: Load `openspec/changes/<name>/proposal.md` (context/motivation)

**Note:** Delta specs are incremental. They may use `## ADDED Requirements`, `## MODIFIED Requirements`, or `## REMOVED Requirements` sections. The reviewer must understand both the delta and the main spec to assess compliance correctly.

### Step 4: Dispatch Review

**Primary:** Dispatch to Oracle subagent with:
- The full diff (code changes)
- The relevant specs
- The design doc (if available)
- Instructions to check compliance

**Fallback:** If Oracle is unavailable, the orchestrator performs the review directly.

### Step 5: Generate Report

The report should include:

```markdown
## Spec Compliance Report

**Scope**: <scope description>
**Files reviewed**: <count>
**Specs checked**: <list of capabilities>

### CRITICAL
- `<file>:<line>`: <description of violation>
  Spec: <requirement that is violated>
  
### WARNING
- `<file>:<line>`: <description of potential issue>
  Spec: <requirement that may not be fully met>

### SUGGESTION
- `<file>:<line>`: <description of improvement>
  Spec: <requirement that could be better satisfied>

### Compliant Requirements
- <list of requirements that are correctly implemented>
```

## Compliance Check Dimensions

For each spec requirement, check:

1. **Completeness**: Is the requirement implemented?
2. **Correctness**: Does the implementation match the spec's intent?
3. **Error handling**: Are error types/conditions correct? (e.g., `ResizeNotSupported` vs `CreateFailed`)
4. **Validation**: Are required validations present? (e.g., name validation, size parsing)
5. **Status conditions**: Are status conditions set correctly?
6. **Edge cases**: Are edge cases from spec scenarios handled?
7. **Design decisions**: Are design decisions from `design.md` followed?

## Examples

### Example 1: Review uncommitted changes

```bash
/spec-compliance-review
```

Reviews all uncommitted changes against auto-detected specs.

### Example 2: Review a commit range

```bash
/spec-compliance-review HEAD~5..HEAD
```

Reviews the last 5 commits.

### Example 3: Review against a specific change

```bash
/spec-compliance-review --change slice-1-2-volume-management
```

Loads specs from the change directory and reviews uncommitted changes against them.

### Example 4: Review specific specs

```bash
/spec-compliance-review --specs volume-management,storage-pool-management
```

Reviews uncommitted changes against only the specified specs.

## Integration with Other Skills

This skill is part of the OpenSpec workflow:

1. `/openspec-propose` → create change proposal
2. `/openspec-apply` → implement the change
3. `/spec-compliance-review` → verify implementation against specs
4. `/code-review` → review code quality
5. `/openspec-archive` → archive the change

## Notes

- The skill is **read-only** — it does not modify code
- If violations are found, suggest fixes but do not apply them automatically
- The skill should be fast — only load relevant specs, not all specs
- If a file doesn't map to any spec, skip it (don't fail)
