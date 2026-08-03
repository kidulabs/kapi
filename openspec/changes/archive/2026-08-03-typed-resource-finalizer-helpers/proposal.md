## Why

Typed controllers currently fetch a resource through `TypedClient`, then fetch the same object again as a raw `StoredObject` solely to inspect deletion metadata or invoke finalizer helpers. This adds an unnecessary API round trip and makes typed controllers depend on the raw object representation. The existing `TypedResource` contract already owns the resource metadata and system metadata needed for these lifecycle checks.

## What Changes

- Add default lifecycle helpers to `TypedResource` for checking deletion state and finalizer presence.
- Add a supported typed-to-`StoredObject` conversion helper so controllers can reuse the already-fetched resource when raw finalizer mutation APIs are required.
- Expose the conversion through the typed client/typed-resource API without changing the server-side finalizer protocol.
- Preserve optimistic-concurrency behavior and existing raw finalizer mutation helpers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `kapibuild-typed-client`: extend typed resources with lifecycle inspection and conversion helpers for controller finalizer workflows.

## Impact

- **API:** Public additions to `kapi-client::TypedResource` and/or its typed conversion API. Implementations and generated resources must continue to satisfy the trait.
- **Controllers:** Typed controllers can perform deletion checks without a second GET and can convert the fetched typed resource when raw finalizer updates are still needed.
- **Compatibility:** No changes to API-server deletion behavior, `ObjectMeta`, `SystemMetadata`, or the raw `kapi-controller::finalizer` update protocol.
- **Testing:** Add typed-resource unit tests covering deletion timestamps, finalizer presence, and conversion fidelity.

## Non-goals

- Changing server-side finalizer semantics or deletion ordering.
- Moving optimistic-concurrency finalizer update logic into `TypedResource`.
- Making every server error or finalizer operation a typed-resource method.
