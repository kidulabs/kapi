## Why

Format validation (label regex, annotation size limits) currently runs inside `ObjectService` after the handler has already extracted labels and annotations. This means invalid requests travel through the full request pipeline — handler extraction, service invocation, schema registry lookup — before failing on checks that could have run immediately after extraction. Moving stateless format validation to the handler edge provides fail-fast behavior: bad input dies before any I/O or orchestration work begins.

## What Changes

- Extract `validate_labels`, `validate_annotations`, and supporting functions (`validate_label_key`, `validate_label_value`, `validate_annotation_key`) from `object/service.rs` into a new `src/validation/` module
- Handler calls these validation functions eagerly after extraction in `create()` and `update()` handlers
- Service retains validation calls as defense-in-depth (no behavior change for non-HTTP callers)
- Convert regex compilations in validation functions to `LazyLock<Regex>` statics to avoid recompilation on every call
- Update handler module documentation to reflect the new responsibility: "Handlers validate input format and deserialization constraints. They never access the store, event bus, or schema registry, and never contain conditional mutation logic."

## Capabilities

### New Capabilities
- `validation-module`: Stateless format validation functions (label/annotation regex, length limits, total size) extracted into a dedicated module callable from both handler and service layers

### Modified Capabilities
- `object-handlers`: Handlers SHALL call format validation functions eagerly after extraction, before invoking the service. This applies to both `create()` and `update()` handlers.
- `object-service`: Service SHALL retain format validation calls as defense-in-depth, ensuring non-HTTP callers (tests, future gRPC/CLI) receive the same validation guarantees.

## Impact

- **Code**: New `src/validation/` module; modifications to `src/object/handler.rs` (add validation calls) and `src/object/service.rs` (import from validation module)
- **APIs**: No API behavior change — same error types, same HTTP status codes
- **Dependencies**: None (uses `std::sync::LazyLock` from std)
- **Performance**: Improved — invalid requests fail faster; regex compilation overhead eliminated via `LazyLock`
- **Tests**: Existing integration tests continue to pass (validation still happens). Add handler-level unit tests for the new validation calls.

## Non-Goals

- Moving stateful validation (schema lookup, JSON Schema validation, OCC checks, deletion guards) to the handler — these require store/cache access and remain in the service
- Changing validation rules or error messages — the same functions, same logic, just called from two places
- Removing service-level validation — defense-in-depth is intentional for API safety

## Future Work

- Consider extracting other format checks (spec shape validation, selector parsing) into the validation module for consistency
- Add request-level metrics to measure fail-fast impact (requests rejected at handler vs service)
