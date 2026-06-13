## Why

The create API has an asymmetry: POST accepts flat fields (no `spec` key), but GET and PUT use the full `StoredObject` shape with nested `spec`. The handler silently strips `metadata` and `status`, treating the remainder as `spec`. This creates confusion, hides typos, and contradicts the Kubernetes convention this project is inspired by.

## What Changes

- **BREAKING**: POST create for non-Schema objects now requires a `spec` field containing the domain data
- **BREAKING**: Unknown top-level fields in create requests are rejected with 400 Bad Request
- **BREAKING**: Empty `spec` objects (`{}`) are rejected with 400 Bad Request
- Add `AppError::InvalidRequestBody(String)` variant mapped to HTTP 400 for client input errors
- Schema object creation remains unchanged (keeps flat format)

**Before:**
```json
POST /apis/example.io/v1/Widget
{
  "metadata": { "name": "my-widget" },
  "color": "blue",
  "size": 42
}
```

**After:**
```json
POST /apis/example.io/v1/Widget
{
  "metadata": { "name": "my-widget" },
  "spec": {
    "color": "blue",
    "size": 42
  }
}
```

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `object-handlers`: Create handler now requires `spec` field, rejects unknown top-level fields, rejects empty spec. New error variant `InvalidRequestBody` for validation failures.

## Impact

- **API**: Breaking change to POST create endpoint for non-Schema objects
- **Handler**: `src/object/handler.rs` — replace strip-metadata logic with spec extraction and validation
- **Error handling**: New `AppError::InvalidRequestBody(String)` variant → HTTP 400
- **Tests**: ~33 test bodies need `spec` wrapper (21 via `widget()` helper, 12 inline)
- **No change**: Schema validation, update handler, events, store

## Non-Goals

- Changing Schema object creation (remains flat format)
- Changing update (PUT) request shape (already uses `spec`)
- Adding partial update / PATCH support
- Changing the internal `StoredObject` structure
