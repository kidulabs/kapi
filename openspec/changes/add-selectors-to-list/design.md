## Context

Phase 1 added labels to objects. Phase 2 added label selectors for watch. This phase completes the selector story by enabling filtering on list requests and combining field + label selectors on watch.

Current state:
- `ListOptions { limit, continue_token }` — no filter parameters
- `ObjectStore::list(key, opts)` — no filtering at store level
- `fieldSelector` on list returns 400 — explicitly rejected by handler
- `WatchFilter::All | FieldSelector | LabelSelector` — no combinator
- InMemoryStore::list() fetches all, sorts, paginates — no filtering
- SQLiteStore::list() builds SQL with WHERE on ResourceKey + pagination — no label/field filtering

## Goals / Non-Goals

**Goals:**
- Store-level filtering for list requests (both field and label selectors)
- Filtering happens before pagination (correct page sizes)
- `WatchFilter::And` combinator for watch with both selectors
- Remove 400 error for `fieldSelector` on list
- OpenAPI spec and Swagger UI updated

**Non-Goals:**
- OR combinators
- Complex nested boolean logic
- Query optimization beyond basic SQL WHERE

## Decisions

### 1. Filter parameters on ListOptions

**Decision:** Add optional filter fields to `ListOptions`:

```rust
pub struct ListOptions {
    pub limit: Option<usize>,
    pub continue_token: Option<ContinueToken>,
    pub field_selector: Option<FieldSelector>,
    pub label_selector: Option<LabelSelector>,
}
```

**Alternatives considered:**
- Separate `ListFilter` struct — adds indirection for two optional fields
- Pass filters as separate parameters to `list()` — changes trait signature more than needed

**Rationale:** Keeps all list parameters in one struct. Optional fields mean existing callers don't need to change (just add `..Default::default()` or set to `None`).

### 2. Filtering before pagination

**Decision:** Both store implementations MUST apply filters before pagination. This ensures correct page sizes and cursor semantics.

**InMemoryStore:**
1. Collect all objects for key
2. Apply field_selector filter (in Rust)
3. Apply label_selector filter (in Rust)
4. Sort by name
5. Apply continue_token skip
6. Truncate to limit

**SQLiteStore:**
1. Build SQL with WHERE clauses for field + label filters
2. Include `name > ?` for continue_token
3. ORDER BY name, LIMIT

**Rationale:** If you filter after pagination, you get wrong page sizes (fetch 100, filter to 3, client asked for 10). Filtering must happen first.

### 3. SQLite label filtering with EXISTS subqueries

**Decision:** Use `EXISTS` subqueries for label conditions:

```sql
-- Equality: app=nginx
AND EXISTS (
    SELECT 1 FROM labels l
    WHERE l.resource_group = o.resource_group
      AND l.api_version = o.api_version
      AND l.resource_kind = o.resource_kind
      AND l.name = o.name
      AND l.label_key = 'app'
      AND l.label_value = 'nginx'
)

-- Inequality: env!=production
AND (
    NOT EXISTS (SELECT 1 FROM labels l WHERE ... AND l.label_key = 'env')
    OR EXISTS (SELECT 1 FROM labels l WHERE ... AND l.label_key = 'env' 
               AND l.label_value != 'production')
)

-- Existence: gpu
AND EXISTS (SELECT 1 FROM labels l WHERE ... AND l.label_key = 'gpu')

-- Non-existence: !experimental
AND NOT EXISTS (SELECT 1 FROM labels l WHERE ... AND l.label_key = 'experimental')
```

**Alternatives considered:**
- JOIN-based approach — more complex, harder to compose multiple conditions
- Fetch all + filter in Rust — defeats the purpose of SQL-level filtering

**Rationale:** `EXISTS` subqueries are composable (each requirement is one clause), efficient with the composite PK index on labels, and map directly to the `LabelRequirement` enum.

### 4. SQLite field filtering

**Decision:** For `FieldSelector::NameEquals(name)`, add `AND name = ?` to the WHERE clause.

**Rationale:** Direct SQL comparison. Simple and efficient.

### 5. WatchFilter::And combinator

**Decision:** Add `WatchFilter::And(Box<WatchFilter>, Box<WatchFilter>)` variant:

```rust
pub enum WatchFilter {
    All,
    FieldSelector(FieldSelector),
    LabelSelector(LabelSelector),
    And(Box<WatchFilter>, Box<WatchFilter>),
}
```

Matching logic:
```rust
WatchFilter::And(a, b) => a.matches(event) && b.matches(event),
```

**Alternatives considered:**
- `And(Vec<WatchFilter>)` — more flexible but adds complexity for the common case (2 filters)
- Separate `CombinedFilter` type — adds indirection

**Rationale:** `Box` avoids recursive type sizing. Two-element `And` covers the use case (field + label). If more complex combinations are needed later, `And(And(a, b), c)` works.

### 6. Handler combination logic

**Decision:** When both `fieldSelector` and `labelSelector` are present on a watch request, combine them with `WatchFilter::And`:

```rust
let filter = match (field_filter, label_filter) {
    (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
    (Some(f), None) => f,
    (None, Some(l)) => l,
    (None, None) => WatchFilter::All,
};
```

**Rationale:** Clean pattern matching. Handles all four cases explicitly.

### 7. Remove 400 for fieldSelector on list

**Decision:** Remove the handler code that returns 400 for `fieldSelector` on non-watch requests. Instead, pass the parsed selector to `ListOptions`.

**Rationale:** The store now supports filtering. The 400 was a temporary restriction.

## Risks / Trade-offs

- **[SQL complexity]** Dynamic SQL generation with multiple WHERE clauses adds code complexity. → Mitigation: Build SQL incrementally with a query builder pattern or string concatenation with clear structure.
- **[N+1 label queries]** Naive implementation might query labels per object. → Mitigation: Use EXISTS subqueries (single query) or batch label fetches.
- **[Pagination correctness]** If filtering is applied after pagination, results are wrong. → Mitigation: Explicit ordering in both store implementations (filter → sort → paginate). Tests verify page sizes with filters.
- **[Breaking change]** `fieldSelector` on list previously returned 400, now returns filtered results. → Mitigation: No client should rely on the 400. Document the behavior change.
