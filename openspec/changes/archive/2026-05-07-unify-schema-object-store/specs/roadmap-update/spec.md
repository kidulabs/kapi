## ADDED Requirements

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

### Requirement: Task completion status is accurate

The roadmap Backlog SHALL mark tasks T1–T12 as completed (`[x]`). Task T5 SHALL reflect actual build status.

#### Scenario: Completed tasks are marked
- **WHEN** a reader views the Backlog
- **THEN** T1 through T12 show `[x]` (completed) status
