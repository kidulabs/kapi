## Context

`TypedClient<T>::get` currently converts the server’s `StoredObject` into `T` and discards the raw value. Controllers that use finalizers then perform a second GET because the existing finalizer helpers accept `StoredObject`. Generated resources already expose metadata, system metadata, spec, and status through `TypedResource`, so lifecycle inspection and conversion can be provided at the typed-resource boundary.

## Goals / Non-Goals

**Goals:**

- Let typed controllers inspect deletion state and finalizer membership without raw-object access.
- Let a typed resource be converted back to a `StoredObject` for existing raw finalizer update helpers.
- Keep the API-server finalizer protocol and optimistic-concurrency behavior unchanged.
- Avoid new dependencies and avoid changes to generated resource implementations where defaults suffice.

**Non-Goals:**

- Moving network-backed finalizer mutation or retry loops into `TypedResource`.
- Changing server deletion semantics or the raw `kapi-controller::finalizer` API.
- Preserving arbitrary unknown JSON fields that were not represented by the typed spec/status types.

## Decisions

### Decision 1: Add default helpers to `TypedResource`

Add default methods for `is_deleting`, `has_finalizer`, and `to_stored_object`. The first two read existing metadata; the conversion clones key/metadata/system fields and serializes spec/status using the existing serde strategy.

**Alternative considered:** Add controller-specific helper functions only. This would leave every typed controller to repeat the same lifecycle and conversion logic.

### Decision 2: Keep raw finalizer mutation helpers

`ensure_finalizer` and `remove_finalizer` remain in `kapi-controller` and continue to accept `StoredObject`. Controllers can pass `resource.to_stored_object()` without introducing a dependency from the controller helper module into typed-resource generics.

**Alternative considered:** Make the finalizer module generic over `TypedResource`. This couples controller lifecycle utilities to serialization bounds and expands the API surface without changing server behavior.

### Decision 3: Use `TypedError` for conversion failures

`to_stored_object` returns `Result<StoredObject, TypedError>`, mapping spec/status serialization failures to the existing `TypedError::Serialization` variant. This matches all other typed-client operations.

**Alternative considered:** Return `serde_json::Error` directly. That would make callers handle a different error type from the rest of the typed client.

## Risks / Trade-offs

- **Typed conversion can omit unknown fields** → Treat generated typed resources as the authoritative representation and document the behavior; raw clients remain available when lossless passthrough is required.
- **Trait API growth** → Use default methods so existing third-party implementations remain source-compatible.
- **Local serialization cost** → Accept one local serialization round trip to avoid an additional network GET; benchmark only if real workloads show a bottleneck.

## Migration Plan

Add the default methods and tests in `kapi-client`, then update typed controllers to use them. No server migration or data migration is required. Existing raw finalizer callers continue to work unchanged.

## Open Questions

- Whether the conversion helper should be named `to_stored_object` or `into_stored_object` in the final public API.
- Whether `has_finalizer` belongs on `TypedResource` or remains a controller-level convenience once `metadata()` is available.
