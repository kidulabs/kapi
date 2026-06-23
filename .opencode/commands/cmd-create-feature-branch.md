---
description: Create a feature branch and commit the OpenSpec change proposal files
---

Create a feature branch for an OpenSpec change and commit the proposal artifacts.

**Input**: The argument after `/opsx-branch` is the change name (kebab-case), e.g., `multi-version-schema-support`.

**Steps**

1. **If no change name provided, ask for it**

   Use the **AskUserQuestion tool** (open-ended) to ask:
   > "Which change do you want to branch? Give the change name (kebab-case)."

2. **Ensure you're on main and clean (relative to this change)**

   ```bash
   git branch --show-current
   ```
   If not on `main`, ask the user if they want to proceed from the current branch. If there are uncommitted changes to files outside `openspec/changes/<name>/`, warn the user — those files will NOT be included in this commit.

3. **Run `cargo fmt` to satisfy pre-commit hooks**

   ```bash
   cargo fmt --all
   ```

4. **Create the feature branch**

   ```bash
   git checkout -b feat/<name>
   ```
   Use `feat/` prefix to match conventional branch naming. If the branch already exists, ask the user if they want to switch to it instead.

5. **Stage only the OpenSpec change files**

   ```bash
   git add openspec/changes/<name>/
   ```
   This stages exactly the proposal artifacts (proposal.md, design.md, specs/, tasks.md, .openspec.yaml) without pulling in any unrelated working-tree changes.

6. **Commit with a conventional commit message**

   ```bash
   git commit -m "docs: add <name> change proposal

   Add OpenSpec change proposal artifacts.

   Artifacts:
   - proposal.md: motivation, scope, breaking change callout
   - design.md: technical decisions, risks, migration plan
   - specs/: delta specs for modified capabilities
   - tasks.md: implementation checklist"
   ```

   Use `docs:` prefix — proposals are documentation of intent, not the implementation itself.

7. **Report the result**

   Show: branch name, commit hash (short), files committed.
