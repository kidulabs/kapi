# kapi — API Reference

## Base URL

API paths follow the pattern `/apis/{group}/{version}/{kind}` for cluster-scoped resources and `/apis/{group}/{version}/namespaces/{namespace}/{kind}` for namespace-scoped resources.

### Scope Model

Each Schema can declare a `scope` of `"Namespaced"` (default) or `"Cluster"`:

- **Namespaced** resources (e.g., `NamespacedWidget`) can be created/managed via namespace-scoped URLs or cluster-scoped URLs (which default to the `"default"` namespace).
- **Cluster-scoped** resources (e.g., `ClusterWidget`, `Schema`) are only accessible via cluster-scoped URLs. Using a namespace-scoped URL on a cluster-scoped kind returns `400 InvalidRequest`.
- **Schema** itself is always cluster-scoped — its URLs never include a namespace segment.

## Schema Registry

Schemas define the JSON Schema that objects of a given kind must conform to. They are themselves stored as objects of kind `Schema` in group `kapi.io`.

### Register a Schema

Creates a new Schema. The name is auto-generated as `{targetKind}.{targetGroup}.{targetVersion}`.

```
POST /apis/kapi.io/v1/Schema
```

**Request body:**

```json
{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "jsonSchema": {
        "type": "object",
        "properties": {
            "color": { "type": "string" },
            "size": { "type": "integer" }
        },
        "required": ["color"]
    },
    "statusSchema": {
        "type": "object",
        "properties": {
            "phase": { "type": "string" }
        }
    }
}
```

**Response:** `201 Created`

```json
{
    "key": {
        "group": "kapi.io",
        "version": "v1",
        "kind": "Schema"
    },
    "metadata": {
        "name": "Widget.example.io.v1",
        "labels": {}
    },
    "system": {
        "resourceVersion": 1,
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    },
    "spec": {
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "jsonSchema": { ... }
    }
}
```

**Errors:** `422` (invalid schema), `409` (duplicate name)

### List Schemas

```
GET /apis/kapi.io/v1/Schema
```

**Response:** `200 OK`

```json
{
    "items": [ /* StoredObject array */ ],
    "continueToken": null
}
```

### Get a Schema

```
GET /apis/kapi.io/v1/Schema/{name}
```

`name` is the auto-generated name, e.g. `Widget.example.io.v1`. The name includes the `targetVersion` component to distinguish schemas for the same kind across different API versions.

**Response:** `200 OK` — single StoredObject

**Error:** `404` (not found)

### Delete a Schema

```
DELETE /apis/kapi.io/v1/Schema/{name}
```

**Response:** `200 OK` — the deleted StoredObject

**Error:** `409` if objects of the target kind still exist, `404` if not found

---

## Object CRUD

### Create an Object

Cluster-scoped (for cluster-scoped kinds or cross-namespace):

```
POST /apis/{group}/{version}/{kind}
```

Namespace-scoped (for namespace-scoped kinds):

```
POST /apis/{group}/{version}/namespaces/{namespace}/{kind}
```

**Request body:**

```json
{
    "metadata": {
        "name": "my-widget",
        "labels": {
            "app.example.io/name": "my-widget",
            "tier": "frontend"
        },
        "annotations": {
            "description": "Production deployment",
            "owner": "team-platform"
        }
    },
    "spec": {
        "color": "blue",
        "size": 10
    }
}
```

**Key behaviors:**

