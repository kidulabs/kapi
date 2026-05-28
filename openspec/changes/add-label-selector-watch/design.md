## Context

Phase 1 added labels to `ObjectMeta` and persisted them in both stores. This phase adds the ability to filter watch streams by labels using a `labelSelector` query parameter. The existing `WatchFilter` enum and `EventBus` predicate routing provide the integration surface — this change adds a new `LabelSelector` variant and the parsing/matching logic.

Current state:
- `WatchFilter::All | FieldSelector(FieldSelector)` — two variants
- `WatchFilter::matches(&self, event: &WatchEvent) -> bool` — predicate for EventBus routing
- `parse_field_selector(raw: &str) -> Result<WatchFilter, AppError>` — handler-level parsing
- `ListQuery` struct has `field_selector: Option<String>` — query parameter binding
- `InvalidFieldSelector(String)` error variant — HTTP 400

## Goals / Non-Goals

**Goals:**
- `LabelSelector` type with moderate K8s syntax (equality, inequality, existence, non-existence, AND)
- `WatchFilter::LabelSelector` variant for predicate routing
- `labelSelector` query parameter on watch endpoint
- Clear error messages for malformed selectors
- OpenAPI spec and Swagger UI updated

**Non-Goals:**
- Set-based operators (`in`, `notin`) — future work
- Label selectors on list requests — Phase 3
- Watch filter combinators — Phase 3

## Decisions

### 1. LabelSelector as an enum of conditions

**Decision:** `LabelSelector` is a struct containing a `Vec<LabelRequirement>`, where each requirement is an enum:

```rust
pub enum LabelRequirement {
    Equals { key: String, value: String },
    NotEquals { key: String, value: String },
    Exists { key: String },
    NotExists { key: String },
}

pub struct LabelSelector {
    pub requirements: Vec<LabelRequirement>,
}
```

**Alternatives considered:**
- Single enum with `And(Vec<LabelRequirement>)` — adds nesting for the common case (AND is implicit in K8s)
- Flat enum without struct wrapper — loses the ability to represent "no requirements" (match all)

**Rationale:** K8s label selectors are implicitly ANDed — comma-separated requirements all must match. A struct with a vec of requirements models this naturally. An empty vec means "match all" (equivalent to no selector).

### 2. Moderate K8s syntax

**Decision:** Support these selector forms:
- `key=value` — equality
- `key!=value` — inequality
- `key` — existence (key present, any value)
- `!key` — non-existence (key not present)
- Comma-separated — AND combinator (e.g., `app=nginx,env=prod`)

**Alternatives considered:**
- Minimal (only `key=value`) — too limited for real use
- Full K8s (add `in`, `notin`) — parser complexity for marginal benefit

**Rationale:** Covers 90%+ of real-world label selection use cases. The existence/non-existence operators are important for filtering by optional labels. AND is essential for multi-dimensional selection.

### 3. Parsing strategy

**Decision:** Split on commas first (respecting no escaping — K8s doesn't allow commas in keys/values), then parse each requirement:
1. Check for `!=` (inequality)
2. Check for `=` (equality)
3. Check for `!` prefix (non-existence)
4. Otherwise: existence

**Alternatives considered:**
- Regex-based parsing — harder to produce good error messages
- Parser combinator crate (nom, pest) — overkill for this grammar

**Rationale:** Hand-rolled parsing with `split_once` and `starts_with` is simple, produces clear error messages, and has no dependencies. The grammar is small enough that a parser combinator adds more complexity than it removes.

### 4. Matching logic

**Decision:** `LabelSelector::matches(labels: &HashMap<String, String>) -> bool` evaluates all requirements with AND semantics:

```rust
impl LabelSelector {
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        self.requirements.iter().all(|req| req.matches(labels))
    }
}
```

**Rationale:** Direct translation of K8s semantics. `Iterator::all` short-circuits on first non-match, which is efficient.

### 5. WatchFilter integration

**Decision:** Add `WatchFilter::LabelSelector(LabelSelector)` variant. Update `matches()` to delegate to `LabelSelector::matches()`:

```rust
pub fn matches(&self, event: &WatchEvent) -> bool {
    match self {
        WatchFilter::All => true,
        WatchFilter::FieldSelector(fs) => fs.matches(event),
        WatchFilter::LabelSelector(ls) => ls.matches(&event.object.metadata.labels),
    }
}
```

**Rationale:** Follows the existing pattern. The EventBus doesn't need to change — it already calls `filter.matches()` per watcher.

### 6. Error handling

**Decision:** `InvalidLabelSelector(String)` error variant, separate from `InvalidLabel` (which is for label validation on create/update). Maps to HTTP 400.

**Alternatives considered:**
- Reuse `InvalidFieldSelector` — confusing, labels and fields are different concepts
- Single `InvalidSelector` variant — loses specificity in error messages

**Rationale:** Separate error variants produce clear, specific error messages. The handler can distinguish between field and label selector parse failures.

## Risks / Trade-offs

- **[Parser edge cases]** Comma-separated parsing may have edge cases with whitespace or empty segments. → Mitigation: Trim whitespace, reject empty segments with clear error messages.
- **[Performance]** Label matching iterates all requirements per event per watcher. → Mitigation: Requirements are typically few (< 5), and `Iterator::all` short-circuits. No performance concern at expected scale.
- **[No OR support]** Users may expect OR combinators. → Mitigation: Document that only AND is supported. Future work can add OR if needed.
