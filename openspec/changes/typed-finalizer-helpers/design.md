## Context

The controller runtime's finalizer helpers in `kapi-controller/src/finalizer.rs`
(`ensure_finalizer` and `remove_finalizer`) accept `&StoredObject`. Typed
controllers must therefore call `TypedResource::to_stored_object()` and thread
the raw value through the reconcile path, even though every typed resource
already exposes the metadata the helpers need. With kapi 0.3.1 shipping
`TypedResource::to_stored_object`, `is_deleting`, and `has_finalizer`, the
missing seam is a typed-entry mutation helper that hides the raw conversion
from generated and hand-written typed controllers.

The existing raw helpers already implement the CAS-retry loop (up to 5
attempts, 10 ms backoff, 409 Conflict re-fetch) — the typed helpers must
preserve that behaviour exactly, not re-implement it.

## Goals / Non-Goals

**Goals:**

- Let typed controllers pass `&T where T: TypedResource` directly to
  `ensure_finalizer` / `remove_finalizer` without materialising a
  `StoredObject` at the call site.
- Preserve the existing CAS-retry semantics byte-for-byte (5 attempts, 409
  re-fetch, 10 ms backoff).
- Keep the raw `&StoredObject` helpers untouched for backward compatibility.
- Add no new crate dependencies — `TypedResource` is already re-exported by
  `kapi-client`, which `kapi-controller` already depends on.

**Non-Goals:**

- Changing server-side finalizer semantics or the deletion ordering (covered
  by the `finalizer-support` spec).
- Removing or deprecating the raw helpers — they remain the canonical API for
  raw-client code paths.
- Rewriting the CAS retry loop into a shared generic — out of scope; the
  typed helpers delegate to the existing raw ones.
- Adding typed helpers for status subresource or other mutation paths — this
  change is scoped to finalizers only.

## Decisions

### Decision 1: Same-name free functions with a `TypedResource` bound

Add `ensure_finalizer<T: TypedResource>(client, obj: &T, finalizer)` and
`remove_finalizer<T: TypedResource>(client, obj: &T, finalizer)` as new
free functions in `kapi-controller/src/finalizer.rs`. Rust's lack of name
overloading is sidestepped because the second parameter type is
disjoint — `&StoredObject` vs `&T` — so existing call sites resolve to the
raw helper and new typed call sites resolve to the generic helper by
type inference.

**Alternative considered:** Distinct names like `ensure_typed_finalizer`.
Rejected — adds a naming tax with no semantic benefit; callers already
pick the right function by the type they pass.

**Alternative considered:** A `FinalizerOps` trait with `ensure_finalizer`
as a method on `T`. Rejected — larger API surface, less consistent with
the existing free-function style, and awkward for the raw variant which
would either need a parallel trait impl on `StoredObject` or remain a
free function (creating two APIs for the same operation).

### Decision 2: Delegate to the existing raw helpers; no retry-loop duplication

Each typed helper performs exactly one `obj.to_stored_object()?` conversion,
then calls the existing `ensure_finalizer(client, &stored, finalizer)` /
`remove_finalizer(client, &stored, finalizer)`. The CAS retry stays inside
the raw helper — re-fetches return `StoredObject`, which is what the raw
helper already expects. The typed layer does not re-materialise `T` from
the re-fetched value.

**Alternative considered:** Extract the CAS loop into a private
`cas_update_finalizer(client, stored, add_or_remove)` and have both raw
and typed helpers call it. Rejected — more churn than this change needs;
the existing raw helpers are already correct and well-tested.

### Decision 3: Propagate `TypedError` via `Box<dyn Error + Send + Sync>`

The typed helpers return
`Result<(), Box<dyn std::error::Error + Send + Sync>>` — the same error type
as the raw helpers. `TypedError` (from `to_stored_object` failures) is
already `Error + Send + Sync + 'static`, so `?` converts it into the boxed
error without introducing a new error enum. Callers can downcast with
`.downcast_ref::<TypedError>()` if they need to distinguish serialization
failures from client errors.

**Alternative considered:** A dedicated `TypedFinalizerError` enum. Rejected
— new public error type for a thin wrapper; downcasting on the boxed error
is sufficient and consistent with the raw helpers.

**Alternative considered:** Return `Result<(), ClientError>`. Rejected —
`TypedError` is not a `ClientError`, so the conversion would require a new
variant or wrapping, expanding the client error surface for a helper that
isn't really the client's concern.

### Decision 4: Update the kapibuild controller template

The kapibuild controller generator template (`kapibuild/src/controller_generate.rs`)
currently emits `let stored = resource.to_stored_object()?;` followed by
`finalizer::ensure_finalizer(&ctx.client, &stored, FINALIZER_NAME)` /
`finalizer::remove_finalizer(&ctx.client, &stored, FINALIZER_NAME)`. Update
the template to emit `finalizer::ensure_finalizer(&ctx.client, resource, FINALIZER_NAME)`
directly — the typed helper handles the conversion internally. The
`to_stored_object` call disappears from generated reconcile paths.

Golden tests for the generator are updated to assert the new call shape and
the absence of the intermediate `stored` binding.

## Risks / Trade-offs

- **One local serialization per helper call** → `to_stored_object` serialises
  spec/status via serde_json. Acceptable — controllers already perform at
  least one network round trip per reconcile, and the local serialisation
  cost is negligible relative to HTTP. Benchmark only if real workloads
  show it.
- **Re-fetch inside CAS retry returns `StoredObject`, not `T`** → This is
  correct: the retry loop operates on the raw object and never needs to
  re-materialise `T`. Document this in the helper's doc comment so callers
  don't expect the typed value to refresh.
- **Trait bound `T: TypedResource` transitively requires `Sized + Send + Sync + 'static`** → All generated typed resources already satisfy these bounds. Third-party implementations that pre-date the bound will still compile because the typed helpers are additive; they only restrict callers of the new helpers, not the raw ones.
- **Same-name resolution could surprise callers** → If a caller accidentally
  passes a `StoredObject` to what they intended as a typed call, type
  inference silently resolves to the raw helper. Mitigation: doc comments
  on both helpers cross-reference each other and state the preferred entry
  point for each caller type.

## Migration Plan

1. Add the typed helpers and unit tests in `kapi-controller/src/finalizer.rs`.
   Raw helpers remain untouched — no migration needed for existing callers.
2. Update the kapibuild controller generator template and golden tests.
   Regenerate or re-verify affected test fixtures.
3. Downstream typed controllers (e.g., kcloud's Storagepool controller) can
   then drop `StoredObject` from their reconcile paths. Tracked as future
   work in this proposal, not in this change.

No server migration, data migration, or deployment ordering is required.
The change is purely additive client-side.

## Open Questions

None. The proposal, the existing `TypedResource` trait, and the raw helpers
are concrete enough that all implementation-relevant decisions are resolved.