- **Scope validation**: The server looks up the schema's scope. Cluster-scoped kinds reject namespace-scoped URLs with `400 InvalidRequest`. Namespaced kinds on cluster-scoped URLs default to the `"default"` namespace.
- **Namespace precedence**: The namespace from the URL path always takes precedence over any `metadata.namespace` in the request body. The body `namespace` field is ignored if present.
- The `metadata.name` field is extracted by the handler. The optional `metadata.labels` field is extracted and validated (see [Label Validation](#label-validation)). The optional `metadata.annotations` field is extracted and validated (see [Annotation Validation](#annotation-validation)).
- The `spec` field contains the domain data, validated against the registered JSON Schema. Only `metadata` and `spec` are allowed as top-level fields; unknown fields are rejected with 400 Bad Request.

**Response:** `201 Created`

```json
{
    "key": {
        "group": "example.io",
        "version": "v1",
        "kind": "Widget"
    },
    "metadata": {
        "name": "my-widget",
        "labels": {
            "app.example.io/name": "my-widget",
            "tier": "frontend"
        }
    },
    "system": {
        "resourceVersion": 1,
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    },
    "spec": {
        "color": "blue",
        "size": 10
    }
}
```

### List Objects

Cluster-scoped (includes cross-namespace for namespaced kinds):

```
GET /apis/{group}/{version}/{kind}
```

Namespace-scoped:

```
GET /apis/{group}/{version}/namespaces/{namespace}/{kind}
```

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `watch` | boolean | Enable SSE watch stream |
| `limit` | integer | Page size for paginated listing |
| `continue` | string | Cursor from previous page (base64-encoded) |
| `fieldSelector` | string | Filter results by field selector. On list requests, filters returned objects. On watch requests, filters the event stream. |
| `labelSelector` | string | Filter results by label selector. On list requests, filters returned objects. On watch requests, filters the event stream. When both are present on watch, they are combined with AND semantics. |

**Response:** `200 OK`

```json
{
    "items": [ /* StoredObject array */ ],
    "continueToken": "aX ... base64 ..."
}
```

### Get an Object

```
GET /apis/{group}/{version}/{kind}/{name}                                         (cluster-scoped)
GET /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}                  (namespace-scoped)
```

**Response:** `200 OK` — single StoredObject

**Error:** `404`

### Update an Object

Requires the full StoredObject with the correct `system.resourceVersion`.

```
PUT /apis/{group}/{version}/{kind}/{name}                                        (cluster-scoped)
PUT /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}                 (namespace-scoped)
```

**Request body:** Full StoredObject with updated `spec` and optionally updated `metadata.labels` and `metadata.annotations`.

```json
{
    "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
    "metadata": {
        "name": "my-widget",
        "labels": {
            "app.example.io/name": "my-widget",
            "tier": "backend"
        }
    },
    "system": {
        "resourceVersion": 1,
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    },
    "spec": {
        "color": "red",
        "size": 20
    }
}
```

Labels and annotations are updated by the client sending the full `StoredObject` with the desired values. The server replaces the stored metadata with the provided values.

**Response:** `200 OK` — updated StoredObject with bumped `system.resourceVersion`

**Errors:** `409` (version conflict), `422` (validation failure), `400` (invalid labels), `404`

### Delete an Object

```
DELETE /apis/{group}/{version}/{kind}/{name}                                       (cluster-scoped)
DELETE /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}                (namespace-scoped)
```

**Response:** `200 OK` — the deleted StoredObject

**Errors:** `404`

#### Finalizer Support

When an object has `metadata.finalizers` (a list of strings), DELETE behaves differently:

- **Empty finalizers** → object is hard-deleted immediately, `Deleted` event published
- **Non-empty finalizers** → object is marked for deletion (`system.deletionTimestamp` is set), `Modified` event published, object still exists
- **Already deleting** (`deletionTimestamp` already set) → idempotent 200, no event

Controllers watch for objects with `deletionTimestamp` set, perform cleanup, then remove their finalizer via UPDATE. When all finalizers are removed, the object is hard-deleted.

**Constraints while `deletionTimestamp` is set:**
- Only `metadata.finalizers` can be modified (all other changes → 409 `ObjectBeingDeleted`)
- Cannot add new finalizers (only removal allowed)
- Cannot create a new object with the same name (→ 409 `AlreadyExists`)

**Finalizer validation:** Max 20 finalizers per object. Names must be label-key-shaped (e.g., `example.io/cleanup`). Invalid names → 400 `InvalidFinalizer`.

---

## Watch (SSE)

Add `?watch=true` to any list request to receive an SSE stream of real-time events.

```
GET /apis/example.io/v1/Widget?watch=true                                          (cluster-scoped / cross-namespace)
GET /apis/example.io/v1/namespaces/staging/NamespacedWidget?watch=true            (namespace-scoped)
```

**Namespace-aware watching:**
- Namespace-scoped watch streams receive only events for objects in the specified namespace
- Cluster-scoped watch streams receive events from all namespaces (cross-namespace watch)

Stream delivers `WatchEvent` messages:

```
event: message
data: {"eventType":"Added","object":{...}}
```

### Filtering by field selector

Add `?fieldSelector=metadata.name=<name>` to watch only events for a specific object:

```
GET /apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-widget
```

Only events where `object.metadata.name == "my-widget"` will be delivered. The syntax follows the standard convention: `fieldSelector=metadata.name=<value>`.

**Supported fields:**
| Field | Description |
|-------|-------------|
| `metadata.name` | Filter by exact object name |

**Errors:** `400` for unsupported fields or malformed syntax (missing `=` sign).

### Filtering by label selector

Add `?labelSelector=<selector>` to watch only events for objects matching specific labels:

```
GET /apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx
```

Label selectors are evaluated against `object.metadata.labels`. An empty selector (`labelSelector=`) matches all objects.

**Supported syntax:**
| Syntax | Description | Example |
|--------|-------------|---------|
| `key=value` | Equality — label key must exist with exact value | `app=nginx` |
| `key!=value` | Inequality — label key must not have this value **or must be absent** | `env!=prod` |
| `key` | Existence — label key must be present (any value) | `gpu` |
| `!key` | Non-existence — label key must not be present | `!experimental` |
| Comma-separated | AND combinator — all requirements must match | `app=nginx,env=prod` |

**Matching semantics:**

- `key=value` requires the key to exist **and** have the specified value. Objects without the key do not match.
- `key!=value` matches when the key has a different value **or when the key is absent entirely**. This follows Kubernetes semantics — absence satisfies inequality.
- `key` (existence) matches when the key is present, even if its value is an empty string.
- `!key` (non-existence) matches only when the key is not present in the labels map.
- Comma-separated requirements use AND semantics — all must match for the event to be delivered.

**Examples:**

```
# Watch objects with app=nginx
?watch=true&labelSelector=app=nginx

# Watch objects with app=nginx AND env=prod
?watch=true&labelSelector=app=nginx,env=prod

# Watch objects without the experimental label
?watch=true&labelSelector=!experimental

# Mixed operators
?watch=true&labelSelector=app=nginx,!experimental,gpu
```

**Combining with fieldSelector:**

When both `fieldSelector` and `labelSelector` are present on a watch request, they are combined with AND semantics — both must match for an event to be delivered. On list requests, both selectors are applied as filters to the returned objects.

**Errors:** `400` for malformed selectors (empty value, whitespace in key, empty segments).

Watch streams terminate when the client disconnects or the watcher's buffer is full. The client must re-list and re-subscribe.

---

## Label Validation

Labels on `ObjectMeta` follow structured validation rules. Invalid labels cause the request to be rejected with `400 Bad Request` and error code `InvalidLabel`.

### Key Rules

| Rule | Constraint |
|------|------------|
| Non-empty | Key must not be empty |
| Max length | 256 characters (including prefix if present) |
| Name format | Must match `[a-zA-Z0-9][-_.a-zA-Z0-9]*` |
| Optional prefix | `{prefix}/` — separated by `/` |
| Prefix format | DNS subdomain: lowercase alphanumeric segments separated by dots, max 253 chars |
| Prefix segments | Each segment matches `[a-z0-9]([-a-z0-9]*[a-z0-9])?` |

**Valid key examples:**

| Key | Notes |
|-----|-------|
| `app` | Simple name only |
| `my-label` | Hyphen allowed in name |
| `app.example.io/name` | Prefix + name |
| `example.com/tier` | Prefix with dot separators |
| `label_name.v2` | Underscore and dot in name |

**Invalid key examples:**

| Key | Reason |
|-----|--------|
| `` (empty) | Must not be empty |
| `UPPERCASE/name` | Prefix must be lowercase DNS subdomain |
| `/name` | Empty prefix before `/` |
| `key with spaces` | Spaces not allowed in name |
| `key!` | Special characters not allowed |

### Value Rules

| Rule | Constraint |
|------|------------|
| Empty allowed | Empty string is a valid value |
| Max length | 256 characters |
| Format | Must match `[a-zA-Z0-9][-_.a-zA-Z0-9]*` when non-empty |

**Valid value examples:** `prod`, `v1.2.3`, `abc`, `` (empty)

**Invalid value examples:** ` value` (leading space), `my value` (space), `value!` (special char)

### Error Response

```json
{
    "error": "invalid label: label key 'invalid key!' contains invalid characters",
    "code": "InvalidLabel",
    "details": {
        "message": "label key 'invalid key!' contains invalid characters"
    }
}
```

---

## Annotation Validation

Annotations on `ObjectMeta` follow minimal validation rules. Invalid annotations cause the request to be rejected with `400 Bad Request` and error code `InvalidAnnotation`.

### Key Rules

| Rule | Constraint |
|------|------------|
| Non-empty | Key must not be empty |
| Max length | 256 characters |
| Character restrictions | None — any Unicode characters allowed |

### Value Rules

| Rule | Constraint |
|------|------------|
| Any string allowed | Values accept any string, including empty strings |
| Per-value length | No per-value limit (total size limit applies instead) |
| Character restrictions | None |

### Total Size Limit

The total serialized size of all annotations for a single object must not exceed **256KB**. This prevents abuse while allowing flexibility for arbitrary metadata.

### Valid Examples

| Key | Value | Notes |
|-----|-------|-------|
| `description` | `My widget` | Simple key-value |
| `kapi.io/last-applied-config` | `{}` | Prefixed key (no prefix validation) |
| `example.com/path@v1` | `data` | Special characters in key |
| `build-url` | `https://example.com/path?query=value` | URL in value |
| `config` | `{"key": "value", "nested": true}` | JSON in value (stored as string) |

### Invalid Examples

| Key | Value | Reason |
|-----|-------|--------|
| `` (empty) | `value` | Empty key |
| `a...a` (257 chars) | `value` | Key exceeds 256 character limit |
| `key` | `x...x` (>256KB total) | Total serialized size exceeds 256KB |

### Error Response

```json
{
    "error": "invalid annotation: annotation key '' exceeds maximum length of 256 characters",
    "code": "InvalidAnnotation",
    "details": {
        "message": "annotation key '' exceeds maximum length of 256 characters"
    }
}
```

---

## Status Subresource

The status subresource provides a separate write path for controller-runtime semantics. Controllers write observed state to `status` while users write desired state to `spec`. Status updates do not use optimistic concurrency control.

### Get Status

```
GET /apis/{group}/{version}/{kind}/{name}/status                                  (cluster-scoped)
GET /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status           (namespace-scoped)
```

**Response:** `200 OK` — returns the status as an inline JSON value, or `null` if not set.

```json
{
    "phase": "Running",
    "message": "All systems go"
}
```

**Response:** `404 Not Found` — if the kind does not have a `statusSchema` defined.

```json
{
    "error": "status subresource not enabled for kind 'Widget'",
    "code": "StatusSubresourceNotEnabled",
    "details": { "kind": "Widget" }
}
```

### Update Status

```
PUT /apis/{group}/{version}/{kind}/{name}/status                                  (cluster-scoped)
PUT /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status           (namespace-scoped)
```

**Request body:**

```json
{
    "status": {
        "phase": "Running",
        "message": "All systems go"
    }
}
```

**Response:** `200 OK` — returns the full `StoredObject` with updated status.

```json
{
    "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
    "metadata": { "name": "my-widget", "labels": {} },
    "system": { "resourceVersion": 5, "createdAt": "...", "updatedAt": "..." },
    "spec": { "color": "blue", "size": 10 },
    "status": { "phase": "Running", "message": "All systems go" }
}
```

**Response:** `404 Not Found` — if the kind does not have a `statusSchema` defined.

**Response:** `422 Unprocessable Entity` — if the status fails validation against `statusSchema`.

```json
{
    "error": "object data violates schema",
    "code": "SchemaValidation",
    "details": {
        "errors": [
            { "path": "phase", "message": "expected string, got integer" }
        ]
    }
}
```

---

## OpenAPI

### GET /openapi

Returns a dynamically generated OpenAPI 3.0.3 specification based on all registered Schemas.

**Response:** `200 OK` — OpenAPI document as JSON

### GET /swagger-ui

Serves Swagger UI loaded from CDN, configured to read from `/openapi`.

---

## Error Responses

All errors follow this format:

```json
{
    "error": "human-readable message",
    "code": "ErrorCode",
    "details": {}
}
```

| Status | Code | Description |
|--------|------|-------------|
| 400 | `InvalidRequest` | Namespace mismatch (URL vs body), or cluster-scoped kind used with namespace URL |
| 404 | `NotFound` | Resource not found |
| 404 | `StatusSubresourceNotEnabled` | Status subresource accessed for kind without statusSchema |
| 409 | `Conflict` | OCC version mismatch or duplicate |
| 409 | `SchemaHasObjects` | Cannot delete schema with existing objects |
| 409 | `ObjectBeingDeleted` | Object is being deleted; only finalizer modifications are allowed |
| 400 | `InvalidFieldSelector` | Invalid fieldSelector query parameter (unsupported field or malformed syntax) |
| 400 | `InvalidLabelSelector` | Invalid labelSelector query parameter (malformed syntax, empty value) |
| 400 | `InvalidAnnotation` | Annotation key is empty, exceeds 256 chars, or total size exceeds 256KB |
| 400 | `InvalidLabel` | Label key or value violates format or length rules |
| 400 | `InvalidFinalizer` | Finalizer name is invalid or too many finalizers (max 20) |
| 400 | `InvalidRequestBody` | Request body validation failed (missing spec, unknown fields, empty spec) |
| 422 | `SchemaValidation` | Object data doesn't match schema |
| 422 | `InvalidSchema` | Schema registration failed validation |
| 500 | `Internal` | Unexpected server error |
