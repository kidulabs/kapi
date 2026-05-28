# kapi — API Reference

## Base URL

All API paths are under `/apis/{group}/{version}/{kind}`.

## Schema Registry

Schemas define the JSON Schema that objects of a given kind must conform to. They are themselves stored as objects of kind `Schema` in group `kapi.io`.

### Register a Schema

Creates a new Schema. The name is auto-generated as `{targetKind}.{targetGroup}`.

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
        "name": "Widget.example.io",
        "labels": {}
    },
    "system": {
        "resourceVersion": 1,
        "createdAt": "2024-01-01T00:00:00Z",
        "updatedAt": "2024-01-01T00:00:00Z"
    },
    "data": {
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

`name` is the auto-generated name, e.g. `Widget.example.io`.

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

```
POST /apis/{group}/{version}/{kind}
```

**Request body:**

```json
{
    "metadata": {
        "name": "my-widget",
        "labels": {
            "app.example.io/name": "my-widget",
            "tier": "frontend"
        }
    },
    "color": "blue",
    "size": 10
}
```

The `metadata.name` field is extracted by the handler. The optional `metadata.labels` field is extracted and validated (see [Label Validation](#label-validation)). All other fields are validated against the registered JSON Schema.

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
    "data": {
        "color": "blue",
        "size": 10
    }
}
```

### List Objects

```
GET /apis/{group}/{version}/{kind}
```

**Query parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `watch` | boolean | Enable SSE watch stream |
| `limit` | integer | Page size for paginated listing |
| `continue` | string | Cursor from previous page (base64-encoded) |

**Response:** `200 OK`

```json
{
    "items": [ /* StoredObject array */ ],
    "continueToken": "aX ... base64 ..."
}
```

### Get an Object

```
GET /apis/{group}/{version}/{kind}/{name}
```

**Response:** `200 OK` — single StoredObject

**Error:** `404`

### Update an Object

Requires the full StoredObject with the correct `system.resourceVersion`.

```
PUT /apis/{group}/{version}/{kind}/{name}
```

**Request body:** Full StoredObject with updated `data.value` and optionally updated `metadata.labels`.

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
    "data": {
        "color": "red",
        "size": 20
    }
}
```

Labels are updated via diff-based strategy: the server reads the existing labels, computes the delta, and applies only the changed key-value pairs in the same transaction as the object update.

**Response:** `200 OK` — updated StoredObject with bumped `system.resourceVersion`

**Errors:** `409` (version conflict), `422` (validation failure), `400` (invalid labels), `404`

### Delete an Object

```
DELETE /apis/{group}/{version}/{kind}/{name}
```

**Response:** `200 OK` — the deleted StoredObject

**Errors:** `404`

---

## Watch (SSE)

Add `?watch=true` to any list request to receive an SSE stream of real-time events.

```
GET /apis/example.io/v1/Widget?watch=true
```

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

**Errors:** `400` for unsupported fields, malformed syntax (missing `=` sign), or `fieldSelector` on a non-watch request.

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
| 404 | `NotFound` | Resource not found |
| 409 | `Conflict` | OCC version mismatch or duplicate |
| 409 | `SchemaHasObjects` | Cannot delete schema with existing objects |
| 400 | `InvalidFieldSelector` | Invalid fieldSelector query parameter (unsupported field, malformed syntax, or fieldSelector on non-watch request) |
| 400 | `InvalidLabel` | Label key or value violates format or length rules |
| 422 | `SchemaValidation` | Object data doesn't match schema |
| 422 | `InvalidSchema` | Schema registration failed validation |
| 500 | `Internal` | Unexpected server error |
