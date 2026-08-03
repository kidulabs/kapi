# cmd-explore Command

Structured design exploration using openspec-explore skill with Socratic questioning methodology.

## Instructions

When the user invokes `/cmd-explore`, follow this workflow:

### Step 1: Load openspec-explore skill

First, load the openspec-explore skill to get the base exploration capabilities:
```
Load skill: openspec-explore
```

### Step 2: Apply structured workflow

Build on top of openspec-explore with these additional constraints:

**ONE QUESTION AT A TIME.** This is the most critical rule. Never present multiple questions. Wait for explicit resolution before moving to the next question.

**Socratic method.** For each design decision:
1. Ask for their instinct: "What's your instinct on X?"
2. Listen to their response
3. Provide your read with reasoning: "Here's my read: [reasoning]"
4. Ask for approval: "Does this make sense?"
5. Wait for explicit approval before moving to the next decision

**Break into slices.** If the problem is large, break it into incremental slices:
- Each slice should be 1-3 days of work
- Each slice should have a clear deliverable statement: "I can [do something]"
- Slices should be vertically sliced (end-to-end value), not horizontally layered

**Explicit approval gates.** Don't move to the next question until the user explicitly approves.

### Step 3: Explore and decide

For each slice, make design decisions incrementally:
- Async vs sync patterns
- Error handling approach
- State management (stateless vs stateful)
- Data storage (file, database, in-memory)
- Idempotency and retry behavior
- API design
- Concurrency model
- Configuration management

### Step 4: Summarize

After exploring, present a summary of all decisions:
```
┌─────────────────────────────────────────────────────────────┐
│  Design Decisions                                           │
└─────────────────────────────────────────────────────────────┘

✅ Decision 1: [what was decided]
✅ Decision 2: [what was decided]
✅ Decision 3: [what was decided]

Next steps:
- [Step 1]
- [Step 2]
```

### Step 5: Update documentation

Check if documentation needs updating:
- Roadmaps
- Architecture docs
- README

**Ask:** "Are there any changes to make to the roadmap or architecture docs?"

## Critical Rules

1. **One question at a time** - Never present multiple questions
2. **Ask instinct first** - "What's your instinct on X?" before giving your read
3. **Explicit approval** - Wait for "yes" before moving to next question
4. **Ground in reality** - Read actual codebase, don't theorize
5. **Incremental slices** - Break into deliverable chunks
6. **Summarize decisions** - Capture all decisions clearly

## Anti-Patterns

❌ Presenting multiple questions at once
❌ Assuming without asking
❌ Moving forward without explicit approval
❌ Overwhelming with information
❌ Theorizing without grounding in codebase
