## Why

The system currently treats all resources as globally unique within their kind, with no concept of partitioning or scoping. This limits multi-tenancy support and makes it impossible to have resources with the same name in different contexts. Adding schema scope and namespace-aware storage lays the foundation for proper resource isolation and enables the Namespace resource in a follow-up change.

## What Changes

- **BREAKING**: Storage key changes from `(ResourceKey, name)` to `(ResourceKey, namespace: Option<String>, name)`. Name uniqueness is now scoped to `(namespace, kind)` instead of just `kind`.
- **BREAKING**: Store trait signatures change: `get()`, `list()`, and `transaction()` now accept `namespace: Option<&str>` parameter.
- **BREAKING**: URL structure changes for namespaced resources: `/apis/{g}/{v}/namespaces/{ns}/{kind}[/{name}]`. Cluster-scoped resources use `/apis/{g}/{v}/{kind}[/{name}]`.
- Add `scope` field to Schema definition: `"Namespaced"` (default) or `"Cluster"`.
- Handlers extract namespace from URL path and pass to service layer.
- Service layer validates scope vs URL pattern (cluster-scoped kinds reject namespace in URL, namespaced kinds require it for get/update/delete).
- Schema resource becomes cluster-scoped (scope: "Cluster") instead of hardcoded special-case.
- ObjectMeta gains `namespace: Option<String>` field.
- SQLite schema adds `namespace` column (nullable).
- InMemory store key changes to `(ResourceKey, Option<String>, String)`.
- Cross-namespace list: namespaced kinds at `/apis/{g}/{v}/{kind}` return objects from all namespaces.
- Continue token encodes `(namespace, name)` for cross-namespace pagination.

## Non-goals

- Namespace as a resource (follow-up proposal)
- Namespace lifecycle management (creation, deletion, cascade)
- Namespace existence validation on object creation
- WatchFilter namespace support
- "default" namespace auto-creation

## Capabilities

### New Capabilities
- `schema-scope`: Schema scope field (Namespaced vs Cluster) and scope-aware routing/validation
- `namespace-storage`: Namespace-aware storage key and store trait changes

### Modified Capabilities
- `object-store`: Store trait signatures change to include namespace parameter
- `object-handlers`: Handlers extract namespace from URL, pass to service
- `object-service`: Service validates scope vs URL, interprets namespace
- `core-types`: ObjectMeta gains namespace field
- `schema-registry`: Schema definition includes scope field
- `list-filtering`: Cross-namespace list support, continue token includes namespace

## Impact

- **Storage**: Both InMemoryStore and SQLiteStore require schema/key changes. Existing data migration needed (all objects → "default" namespace, Schema → None).
- **API**: URL structure changes for all namespaced resources. Breaking change for existing clients.
- **Code**: Store trait, handlers, service layer, types, SQLite schema, InMemory store all require updates.
- **Tests**: Integration tests and e2e tests need updates for new URL structure and namespace parameter.
- **Documentation**: AGENTS.md, API docs, OpenAPI spec generation need updates.

## Future Work

- Namespace as a registered core resource (follow-up proposal)
- Namespace lifecycle: creation, deletion, cascade behavior
- WatchFilter::Namespace variant for namespace-scoped watch
- "default" namespace auto-creation and protection
- Namespace existence validation on object creation
