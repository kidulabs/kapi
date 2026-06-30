## Purpose

Define the Namespace resource type — a cluster-scoped core type that provides namespace isolation for objects. Namespaces enable multi-tenancy within a single API server instance by grouping objects into logical partitions. This spec covers registration, lifecycle, and validation of the built-in Namespace resource.

## Requirements

### Requirement: Namespace is a registered core type
The system SHALL register a built-in Namespace schema at server startup with `kind: "Namespace"`, `group: "kapi.io"`, `version: "v1"`, and `scope: "Cluster"`. The schema SHALL have a minimal JSON schema: `{ "type": "object", "properties": {} }`. Namespace objects SHALL flow through ObjectService like any other cluster-scoped resource.

#### Scenario: Namespace schema registered at startup
- **WHEN** the server starts
- **THEN** a Schema object for Namespace SHALL be registered with `kind: "Namespace"`, `group: "kapi.io"`, `version: "v1"`, `scope: "Cluster"`

#### Scenario: Namespace objects are cluster-scoped
- **WHEN** a Namespace object is created
- **THEN** it SHALL be stored with `metadata.namespace = None`

#### Scenario: Namespace CRUD uses cluster-scoped URLs
- **WHEN** Namespace CRUD operations are performed
- **THEN** they SHALL use `/apis/kapi.io/v1/namespaces[/{name}]` (no namespace in URL)

### Requirement: "default" namespace auto-created at startup
The system SHALL create a Namespace object with `name: "default"` at server startup if it does not already exist. This SHALL happen before the server starts accepting requests.

#### Scenario: "default" namespace created on fresh start
- **WHEN** the server starts with an empty store
- **THEN** a Namespace object with `name: "default"` SHALL be created

#### Scenario: "default" namespace not recreated if exists
- **WHEN** the server starts and "default" namespace already exists
- **THEN** no duplicate Namespace object SHALL be created

#### Scenario: Startup fails if bootstrap fails
- **WHEN** the "default" namespace creation fails
- **THEN** the server SHALL fail to start with an error

### Requirement: "default" namespace is undeletable
The system SHALL reject DELETE requests for the "default" namespace with 403 Forbidden.

#### Scenario: Delete "default" namespace rejected
- **WHEN** DELETE `/apis/kapi.io/v1/namespaces/default` is called
- **THEN** the response SHALL be 403 Forbidden with an error message

#### Scenario: Delete other namespaces allowed
- **WHEN** DELETE `/apis/kapi.io/v1/namespaces/production` is called
- **THEN** the deletion SHALL proceed (subject to other validation)

### Requirement: Namespace existence validated on object creation
The system SHALL validate that the target namespace exists before creating an object. If the namespace does not exist, the system SHALL return 404 Not Found.

#### Scenario: Create object in existing namespace
- **WHEN** an object is created in namespace "production" and the namespace exists
- **THEN** the object SHALL be created successfully

#### Scenario: Create object in non-existent namespace
- **WHEN** an object is created in namespace "nonexistent" and the namespace does not exist
- **THEN** the response SHALL be 404 Not Found with an error message

#### Scenario: Create object in "default" namespace
- **WHEN** an object is created without explicit namespace (defaults to "default")
- **THEN** the object SHALL be created successfully (since "default" always exists)

### Requirement: Namespace deletion blocked if non-empty
The system SHALL reject DELETE requests for a namespace that contains objects. The response SHALL be 409 Conflict with an error message indicating the namespace is not empty.

#### Scenario: Delete empty namespace
- **WHEN** DELETE `/apis/kapi.io/v1/namespaces/production` is called and the namespace has no objects
- **THEN** the namespace SHALL be deleted

#### Scenario: Delete non-empty namespace blocked
- **WHEN** DELETE `/apis/kapi.io/v1/namespaces/production` is called and the namespace contains objects
- **THEN** the response SHALL be 409 Conflict with an error message

#### Scenario: Error message includes object count
- **WHEN** DELETE is blocked for a non-empty namespace
- **THEN** the error message SHALL indicate how many objects exist in the namespace

### Requirement: Namespace constants defined
The system SHALL define constants for the Namespace resource: `NAMESPACE_KIND = "Namespace"`, `NAMESPACE_GROUP = "kapi.io"`, `NAMESPACE_VERSION = "v1"`.

#### Scenario: Namespace constants accessible
- **WHEN** code references `NAMESPACE_KIND`
- **THEN** the value SHALL be `"Namespace"`

#### Scenario: Namespace constants used in bootstrap
- **WHEN** the server bootstraps the "default" namespace
- **THEN** it SHALL use the Namespace constants to construct the ResourceKey
