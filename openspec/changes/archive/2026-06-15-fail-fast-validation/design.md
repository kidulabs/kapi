## Context

The kapi request pipeline currently runs format validation (label regex, annotation size limits) inside `ObjectService`, after the handler has already extracted labels and annotations into `HashMap<String, String>`. This means a request with an invalid label key (e.g., containing `!`) travels through handler extraction, service method dispatch, and potentially schema registry lookup before failing on a pure string check.

The handler already performs structural validation (type checks, body shape, selector parsing). Format validation is a natural extension of this responsibility — it answers "is this request syntactically valid?" rather than "does this request make sense given current state?"

Current validation functions in `object/service.rs`:
- `validate_label_key` — regex + length (lines 28-86)
- `validate_label_value` — regex + length (lines 90-110)
- `validate_labels` — iterates map, calls key/value validators (lines 114-120)
- `validate_annotation_key` — length only (lines 123-134)
- `validate_annotations` — key validation + 256KB total size (lines 140-155)

These functions compile `Regex` patterns on every call (3 regex compilations per label with prefix).

## Goals / Non-Goals

**Goals:**
- Fail-fast: reject format-invalid requests at the handler edge before any service I/O
- Extract validation into a reusable module callable from both handler and service
- Eliminate per-call regex recompilation via `LazyLock<Regex>` statics
- Preserve defense-in-depth: service retains validation calls for non-HTTP callers
- Update the handler principle to reflect format validation as a handler responsibility

**Non-Goals:**
- Moving stateful validation (schema lookup, JSON Schema validation, OCC, deletion guards) to the handler
- Changing validation rules, error messages, or error types
- Removing service-level validation
- Adding new validation rules

## Decisions

### Decision 1: New `src/validation/` module

**Choice:** Create `src/validation/mod.rs` containing all format validation functions.

**Alternatives considered:**
- *Keep in `object/service.rs`, make `pub`*: Works but conflates orchestration concerns with format rules. The module name `service` implies business logic, not parsing.
- *Put in `object/validation.rs`*: Ties validation to the object module, but these rules are domain-level (labels/annotations are cross-cutting). A top-level `validation/` module signals reusability.
- *Put in `object/types.rs`*: Types module is for data structures, not validation logic.

**Rationale:** A dedicated `validation/` module clearly separates "what is valid" from "what to do with it." Both handler and service import from the same source of truth.

### Decision 2: `LazyLock<Regex>` for compiled patterns

**Choice:** Use `std::sync::LazyLock<Regex>` statics for the three regex patterns (prefix DNS subdomain, label name, label/annotation value).

**Alternatives considered:**
- *`once_cell::sync::Lazy`*: Requires an external dependency. `LazyLock` is stable in std since Rust 1.80.
- *`Regex::new` at call site (current)*: Compiles regex on every call. For a request with 10 labels (some with `/` prefix), up to 30 compilations per request.
- *Compile once in `ObjectService::new`*: Ties regex lifetime to service instance. Validation functions should be callable without a service instance.

**Rationale:** `LazyLock` is zero-cost (compile once, reuse forever), requires no dependencies, and keeps validation functions as free functions (no instance needed).

### Decision 3: Defense-in-depth — validate in both layers

**Choice:** Handler calls validation eagerly; service retains validation calls.

**Alternatives considered:**
- *Handler only, skip service validation*: Breaks if a future caller (gRPC, CLI, background job) bypasses the handler. The service is the public API of the domain.
- *Service only (current)*: Misses the fail-fast opportunity.

**Rationale:** The cost of double validation is negligible (O(n) string scans on in-memory data). The benefit is that the service remains self-defending regardless of caller. This is standard practice in layered architectures.

### Decision 4: Update handler principle wording

**Choice:** Update the module doc comment in `handler.rs` from "no business logic in handlers" to: "Handlers validate input format and deserialization constraints. They never access the store, event bus, or schema registry, and never contain conditional mutation logic."

**Alternatives considered:**
- *Keep current wording*: Already aspirational — handler already validates spec shape, rejects unknown keys, checks URL/body consistency. The current wording is misleading.
- *Remove the principle entirely*: Loses the architectural intent.

**Rationale:** The new wording accurately describes what handlers do (format validation) and what they don't do (state access, mutation logic). Format validation is parsing, not business logic.

### Decision 5: Validation applies to both create and update paths

**Choice:** Add validation calls in both `create()` and `update()` handlers.

**Rationale:** The `update()` handler deserializes the body as `Json<StoredObject>` via serde — labels and annotations are already inside `body.metadata`. Without explicit validation calls, the update path would not get the fail-fast benefit. The council flagged this gap.

## Risks / Trade-offs

**[Double validation overhead]** → Negligible. Format checks are O(n) string scans. The regex compilation fix (LazyLock) actually makes the second call cheaper than the current single call.

**[Handler becomes "thicker"]** → Mitigated by clear principle: handlers validate format, never access state. The validation functions are pure — no I/O, no side effects. The handler code change is 2 lines per handler.

**[Test coverage gap during transition]** → Existing integration tests exercise validation via HTTP (will now hit handler-level). Service unit tests still exercise service-level. Add handler-level unit tests for the new validation calls to ensure both paths are covered.

**[Module boundary confusion]** → Mitigated by clear naming: `validation/` for format rules, `object/service.rs` for orchestration. The module doc comments should explicitly state the boundary.
