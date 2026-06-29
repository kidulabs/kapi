## Context

The schema-scope-storage-foundation change provides namespace-aware storage, scope-based routing, and the ability to store cluster-scoped resources. This change builds on that foundation to make Namespace a first-class resource with its own lifecycle.

Namespace objects are cluster-scoped (like Schema) and flow through the same ObjectService as other resources. The key difference is bootstrap (auto-creating "default" at startup) and lifecycle rules (undeletable default, existence validation, deletion blocking).

## Goals / Non-Goals

**Goals:**
- Register Namespace as a built-in core type (kind: "Namespace", group: "kapi.io", version: "v1", scope: "Cluster")
- Auto-create "default" namespace at server startup
- Make "default" namespace undeletable (403 Forbidden)
- Validate namespace existence on object creation (reject if namespace doesn't exist)
- Block namespace deletion if namespace contains objects (409 Conflict)
- Add WatchFilter::Namespace variant for namespace-scoped watch

**Non-Goals:**
- Cascade deletion with finalizers (Phase 2)
- Namespace quotas or resource limits
- Namespace-scoped RBAC
- Namespace metadata beyond name, labels, annotations

## Decisions

### 1. Namespace registration: built-in schema vs hardcoded special-case

**Decision:** Register Namespace schema at startup via the normal Schema registration flow. Namespace is a cluster-scoped resource that flows through ObjectService like any other resource.

**Alternatives considered:**
- Hardcoded like current Schema: Rejected because it doesn't follow the "everything is an object" principle and creates special-case code paths.
- Separate NamespaceService: Rejected because Namespace CRUD is simple enough to use ObjectService directly.

**Rationale:** Namespace is just a cluster-scoped object with some lifecycle rules. Using ObjectService keeps the architecture consistent.

### 2. Bootstrap: when and how to create "default" namespace

**Decision:** At server startup, after the store is initialized, check if "default" namespace exists. If not, create it. This happens before the server starts accepting requests.

**Alternatives considered:**
- Lazy creation on first use: Rejected because it creates implicit behavior and race conditions.
- Require manual creation: Rejected because "default" is the implicit fallback for namespace-less creates.

**Rationale:** Explicit bootstrap ensures "default" always exists, preventing confusing errors when creating objects without explicit namespace.

### 3. "default" namespace protection: undeletable

**Decision:** DELETE /apis/kapi.io/v1/namespaces/default returns 403 Forbidden. This is a simple check in the delete path.

**Alternatives considered:**
- Allow deletion but auto-recreate: Rejected because it's implicit magic.
- Allow deletion: Rejected because it breaks the "default" fallback contract.

**Rationale:** "default" is special because it's the implicit fallback. Making it undeletable is a small special case that prevents confusing errors.

### 4. Namespace existence validation: when to check

**Decision:** Check namespace existence on object CREATE only. If namespace doesn't exist, return 404. Don't check on UPDATE (object already exists in a namespace) or DELETE (namespace might be deleted while objects exist in Phase 1).

**Alternatives considered:**
- Check on every operation: Rejected because it adds unnecessary overhead for UPDATE/DELETE.
- Don't check at all (Kubernetes behavior): Rejected because it allows orphaned objects in non-existent namespaces.

**Rationale:** CREATE-time validation ensures objects only exist in known namespaces without adding overhead to every operation.

### 5. Namespace deletion: block if non-empty

**Decision:** When deleting a namespace, check if any objects exist in that namespace. If yes, return 409 Conflict. User must delete all objects first.

**Alternatives considered:**
- Cascade hard-delete: Rejected because it bypasses finalizers.
- Cascade with finalizers (Phase 2): Deferred to future work.

**Rationale:** Blocking is simple and safe. It forces explicit cleanup, which respects finalizers. Phase 2 can add cascade later.

### 6. WatchFilter::Namespace: implementation approach

**Decision:** Add `WatchFilter::Namespace(String)` variant. The `matches()` method checks `event.object.metadata.namespace == Some(namespace)`. This is Option A from our exploration — namespace is a filter variant, not a separate subscription dimension.

**Alternatives considered:**
- Namespace in subscription key: Rejected because it doubles publish lookups.
- Separate WatchFilter::AllNamespaces: Rejected because WatchFilter::All already means "all namespaces".

**Rationale:** WatchFilter::Namespace is composable with And(), doesn't change EventBus structure, and matches the existing filter pattern.

### 7. Namespace schema: minimal vs extensible

**Decision:** Namespace schema is minimal: `{ "type": "object", "properties": {} }`. Namespace objects have name, labels, annotations (from ObjectMeta) but no spec data. This keeps it simple while allowing future extension.

**Alternatives considered:**
- Rich schema with namespace metadata: Rejected because it's premature — we don't know what metadata is needed yet.
- No schema (hardcoded): Rejected because it breaks the "everything validated by schema" principle.

**Rationale:** Minimal schema allows Namespace to flow through the normal validation path while leaving room for future extension.

## Risks / Trade-offs

**[Bootstrap timing] "default" namespace creation at startup** → If store initialization fails, "default" won't exist. Mitigation: startup fails fast if bootstrap fails.

**[Performance] Namespace existence check on every create** → Extra store lookup per create. Mitigation: namespace lookup is fast (cached or simple query).

**[Usability] Block namespace deletion if non-empty** → User must manually delete all objects first. Mitigation: clear error message listing object count. Phase 2 can add cascade.

**[Compatibility] WatchFilter::Namespace is a new variant** → Existing code that matches on WatchFilter must handle the new variant. Mitigation: update all match statements.

**[Testing] Namespace lifecycle testing** → Need to test bootstrap, deletion blocking, existence validation. Mitigation: comprehensive integration and e2e tests.
