## Context

kapi-server provides a RESTful API with cluster-scoped and namespace-scoped routes for managing schemas, objects, and namespaces. The API uses `group/version/kind` addressing with JSON payloads. Two placeholder crates exist (`kapi-client` and `kapi-cli`) but contain no implementation. The `kapi-core` crate provides shared types (`StoredObject`, `ResourceKey`, `WatchEvent`, etc.).

## Goals / Non-Goals

**Goals:**
- Implement a thin HTTP client library (`kapi-client`) wrapping all kapi-server endpoints
- Implement a CLI tool (`kapi-cli`) with kubectl-style verb-first commands
- Support short name resolution (kind → group/version/kind) via schema queries
- Support output formatting (table, JSON, YAML) with resource-specific table columns
- Support label selectors and auto-pagination
- Provide YAML-based configuration with env var override

**Non-Goals:**
- `--dry-run` for mutating commands
- `--server` CLI flag (config/env only for v1)
- Watch resume with `--from-version`
- `fieldSelector` support (only `metadata.name` exists, covered by `get <name>`)
- Custom table columns or `--output wide`
- Auth/authentication support
- Schema resolution caching to disk

## Decisions

### 1. Resource Addressing: Short Name Resolution

**Decision**: Users type `kapi get Widget my-widget`. CLI queries `list_schemas()` to resolve `Widget` → `example.io/v1/Widget`. Support `group/kind` syntax (e.g., `kapi get example.io/Widget`) as ambiguity escape hatch.

**Alternatives considered**:
- Always require full path (`example.io/v1/Widget`) — rejected: too verbose
- Cache schema resolutions to disk — rejected: unnecessary complexity for v1

**Rationale**: Matches kubectl mental model. One extra round-trip per command is acceptable for interactive use. In-memory caching per command invocation is sufficient.

### 2. Command Structure: Verb-First (kubectl-Style)

**Decision**: `kapi get`, `kapi apply`, `kapi delete`, `kapi watch`, `kapi status get`, `kapi status apply`, `kapi completions`. Schema and Namespace are regular kinds — no special subcommands.

**Alternatives considered**:
- Resource-first (`kapi widget get`) — rejected: doesn't scale to dynamic kinds
- Hybrid (generic + special) — rejected: two ways to do things creates confusion

**Rationale**: Verb-first is familiar to kubectl users and works naturally with dynamic resource types. Schema and Namespace are just registered schemas, so treating them as regular kinds is consistent.

### 3. Apply Merge Semantics: kubectl-Style

**Decision**: File contains `{ metadata: { name, labels?, annotations? }, spec: {...} }`. CLI does GET, preserves `system.*`, replaces `spec` wholesale, merges `labels`/`annotations` additively. Fail immediately on conflict (no auto-retry, no `--force`).

**Alternatives considered**:
- Full object in file (including `key`, `system`) — rejected: too verbose for users
- Deep merge of `spec` — rejected: harder to reason about, full replacement matches kubectl behavior

**Rationale**: Matches kubectl's `apply` behavior. Users provide the desired state, CLI handles the merge. Immediate failure on conflict is honest and predictable.

### 4. Client API: Low-Level, Explicit Methods

**Decision**: One method per endpoint. Client is route-agnostic — caller passes `namespace: Option<&str>`, client constructs URL. No scope validation in client.

```rust
client.list(key, namespace, opts)
client.get(key, namespace, name)
client.create(key, namespace, meta, spec)
client.update(namespace, obj)
client.delete(key, namespace, name)
client.get_status(key, namespace, name)
client.update_status(key, namespace, name, status)
client.watch(key, filter)
```

**Alternatives considered**:
- Higher-level API with auto-scope detection — rejected: adds hidden behavior and extra round-trips
- Client validates scope from schemas — rejected: client should be thin, CLI handles scope

**Rationale**: Thin wrapper is predictable, testable, and debuggable. CLI handles higher-level logic (scope resolution, short name resolution).

### 5. Output Formats: Table (Default), JSON, YAML

**Decision**: Table output with resource-specific columns:
- Namespaced objects: NAME, NAMESPACE, AGE
- Cluster-scoped objects: NAME, AGE
- Schema: NAME, AGE
- Namespace: NAME, AGE

