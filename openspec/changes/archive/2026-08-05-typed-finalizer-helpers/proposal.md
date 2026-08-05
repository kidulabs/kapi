## Why

The controller-side finalizer helpers in `kapi-controller` — `ensure_finalizer`
and `remove_finalizer` — accept `&StoredObject`. This forces typed controllers
to construct a raw `StoredObject` via `TypedResource::to_stored_object()` and
carry it through the reconcile path, even though all they actually want is
"add or remove a finalizer on this typed resource." The raw conversion is
lossless — the typed resource already exposes everything the helpers need —
so it is pure ceremony. With kapi 0.3.1 the `TypedResource` trait now has
`is_deleting`, `has_finalizer`, and `to_stored_object`, so the typed-side
building blocks are in place; the remaining seam is the finalizer
**mutation** helpers.

## What Changes

- **Add typed overloads in `kapi-controller::finalizer`:**
  `ensure_finalizer<T: TypedResource>(client: &KapiClient, obj: &T, finalizer: &str)`
  and the same shape for `remove_finalizer`. Internally, these call
  `obj.to_stored_object()?` once, then delegate to the existing CAS-retry
  loop. The raw `&StoredObject` helpers remain untouched for raw-client
  users.
- **Add a spec `typed-finalizer-helpers`** covering the new typed helpers:
  semantics identical to the raw variants (idempotent add, idempotent
  remove, CAS retry on 409 up to 5 attempts), plus a failure case where
  `to_stored_object` serialization errors propagate as the returned error.
- **Add unit tests** for the typed helpers using a mock `TypedResource`,
  covering: already-present finalizer (no-op), absent finalizer (add),
  absent finalizer on delete (no-op), present finalizer on delete (remove),
  and serialization failure.
- **Update `kapibuild` controller generator template** to emit code that
  uses the typed helpers directly, so newly generated controllers never
  see `StoredObject`.

## Capabilities

### New Capabilities

- `typed-finalizer-helpers`: Generic finalizer mutation helpers for typed
  controllers — `ensure_finalizer<T: TypedResource>` and
  `remove_finalizer<T: TypedResource>`.

### Modified Capabilities

None. The raw finalizer helpers and all server-side finalizer semantics
remain unchanged.

## Impact

- **Code.** `kapi-controller/src/finalizer.rs` gains two generic functions
  plus tests. The raw helpers are preserved for backward compatibility.
- **API.** Non-breaking — additive. Raw helpers still work; typed helpers
  are new public API.
- **Dependencies.** None. `kapi-controller` already depends on
  `kapi-client` (which re-exports `TypedResource`), so the `T: TypedResource`
  bound adds no new edge.
- **Controllers.** Any typed controller can drop `StoredObject` from the
  reconcile path and call `ensure_finalizer` / `remove_finalizer` with the
  typed resource directly.
- **Prerequisite.** Requires kapi >= 0.3.1, which already ships
  `TypedResource::to_stored_object`.

## Non-goals

- Changing server-side finalizer semantics or the deletion ordering —
  covered by the existing `finalizer-support` spec.
- Removing the raw `&StoredObject` helpers — they remain useful for
  raw-client code paths.
- Rewriting the CAS retry loop — the typed helpers delegate to the
  existing one.

## Future Work

- Once typed helpers ship, downstream controllers (including kcloud's
  Storagepool controller) can drop `StoredObject` from the reconcile path.
  Tracked in kcloud's change `typed-finalizer-helpers`.
