## MODIFIED Requirements

### Requirement: Roadmap reflects actual completion state
The roadmap SHALL accurately reflect the completion status of all tasks.

#### Scenario: P2b false completions corrected
- **WHEN** the roadmap is reviewed
- **THEN** P2b T33-T34 are marked as incomplete or completed based on actual state
- **AND** all checkbox states match the codebase

### Requirement: Roadmap includes P3 design decisions
The P3 section SHALL document the design decisions made during exploration.

#### Scenario: P3 tasks updated
- **WHEN** the roadmap P3 section is reviewed
- **THEN** T26-T30 reflect the finalized design (configurable capacity, auto-create on subscribe, WatchStream wrapper, dead channel cleanup on publish)
- **AND** T27b (WatchStream wrapper) and T30b (dead channel cleanup test) are added

### Requirement: Roadmap includes P10 future work
The roadmap SHALL include a P10 section for periodic event bus cleanup.

#### Scenario: P10 section exists
- **WHEN** the roadmap is reviewed
- **THEN** a P10 section exists with tasks for periodic cleanup background task

### Requirement: Roadmap includes hygiene tasks
The roadmap SHALL include tasks for auditing and correcting completed phases against actual codebase.

#### Scenario: Hygiene tasks exist
- **WHEN** the roadmap is reviewed
- **THEN** tasks exist for auditing P0-P2b, fixing P2b incomplete work, and updating checkboxes