Watch uses same formats (table rows for events: `EVENT_TYPE NAME [NAMESPACE] AGE`).

**Alternatives considered**:
- JSON only — rejected: not human-friendly
- Configurable columns — rejected: overkill for v1
- `--output wide` — rejected: defer to v2

**Rationale**: Table is human-friendly, JSON/YAML are scriptable and familiar. Resource-specific columns show relevant information without noise.

### 6. Configuration: YAML with Env Var Override

**Decision**: YAML config file at `~/.kapi/config.yaml` (override via `KAPI_CONFIG` env var). Minimal v1 structure: `server: http://localhost:8080`. Precedence: flag > env > config > default.

**Alternatives considered**:
- TOML — rejected: YAML is more familiar to k8s users
- Multi-context config (kubectl-style) — rejected: overkill for v1
- `--server` flag — rejected: defer to v2, config is sufficient

**Rationale**: YAML is familiar, minimal structure is simple, env var override enables scripting. Future-proof for auth fields.

### 7. Namespace Handling: Default to "default"

**Decision**: Default namespace is `"default"` for namespaced kinds when `-n` not specified. Cluster-scoped kinds with `-n` flag: warn to stderr, ignore flag, proceed.

**Alternatives considered**:
- Require explicit namespace — rejected: breaks "just works" feeling
- Cross-namespace by default — rejected: too noisy

**Rationale**: Matches kubectl behavior. Server already defaults to `"default"` on cluster-scoped URLs. Warning for cluster-scoped kinds is helpful without being pedantic.

### 8. Pagination: Auto-Paginate by Default

**Decision**: Auto-paginate list operations (follow `continue_token` until exhausted). Expose `--limit` as escape hatch. Don't expose `continue_token` in v1.

**Alternatives considered**:
- Expose `--limit` and `--continue` — rejected: too complex for users
- No pagination — rejected: silently truncates results

**Rationale**: Users expect complete results. Auto-pagination is transparent. `--limit` provides control when needed.

### 9. Label Selectors: Add `-l` Flag

**Decision**: Add `-l/--label-selector` to `get` and `watch` commands. Thin passthrough to client.

**Alternatives considered**:
- Defer to v2 — rejected: major usability gap, `kubectl get -l` is heavily used
- Add `fieldSelector` too — rejected: only `metadata.name` exists, covered by `get <name>`

**Rationale**: Label selectors are essential for filtering. Implementation is trivial (passthrough to server).

### 10. Error Handling: Context-Rich Errors

**Decision**: Context-rich errors to stderr: "Widget 'my-widget' not found in namespace 'production'". Exit code 1 on error. Special-case schema-not-found: "No schema found for kind 'Wdiget'. Use 'kapi get Schema' to list available kinds."

**Alternatives considered**:
- Simple errors ("not found") — rejected: not helpful for debugging
- Suggestions ("did you mean...?") — rejected: overkill for v1

**Rationale**: Context is essential for debugging. Exit code 1 is scriptable. Special-casing schema errors helps users learn.

## Risks / Trade-offs

**[Short name resolution adds latency]** → In-memory caching per command invocation. One HTTP call to list schemas is acceptable for interactive use. If latency becomes problematic, add disk caching in v2.

**[Apply merge is complex]** → kubectl-style merge (full replacement of `spec`, additive merge of `labels`/`annotations`) is well-understood. Document file format clearly with examples.

**[Watch table output is noisy]** → Streaming table rows for watch events may be hard to read. Users can use `-o json` and pipe to `jq` for cleaner output. Consider color-coding event types in v2.

**[Delete with finalizers is confusing]** → DELETE on object with finalizers sets `deletion_timestamp` but doesn't actually delete. Return success (matches server behavior). Document this behavior. Users can `get` to see `deletion_timestamp`.

**[Schema resolution requires extra round-trip]** → Every command queries schemas to resolve short names. Acceptable for v1. If problematic, add caching in v2.

**[No `--dry-run` for mutating commands]** → Users can't preview changes. Defer to v2. Users can `get` before `apply` to see current state.
