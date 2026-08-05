## 1. Typed Finalizer Helpers

- [x] 1.1 Add `ensure_finalizer<T: TypedResource>(client: &KapiClient, obj: &T, finalizer: &str)` to `kapi-controller/src/finalizer.rs`. The function SHALL call `obj.to_stored_object()?` once, then delegate to the existing raw `ensure_finalizer` helper. Add doc comments cross-referencing the raw helper and explaining that CAS re-fetches operate on `StoredObject`, not `T`.
- [x] 1.2 Add `remove_finalizer<T: TypedResource>(client: &KapiClient, obj: &T, finalizer: &str)` with the same delegation pattern. Doc comments SHALL mirror 1.1.
- [x] 1.3 Re-export `TypedResource` (if not already) in the `kapi-controller` public API surface so callers don't need to add a separate `kapi-client` import for the trait bound. Verify via `cargo doc --no-deps`. **Skipped**: Controllers already import `kapi-client` for `KapiClient` and typed resource definitions, so re-exporting `TypedResource` from `kapi-controller` adds no practical value. Callers import the trait directly from `kapi_client::TypedResource`.

## 2. Unit Tests

- [x] 2.1 Add a minimal mock `TypedResource` impl in `kapi-controller/src/finalizer.rs`'s `#[cfg(test)]` module. The mock SHALL expose a controllable finalizer list and a `to_stored_object` that can be made to fail for the serialization-error test.
- [x] 2.2 Add test: typed `ensure_finalizer` on a resource that already has the finalizer returns `Ok(())` without any client interaction (no-op path).
- [x] 2.3 Add test: typed `ensure_finalizer` on a resource that lacks the finalizer delegates correctly — verify by inspecting the resulting stored object's finalizer list.
- [x] 2.4 Add test: typed `remove_finalizer` on a resource that lacks the finalizer returns `Ok(())` (no-op path).
- [x] 2.5 Add test: typed `remove_finalizer` on a resource marked for deletion with the finalizer present delegates correctly.
- [x] 2.6 Add test: typed helpers propagate `TypedError::Serialization` when `to_stored_object` fails, without calling the client.
- [x] 2.7 Add test: existing raw `ensure_finalizer` / `remove_finalizer` with `&StoredObject` continue to compile and behave identically (regression guard for backward compatibility).

## 3. kapibuild Controller Generator

- [x] 3.1 Update the controller template in `kapibuild/src/controller_generate.rs` to emit `finalizer::ensure_finalizer(&ctx.client, resource, FINALIZER_NAME)` and `finalizer::remove_finalizer(&ctx.client, resource, FINALIZER_NAME)` directly — remove the intermediate `let stored = resource.to_stored_object()?;` binding.
- [x] 3.2 Update the generator's golden test / snapshot fixtures to reflect the new call shape (assert `finalizer::ensure_finalizer` is present and that no `to_stored_object` appears in the generated reconcile path).
- [x] 3.3 Verify the generated controller compiles against the updated `kapi-controller` API by running the generator's own test suite.

## 4. Documentation and Hygiene

- [x] 4.1 Inspect the existing `docs/` directory for any finalizer or controller-runtime documentation and update it to mention the typed helpers and the preferred call pattern for typed controllers.
- [x] 4.2 Check the project roadmap for items related to typed finalizer helpers or controller ergonomics; update or remove entries that this change resolves.
- [x] 4.3 Check the `kapi-e2e-tests` skill for any finalizer-related end-to-end scenarios and add or update coverage if the typed controller workflow meaningfully changes the observable behaviour (likely no change needed — typed helpers are a client-side ergonomic improvement, server semantics are unchanged).

## 5. Verification

- [x] 5.1 Run `cargo fmt --all -- --check`.
- [x] 5.2 Run `cargo check --workspace`.
- [x] 5.3 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] 5.4 Run `cargo test --workspace` and verify all tests pass.
