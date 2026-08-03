## 1. Typed Resource API

- [x] 1.1 Add default `is_deleting` and `has_finalizer` lifecycle helpers to `TypedResource`.
- [x] 1.2 Add the typed-to-`StoredObject` conversion helper using the existing serde and `TypedError` conventions.
- [x] 1.3 Keep existing `TypedClient` conversion paths consistent with the new trait helper and preserve raw finalizer mutation APIs.

## 2. Generated Controller Adoption

- [x] 2.1 Update the kapibuild controller generator template to use typed lifecycle helpers and local conversion instead of fetching the same object twice.
- [x] 2.2 Ensure generated controllers remove finalizers only after successful cleanup.
- [x] 2.3 Regenerate or update generator golden tests and verify generated controller output compiles.

## 3. Tests and Compatibility

- [x] 3.1 Add unit tests for deletion-state detection and finalizer-presence helpers.
- [x] 3.2 Add conversion tests covering key, metadata, system metadata, spec, status, and serialization failures.
- [x] 3.3 Check the `kapi-e2e-tests` skill and add or update end-to-end coverage if the typed controller workflow requires it.
- [x] 3.4 Verify existing `TypedResource` implementations compile without implementing the new default methods.

## 4. Documentation and Project Hygiene

- [x] 4.1 Inspect the existing `docs/` directory and update typed-client/finalizer documentation if needed.
- [x] 4.2 Check the project roadmap for impacted typed-client or controller-runtime items and update it if necessary.
- [x] 4.3 Document the distinction between typed lifecycle inspection and raw optimistic-concurrency finalizer mutation.

## 5. Verification

- [x] 5.1 Run `cargo fmt --all -- --check`.
- [x] 5.2 Run `cargo check --workspace`.
- [x] 5.3 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] 5.4 Run `cargo test --workspace` and verify all tests pass.
