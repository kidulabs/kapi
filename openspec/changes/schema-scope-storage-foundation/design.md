## Context

The kapi system currently stores objects with a key of `(ResourceKey, name)`, where `ResourceKey` is `(group, version, kind)`. All objects are globally unique within their kind. The Schema type is hardcoded as a special case with its own service and constants.

This design adds namespace-aware storage and schema scope to lay the foundation for proper resource isolation. The Namespace resource itself is out of scope for this change (follow-up proposal).

## Goals / Non-Goals

**Goals:**
- Add `scope` field to Schema: "Namespaced" (default) or "Cluster"
- Change storage key to include namespace: `(ResourceKey, namespace: Option<String>, name)`
- Update store trait to accept namespace parameter
- Update URL structure: namespaced resources use `/namespaces/{ns}/{kind}`, cluster-scoped use `/{kind}`
- Handlers extract namespace from URL, service validates scope vs URL
- Migrate Schema to be cluster-scoped (remove hardcoded special-case)
- Support cross-namespace list for namespaced kinds

**Non-Goals:**
- Namespace as a resource (follow-up proposal)
- Namespace lifecycle (creation, deletion, cascade)
- Namespace existence validation on object creation
- WatchFilter namespace support
- "default" namespace auto-creation

## Decisions

### 1. Namespace representation: `Option<String>` vs sentinel value

**Decision:** Use `Option<String>` for namespace field. `None` = cluster-scoped, `Some("ns")` = namespaced.

**Alternatives considered:**
- Empty string `""` for cluster-scoped: Rejected because it's a magic value, confusing in SQL (empty string vs NULL), and looks odd in etcd keys (double slash).
- Sentinel value `"_cluster"`: Rejected because it violates "storage is dumb" principle. Sentinel is a convention that must be enforced somewhere, leaking storage details into service layer or requiring store to understand scope.

**Rationale:** `Option<String>` is type-safe, idiomatic Rust, maps cleanly to SQL NULL, and keeps the store dumb (no translation needed).

### 2. URL structure: path segment vs query parameter

**Decision:** Kubernetes-style path segments: `/apis/{g}/{v}/namespaces/{ns}/{kind}[/{name}]` for namespaced resources, `/apis/{g}/{v}/{kind}[/{name}]` for cluster-scoped.

**Alternatives considered:**
- Query parameter `?namespace=ns`: Rejected because it's less RESTful, easy to forget, and doesn't convey hierarchy.
- Hybrid (both supported): Rejected because two ways to do the same thing creates confusion.

**Rationale:** Path segments are more RESTful, familiar to Kubernetes users, and clearly convey the namespace hierarchy.

### 3. Handler scope awareness: lookup in handler vs service

**Decision:** Handlers remain pure extraction. They extract namespace from URL and pass to service. Service looks up schema scope and validates.

**Alternatives considered:**
- Handler looks up scope: Rejected because it changes handler role from "pure translation" to "translation + scope-aware routing".
- Middleware extracts scope: Rejected because it just moves the problem, doesn't solve it.
- Dynamic route registration based on scope: Rejected because it's complex and breaks static route model.

**Rationale:** Keeping handlers pure maintains separation of concerns. Service already owns business logic and has schema access, so scope validation fits naturally there.

### 4. CREATE namespace resolution: URL precedence vs metadata

**Decision:** URL namespace takes precedence. If URL has namespace, use it (discard metadata.namespace). If URL has no namespace, default to "default". Cluster-scoped kinds reject namespace in URL.

**Alternatives considered:**
- Strict (reject if metadata.namespace conflicts): Rejected because it's less lenient and breaks the "URL is where the operation happens" principle.
- Use metadata.namespace if URL is empty: Rejected because it's inconsistent with the URL-first approach.

**Rationale:** URL-first is consistent, predictable, and matches Kubernetes behavior. Lenient approach (discard metadata) is simpler than strict (validate match).

### 5. UPDATE/DELETE namespace handling: strict vs lenient

**Decision:** UPDATE is strict: metadata.namespace must match URL namespace or be absent. DELETE requires namespace in URL (no payload to check).

**Rationale:** UPDATE modifies existing objects that live in a specific namespace. Allowing namespace mismatch would imply moving objects, which is a different operation. DELETE has no payload, so URL is the only source.

### 6. Cross-namespace list: implicit vs explicit

**Decision:** Namespaced kinds at `/apis/{g}/{v}/{kind}` (no namespace in URL) return objects from all namespaces. This is implicit cross-namespace access.

**Alternatives considered:**
- Require explicit flag `?allNamespaces=true`: Rejected because it's more verbose and Kubernetes doesn't require it.
- Separate endpoint `/apis/{g}/{v}/all-namespaces/{kind}`: Rejected because it's non-standard.

**Rationale:** Implicit cross-namespace list matches Kubernetes behavior and is simpler.

### 7. Continue token: name only vs (namespace, name)

**Decision:** Continue token encodes `(namespace, name)` for cross-namespace pagination.

**Rationale:** Cross-namespace list returns objects from multiple namespaces, so pagination must track both namespace and name to resume correctly.

### 8. Schema scope field location: in SchemaData vs separate structure

**Decision:** Add `scope` field to SchemaData (alongside kind, group, version, schema).

**Alternatives considered:**
- Separate "resource definition" structure: Rejected because it's more complexity without much benefit.

**Rationale:** Schema already defines what a kind IS. Scope is part of that definition.

## Risks / Trade-offs

**[Breaking change] Storage key change** → All existing data must be migrated. In-memory store: trivial (restart). SQLite: requires schema migration (add namespace column, update existing rows). Mitigation: we're in active development, can break contracts.

**[Breaking change] URL structure change** → All existing API clients must update URLs. Mitigation: document clearly, provide migration guide.

**[Complexity] Dual URL patterns** → Same path pattern `/apis/{g}/{v}/{kind}` means different things based on scope (cluster-scoped list vs cross-namespace list). Mitigation: service validates and returns clear errors.

**[Performance] Scope lookup on every request** → Service must look up schema scope for every request. Mitigation: schema registry caches compiled schemas, so lookup is fast.

**[Testing] Test coverage** → Many existing tests assume old URL structure and no namespace. Mitigation: update tests systematically, add new tests for namespace scenarios.
