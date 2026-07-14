## Context

kapi is a Kubernetes-apiserver-inspired API server in Rust. Users need to scaffold controller projects with the correct directory structure, dependencies, and boilerplate. Currently, users must manually set up projects.

## Goals / Non-Goals

**Goals:**
- Provide `kapibuild init` command to scaffold new controller projects
- Generate Cargo.toml with correct dependencies (kapi-core, kapi-client, kapi-controller, kapi-derive)
- Create standard directory structure (api/, schemas/, src/)
- Generate src/main.rs with Manager setup
- Generate Kapifile manifest for resource tracking

**Non-Goals:**
- Creating API types (separate command)
- Generating schemas (separate command)
- Controller scaffolding (separate command)
- Interactive prompts (future enhancement)

## Decisions

### 1. Directory structure

**Decision:** Use standard layout:
```
<project>/
├── Cargo.toml
├── Kapifile
├── api/
├── schemas/
└── src/
    └── main.rs
```

**Rationale:** Matches kubebuilder conventions. Separates API types from controllers. Flat schemas directory for easy application.

### 2. Cargo.toml dependencies

**Decision:** Include kapi-core, kapi-client, kapi-controller, kapi-derive, serde, tokio, tracing, async-trait, schemars.

**Rationale:** These are the minimal dependencies needed for a controller project. Users can add more as needed.

### 3. Kapifile format

**Decision:** YAML format with domain and resources list.

**Rationale:** Explicit is better than implicit. Tracks project metadata. Easy to parse and update.

### 4. Main.rs template

**Decision:** Include Manager setup with placeholder for controller wiring.

**Rationale:** Provides working starting point. Users add controllers as needed.

## Risks / Trade-offs

**[Risk] Hardcoded dependency paths** → Mitigation: Detect kapi workspace root dynamically. Use absolute paths in generated Cargo.toml.

**[Risk] Template drift** → Mitigation: Keep templates simple. Document clearly.

**[Trade-off] Static templates vs interactive** → Static templates are simpler but less flexible. Can add interactive mode later.
