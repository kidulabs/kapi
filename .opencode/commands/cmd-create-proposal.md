---
description: Finalize an explore session and create OpenSpec proposals from the decisions made
---

Finalize an explore session by creating OpenSpec proposals from the design decisions made during exploration.

This command transitions from exploration to formal proposal creation. It captures the decisions, determines proposal boundaries, and generates all required artifacts.

---

**Input**: The argument after `/cmd-create-proposal` is optional context or a summary of what was explored. If empty, the command will summarize the explore session and ask for confirmation.

**Steps**

1. **Summarize the explore session**

   Review the conversation history and extract:
   - Key design decisions made
   - Open questions resolved
   - Scope of changes identified
   - Any dependencies or phasing considerations

   Present a concise summary to the user:
   ```
   ## Explore Session Summary
   
   **Topic**: [what was explored]
   
   **Key Decisions**:
   - Decision 1: [what was decided]
   - Decision 2: [what was decided]
   - ...
   
   **Scope**:
   - [change area 1]
   - [change area 2]
   - ...
   
   **Dependencies/Phasing**:
   - [any phasing or dependency considerations]
   ```

   Ask the user: "Does this summary capture the explore session correctly? Should I proceed with creating proposals?"

2. **Determine proposal structure**

   Based on the scope and dependencies, decide whether to create:
   - A single proposal (if changes are tightly coupled)
   - Multiple proposals (if changes can be phased or are independent)

   Present the proposal structure to the user:
   ```
   ## Proposal Structure
   
   **Option A**: Single proposal `<name>`
   - [what it covers]
   
   **Option B**: Multiple proposals
   - Proposal 1: `<name-1>` — [what it covers]
   - Proposal 2: `<name-2>` — [what it covers]
   - Dependencies: [any dependencies between proposals]
   
   **Recommendation**: [which option and why]
   ```

   Ask the user: "Which proposal structure do you prefer?" or accept the recommendation.

3. **Create each proposal using the openspec-propose skill**

   For each proposal in the agreed structure:

   Load the `openspec-propose` skill and follow its workflow to create the change and generate all artifacts:

   ```
   Use the openspec-propose skill to create change "<name>" with the following context:
   
   **Proposal Summary**:
   [Insert the proposal summary from step 2]
   
   **Key Decisions**:
   [Insert key decisions from explore session]
   
   **Scope**:
   [Insert scope from explore session]
   
   **Capabilities**:
   - New: [list new capabilities]
   - Modified: [list modified capabilities]
   ```

   The skill will:
   - Create the change directory
   - Generate proposal.md, design.md, specs/, and tasks.md
   - Ensure all artifacts are complete and ready for implementation

   After the skill completes, verify the status:
   ```bash
   openspec status --change "<name>"
   ```

4. **Show final summary**

   After all proposals are created, present:
   ```
   ## Proposals Created
   
   **Proposal 1**: `<name-1>`
   - Location: `openspec/changes/<name-1>/`
   - Artifacts: proposal.md, design.md, specs/, tasks.md
   - Status: All artifacts complete! Ready for implementation.
   
   **Proposal 2**: `<name-2>` (if applicable)
   - Location: `openspec/changes/<name-2>/`
   - Artifacts: proposal.md, design.md, specs/, tasks.md
   - Status: All artifacts complete! Ready for implementation.
   - Dependencies: Depends on `<name-1>`
   
   **Next Steps**:
   - Run `/opsx-apply` to start implementing `<name-1>`
   - After completing `<name-1>`, implement `<name-2>`
   ```

**Context for the openspec-propose skill**

When invoking the skill, provide:

- **Why**: The motivation from the explore session
- **What Changes**: Specific capabilities being added/modified
- **Non-goals**: What is explicitly out of scope
- **Key Decisions**: All design decisions made during exploration with rationale
- **Alternatives Considered**: Options that were discussed but not chosen
- **Capabilities**: New and modified capability names (kebab-case)
- **Impact**: Affected code, APIs, dependencies
- **Future Work**: Items deferred to later proposals

**Guardrails**

- Use the `openspec-propose` skill to create changes and generate artifacts
- The skill will handle artifact creation following the OpenSpec schema requirements
- If context is critically unclear, ask the user - but prefer making reasonable decisions to keep momentum
- If a change with that name already exists, ask if user wants to continue it or create a new one
- Verify each proposal is complete before moving to the next
- **IMPORTANT**: Pass all explore session context to the skill so it can generate accurate artifacts

**Guidelines**

- **Proposal splitting**: Consider splitting when:
  - Changes have clear phasing (foundation → feature)
  - Changes are independent and can be implemented separately
  - One change is a prerequisite for another
  - Splitting reduces risk and allows incremental validation

- **Task completeness**: Always include:
  - Verification tasks (cargo check, clippy, tests, e2e tests)
  - Documentation tasks (AGENTS.md, docs/, roadmap)
  - "DO NOT auto-commit" task (user wants to review first)
