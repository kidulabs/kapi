# roadmap-update

## Purpose

Define requirements for updating the `roadmap.md` project planning document to reflect architectural decisions, implementation progress, and current module structure. Roadmap updates are specification-level changes that keep the planning document aligned with the actual codebase and design decisions.

## Requirements

### Requirement: Architecture section reflects unified store

The roadmap Architecture section SHALL depict a single `ObjectStore` trait instead of separate `SchemaStore` and `ObjectStore` traits. The AppState diagram SHALL show only one store component.

#### Scenario: Architecture diagram shows single store
- **WHEN** a reader views the Architecture section of roadmap.md
- **THEN** the diagram shows `ObjectStore` (singular) in AppState, not `SchemaStore` + `ObjectStore`

#### Scenario: Architecture text describes unified model
- **WHEN** a reader views the Layers description in roadmap.md
- **THEN** the Store layer description references a single pluggable `ObjectStore` trait

### Requirement: API Surface section uses unified schema path

The roadmap API Surface section SHALL list schema operations at `/apis/kapi.io/v1/Schema` instead of `/schemas` or `/apis/kapi.io/v1/schemas`.

#### Scenario: Schema endpoints use object-style path
- **WHEN** a reader views the Schema Registry table in roadmap.md
- **THEN** all schema paths are under `/apis/kapi.io/v1/Schema`

### Requirement: Key Types section clarifies Schema as object convention

The roadmap Key Types section SHALL clarify that Schema is not a separate struct but a convention: a `StoredObject` with `kind: "Schema"`.

#### Scenario: Schema described as StoredObject convention
- **WHEN** a reader views the Key Types section
- **THEN** Schema is described as a StoredObject with kind="Schema", not as a standalone struct

### Requirement: Storage Traits section shows only ObjectStore

The roadmap Storage Traits section SHALL define only the `ObjectStore` trait. There SHALL NOT be a `SchemaStore` trait definition.

#### Scenario: Only ObjectStore trait is documented
- **WHEN** a reader views the Storage Traits section
- **THEN** only `ObjectStore` trait is shown with its methods

### Requirement: Design Decisions table includes unified architecture decisions

The roadmap Design Decisions table SHALL include entries for:
- Unified single ObjectStore (replacing split traits)
- Builtin meta-schema for Schema validation
- Block Schema deletion when objects exist

#### Scenario: Design decisions table has unified entries
- **WHEN** a reader views the Design Decisions table
- **THEN** it contains rows for unified store, meta-schema, and block-deletion decisions

### Requirement: Module Tree section shows collapsed schema directory

The roadmap Module Tree section SHALL show `src/schema/` containing only `meta_schema.rs`. It SHALL NOT show `schema/types.rs`, `schema/service.rs`, or `schema/handler.rs`.

#### Scenario: Module tree shows correct schema structure
- **WHEN** a reader views the Module Tree section
- **THEN** `src/schema/` lists only `meta_schema.rs`

### Requirement: Backlog tasks reflect unified architecture

The roadmap Backlog section SHALL have tasks that implement the unified architecture. Tasks for `SchemaStore`, separate schema service, and separate schema handlers SHALL NOT exist. Tasks for meta-schema, unified ObjectStore, and unified handlers SHALL exist.

#### Scenario: No SchemaStore tasks exist
- **WHEN** a reader searches the Backlog for "SchemaStore"
- **THEN** no tasks reference SchemaStore

#### Scenario: Meta-schema tasks exist
- **WHEN** a reader views the Backlog
- **THEN** tasks exist for creating the meta-schema module and validation logic

#### Scenario: Unified store tasks exist
- **WHEN** a reader views the Backlog
- **THEN** tasks define a single ObjectStore trait and InMemoryStore implementation

### Requirement: Roadmap includes P3 design decisions
The P3 section SHALL document the design decisions made during exploration.

#### Scenario: P3 tasks updated
- **WHEN** the roadmap P3 section is reviewed
- **THEN** T26–T30 reflect the finalized design (configurable capacity, auto-create on subscribe, WatchStream wrapper, dead channel cleanup on publish)
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
- **THEN** tasks exist for auditing P0–P2b, fixing P2b incomplete work, and updating checkboxes

### Requirement: Task completion status is accurate

The roadmap Backlog SHALL mark completed tasks with `[x]`. All checkbox states SHALL match the codebase.

#### Scenario: Completed tasks are marked
- **WHEN** a reader views the Backlog
- **THEN** T1 through T12 show `[x]` (completed) status

#### Scenario: P2b false completions corrected
- **WHEN** the roadmap is reviewed
- **THEN** P2b T33–T34 are marked as incomplete or completed based on actual state
- **AND** all checkbox states match the codebase
