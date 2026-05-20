# kapi — Data Model

## Core Types

### ResourceKey

Identifies a resource type within the API. Every object belongs to a group, version, and kind.

```rust
struct ResourceKey {
    group: String,    // e.g. "example.io"
    version: String,  // e.g. "v1"
    kind: String,     // e.g. "Widget"
}
```

- `Hash` + `Eq` — used as a map key for per-kind event channels
- Serialized as-is on the wire (no renaming)

### ObjectMetadata

Server-managed lifecycle fields. Clients receive these and echo them back on updates, but never interpret them.

```rust
struct ObjectMetadata {
    name: String,             // unique within a (group, version, kind)
    resource_version: u64,    // monotonic, global counter
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
```

Wire format uses camelCase: `resourceVersion`, `createdAt`, `updatedAt`.

### UserData

Envelope for the user's domain payload. This is what gets validated against the registered JSON Schema.

```rust
struct UserData {
    value: serde_json::Value,
}
```

### StoredObject

The complete unit of storage and retrieval. Everything stored in the system is a `StoredObject`.

```rust
struct StoredObject {
    key: ResourceKey,
    metadata: ObjectMetadata,
    data: UserData,
}
```

### WatchEvent

Notification payload for real-time change events over SSE.

```rust
enum WatchEventType { Added, Modified, Deleted }

struct WatchEvent {
    event_type: WatchEventType,
    object: StoredObject,
}
```

### SchemaData

Payload for Schema registration requests.

```rust
struct SchemaData {
    target_group: String,     // e.g. "example.io"
    target_version: String,   // e.g. "v1"
    target_kind: String,      // e.g. "Widget"
    json_schema: Value,       // valid JSON Schema (Draft 2020-12)
}
```

Wire format uses camelCase: `targetGroup`, `targetVersion`, `targetKind`, `jsonSchema`.

### Pagination Types

```rust
struct ContinueToken(String);   // opaque, base64-encoded cursor

struct ListOptions {
    limit: Option<usize>,
    continue_token: Option<ContinueToken>,
}

struct ListResponse {
    items: Vec<StoredObject>,
    continue_token: Option<ContinueToken>,
}
```

### ValidationError

Field-level validation failure returned in 422 responses.

```rust
struct ValidationError {
    path: String,     // JSON pointer to the offending field
    message: String,  // human-readable error description
}
```

## Error Model

All errors conform to a standard JSON envelope:

```json
{
    "error": "description",
    "code": "ErrorCode",
    "details": { }
}
```

| Error Code | HTTP Status | When |
|------------|-------------|------|
| `NotFound` | 404 | Object or Schema not found |
| `Conflict` | 409 | OCC version mismatch |
| `SchemaValidation` | 422 | Object data violates registered schema |
| `InvalidSchema` | 422 | Schema registration fails meta-schema or compilation |
| `SchemaHasObjects` | 409 | Attempting to delete a Schema that has existing objects |
| `Internal` | 500 | Unexpected errors |

## Watch Semantics

- Add `?watch=true` to a list request to receive an SSE stream instead of JSON
- Stream delivers `WatchEvent` messages as server-sent events
- Events use the `event: message` type in SSE
- If a client falls behind (broadcast channel buffer overflow), the stream terminates with `None` — the client must re-sync by re-listing + re-subscribing
- Channels are created lazily on first subscribe, cleaned up when all receivers drop

### SSE Wire Format

```
event: message
data: {"eventType":"Added","object":{...}}

event: message
data: {"eventType":"Modified","object":{...}}
```

## Wire Format Example

```json
{
    "key": {
        "group": "apps",
        "version": "v1",
        "kind": "deployments"
    },
    "metadata": {
        "name": "my-app",
        "resourceVersion": 42,
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    },
    "data": {
        "replicas": 3
    }
}
```

## Pagination

- List results are sorted alphabetically by name
- `limit` controls page size (unlimited if omitted)
- `continue` token (cursor) is a base64-encoded name for the next page
- Responses include `continueToken` when more results exist
- To paginate: list with limit, extract `continueToken`, pass it as `continue` in the next request

## Optimistic Concurrency

- Every `StoredObject` carries a `resourceVersion` (monotonic global u64 counter)
- Updates require the client to send back the `resourceVersion` from the object they last read
- If the stored version doesn't match, the server returns `409 Conflict`
- The client must re-fetch the object and retry with the updated version
- Deletes are unconditional (no version check)
