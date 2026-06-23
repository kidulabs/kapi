---
description: Close out an OpenSpec change end-to-end — sync delta specs, verify, archive, commit, merge feature branch to main, delete branch
---

Orchestrate the final stages of an OpenSpec-driven feature branch: sync delta specs to main specs, verify the implementation, archive the change, commit, merge to main, and clean up the feature branch.

**Input**: The argument after `/cmd-merge` is the change name (kebab-case), e.g., `multi-version-schema-support`.

## Prerequisites

- Feature branch is checked out and has no uncommitted work (or staged work you're about to commit)
- The change directory exists at `openspec/changes/<name>/` (with design, specs, tasks artifacts)
- `openspec` CLI is available for status checks
- `main` branch exists locally

## Workflow

### Step 1: Identify the Change

If the change name is not provided as `$ARGUMENTS`, ask the user. When in doubt, check:

```bash
openspec list --json
```

Filter for active changes (not archived) that have a `tasks` artifact.

### Step 2: Sync Delta Specs → Verify → Fix → Commit

**2a. Sync delta specs to main specs**

If delta specs exist at `openspec/changes/<name>/specs/`, sync them:

> Delegate to a fixer agent with full delta-spec context. Give it the change name and the delta spec files to read. It should:
> - Read each delta spec under `openspec/changes/<name>/specs/*/spec.md`
> - For ADDED requirements: create new main spec files at `openspec/specs/<capability>/spec.md`
> - For MODIFIED requirements: update the existing main spec in-place
> - For REMOVED requirements: delete the entire requirement block from main spec
> - Preserve all requirements not mentioned in the delta

If no delta specs exist, skip this substep.

**2b. Verify the change**

Delegate to an oracle agent:

> Have the oracle read the tasks.md, delta specs, design.md, and key implementation files. Check:
> - Completeness: all tasks [x], requirements implemented
> - Correctness: implementation matches spec requirements
> - Coherence: design decisions followed, patterns consistent
> - Return a report with CRITICAL/WARNING/SUGGESTION issues

**2c. Fix issues found**

Address any CRITICAL or actionable WARNING issues from the verification report:
- Fix failing tests
- Fix spec-implementation mismatches
- Fix code quality issues (unused imports, missing serde attributes, etc.)
- Run `cargo test --lib` after each fix to confirm
- Run `cargo clippy --lib` to check lints (note pre-existing warnings)

**2d. Commit sync changes**

- Verify git status shows only intended changes
- Stage: `git add -A`
- Commit with message describing what was synced, e.g.:
  ```
  sync <name> delta specs to main specs
  
  - <capability-1>: <summary of changes>
  - <capability-2>: <summary of changes>
  - fix: <any code fixes>
  ```

### Step 3: Archive → Commit

**3a. Archive the change**

Check artifact completion via `openspec status --change "<name>" --json`
Confirm all tasks in tasks.md are [x] complete
Move the change directory: `mkdir -p openspec/changes/archive && mv openspec/changes/<name> openspec/changes/archive/YYYY-MM-DD-<name>`
Verify the archive is in place

**3b. Commit the archive**

```
git add openspec/changes/archive/YYYY-MM-DD-<name>/
git rm -r openspec/changes/<name>
git commit -m "archive <name> change (N/N tasks complete)"
```

The `git rm` is critical — without it, the old change directory remains tracked and will reappear on checkout.

### Step 4: Merge → Delete Branch

**4a. Switch to main and merge**

```
git switch main
git merge <feature-branch>
```

Confirm fast-forward or --no-ff as appropriate. If merge conflicts arise, resolve them manually.

**4b. Delete the feature branch**

```
git branch -d <feature-branch>
```

Only use `-D` (force) if the branch has unmerged work and the user confirms.

### Step 5: Final Verification

- `cargo test --lib` — all tests pass
- `git log --oneline -5` — clean history on main
- Summary of all actions taken

## Important Notes

- **The `git rm` after archive is easy to forget.** Without it, the old change directory stays in git's index and reappears after merge. Always do both: add the archive dir AND rm the old one.
- **Sync before archive.** Do the sync explicitly in Step 2 so archive proceeds cleanly.
- **Fix test failures incrementally.** Don't batch all fixes — fix one, run tests, fix next, run tests. This prevents cascading failures.
- **Pre-existing clippy warnings** (like `large_enum_variant`) should be noted but not block the workflow.
- If the user has a remote tracking branch, offer to push main after merge: `git push origin main`.

## DO NOT

- Skip the verification step — always verify before archiving
- Force-push or use `--force` on merge
- Delete the branch before confirming the merge is clean
- Use `-D` to delete a branch when `-d` works — use `-D` only when the user explicitly confirms
- Leave the old change directory behind — both old and archive paths must not coexist
- Archive without checking that delta specs are synced (or confirming with user if skipping)

## Error Handling

| Symptom | Cause | Fix |
|---------|-------|-----|
| Old change directory still exists after archive | `git rm` was missed in commit | `git rm -r openspec/changes/<name>` and recommit |
| Merge conflict | Divergent branches | Resolve conflicts manually, then `git commit` |
| Tests fail after sync | Spec change doesn't match code | Fix code to match spec (or fix spec to match code) |
| `openspec` CLI not found | Not installed | Check `openspec list --json` fallback; skip if unavailable, ask user |
| Branch delete fails with unmerged | Branch has unpushed/unmerged work | Use `-D` only after user confirms |
