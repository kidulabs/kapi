### Requirement: Reconciler trait with context injection
The system SHALL provide a `Reconciler` trait that users implement to define reconciliation logic. The trait SHALL receive a `ReconcileContext` containing the request, client, and extensible capabilities.

#### Scenario: User implements Reconciler
- **WHEN** user implements the `Reconciler` trait
- **THEN** the `reconcile` method SHALL receive a `ReconcileContext` parameter containing `request`, `client`, and future extensible fields

#### Scenario: Reconciler returns result
- **WHEN** reconciler completes successfully
- **THEN** it SHALL return a `ReconcileResult` with `requeue_after: Option<Duration>`

### Requirement: ReconcileContext provides client and request
The system SHALL provide a `ReconcileContext` struct that contains the `ReconcileRequest` (key, name, namespace) and a `KapiClient` instance for API operations.

#### Scenario: Reconciler reads object
- **WHEN** reconciler needs to read the current state of an object
- **THEN** it SHALL use `ctx.client.get(&ctx.request.key, ctx.request.namespace.as_deref(), &ctx.request.name)` to fetch the object

#### Scenario: Reconciler updates object
- **WHEN** reconciler needs to update an object
- **THEN** it SHALL use `ctx.client.update(...)` to persist changes

### Requirement: Controller ties reconciler, work queue, and watch stream
The system SHALL provide a `Controller` struct that orchestrates a reconciler, work queue, and watch stream for a single resource kind.

#### Scenario: Controller processes watch events
- **WHEN** a watch event arrives for the watched kind
- **THEN** the controller SHALL extract the object key and name, and enqueue it in the work queue

#### Scenario: Controller invokes reconciler
- **WHEN** the work queue yields a key
- **THEN** the controller SHALL invoke the reconciler's `reconcile` method with a `ReconcileContext` containing the request and client

### Requirement: Controller reconnects on watch stream termination
The system SHALL reconnect the watch stream when it terminates (server closes connection, network error). On reconnect, the controller SHALL list all objects of the watched kind and enqueue every key to catch missed events.

#### Scenario: Watch stream terminates
- **WHEN** the watch stream returns `None` (server closed connection)
- **THEN** the controller SHALL log a warning and reconnect the watch stream
- **THEN** the controller SHALL list all objects of the watched kind (respecting namespace scope and watch filter)
- **THEN** the controller SHALL enqueue every object key from the list into the work queue

#### Scenario: Watch stream error
- **WHEN** the watch stream returns an error
- **THEN** the controller SHALL log the error and reconnect the watch stream after a brief backoff
- **THEN** the controller SHALL list all objects and enqueue every key (same as stream termination)

### Requirement: Controller filters StatusModified events
The system SHALL filter out `WatchEventType::StatusModified` events by default to prevent infinite reconcile loops when reconcilers update object status.

#### Scenario: StatusModified event arrives
- **WHEN** a watch event with `event_type == StatusModified` arrives
- **THEN** the controller SHALL NOT enqueue the object key for reconciliation
- **THEN** the controller SHALL silently ignore the event

#### Scenario: Other event types arrive
- **WHEN** a watch event with `event_type` of `Added`, `Modified`, or `Deleted` arrives
- **THEN** the controller SHALL enqueue the object key for reconciliation as normal

### Requirement: Controller supports namespace scope
The system SHALL allow configuring a controller to watch objects in a specific namespace or all namespaces.

#### Scenario: Namespace-scoped controller
- **WHEN** a controller is configured with `.namespace("production")`
- **THEN** the controller SHALL only watch objects in the "production" namespace
- **THEN** the controller SHALL only enqueue keys for objects in the "production" namespace

#### Scenario: Cluster-scoped controller (all namespaces)
- **WHEN** a controller is configured without a namespace (or with `.all_namespaces()`)
- **THEN** the controller SHALL watch objects across all namespaces
- **THEN** the controller SHALL enqueue keys for objects in any namespace

### Requirement: Controller supports watch filter
The system SHALL allow configuring a controller with a `WatchFilter` (label selector, field selector) to filter which events trigger reconciliation.

#### Scenario: Label selector filter
- **WHEN** a controller is configured with `.watch_filter(WatchFilter::LabelSelector(...))`
- **THEN** the controller SHALL only receive watch events for objects matching the label selector
- **THEN** the controller SHALL only enqueue keys for objects matching the filter

#### Scenario: No filter (default)
- **WHEN** a controller is configured without a watch filter
- **THEN** the controller SHALL use `WatchFilter::All` and receive all events

### Requirement: Controller supports optional shutdown signal
The system SHALL allow a controller to accept an optional shutdown signal. When the signal is received, the controller SHALL stop processing new work and exit gracefully.

#### Scenario: Shutdown signal received
- **WHEN** a controller has a shutdown signal configured and the signal is triggered
- **THEN** the controller SHALL stop reading from the watch stream
- **THEN** the controller SHALL stop processing new items from the work queue
- **THEN** the controller SHALL wait for any in-flight reconcile to complete
- **THEN** the controller SHALL exit gracefully

#### Scenario: No shutdown signal (standalone mode)
- **WHEN** a controller is started without a shutdown signal
- **THEN** the controller SHALL run indefinitely until the process exits or an error occurs

### Requirement: Work queue deduplicates events by key
The system SHALL provide a work queue that deduplicates events by key, ensuring that multiple events for the same object result in a single reconcile call.

#### Scenario: Multiple events for same object
- **WHEN** 50 watch events arrive for object "foo" within a short time window
- **THEN** the work queue SHALL deduplicate them and invoke reconcile exactly once for "foo"

#### Scenario: Events for different objects
- **WHEN** watch events arrive for objects "foo" and "bar"
- **THEN** the work queue SHALL invoke reconcile once for "foo" and once for "bar"

### Requirement: Work queue applies exponential backoff on errors
The system SHALL apply exponential backoff when a reconcile returns an error, retrying indefinitely until success. The backoff sequence SHALL be 1s, 2s, 4s, 8s, 16s, 32s, 64s, 128s, 256s, 300s (5min max). Every error SHALL be logged with context: object key, error message, and retry count.

#### Scenario: Reconcile returns error
- **WHEN** a reconcile call returns an error
- **THEN** the work queue SHALL log the error with object key, error message, and retry count
- **THEN** the work queue SHALL requeue the key with exponential backoff (1s, 2s, 4s, ... up to 5min max)
- **THEN** the work queue SHALL retry indefinitely until the reconcile succeeds

#### Scenario: Reconcile succeeds after error
- **WHEN** a reconcile call succeeds after previous errors
- **THEN** the work queue SHALL reset the backoff counter for that key to zero
- **THEN** the work queue SHALL reset the retry count for that key to zero

#### Scenario: Max backoff reached
- **WHEN** the backoff delay reaches the maximum of 5 minutes
- **THEN** the work queue SHALL continue retrying every 5 minutes indefinitely
- **THEN** the work queue SHALL continue logging errors with increasing retry counts

### Requirement: Work queue supports requeue_after
The system SHALL support `ReconcileResult { requeue_after: Some(duration) }` to schedule a reconcile after a specific delay.

#### Scenario: Reconciler requests requeue
- **WHEN** reconciler returns `ReconcileResult { requeue_after: Some(Duration::from_secs(30)) }`
- **THEN** the work queue SHALL requeue the key after 30 seconds

#### Scenario: Reconciler requests immediate requeue
- **WHEN** reconciler returns `ReconcileResult { requeue_after: Some(Duration::ZERO) }`
- **THEN** the work queue SHALL requeue the key immediately

**Note:** A `requeue_after` of `Some(Duration::ZERO)` indicates immediate requeue; `None` indicates no requeue.

### Requirement: Work queue rate limits processing
The system SHALL rate-limit work queue processing to provide predictable load on the API server.

#### Scenario: High event rate
- **WHEN** many events arrive in a short time
- **THEN** the work queue SHALL process them at a controlled rate (e.g., max 10 reconciles per second)

**Note:** Rate limiting is deferred to future work per design decision.

### Requirement: Finalizer helper is_deleting
The system SHALL provide a standalone function `is_deleting(obj: &StoredObject) -> bool` that returns true if the object has a `deletion_timestamp` set.

#### Scenario: Object is being deleted
- **WHEN** an object has `system.deletion_timestamp` set
- **THEN** `is_deleting(&obj)` SHALL return `true`

#### Scenario: Object is not being deleted
- **WHEN** an object has `system.deletion_timestamp` as `None`
- **THEN** `is_deleting(&obj)` SHALL return `false`

### Requirement: Finalizer helper ensure_finalizer
The system SHALL provide a standalone function `ensure_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` that adds a finalizer to an object if not already present. The function SHALL implement compare-and-swap retry logic to handle concurrent modifications.

#### Scenario: Finalizer not present
- **WHEN** an object does not have the specified finalizer
- **THEN** `ensure_finalizer` SHALL add the finalizer to `obj.metadata.finalizers` and update the object via the client
- **THEN** if the update fails with a conflict (409), the function SHALL re-fetch the object and retry

#### Scenario: Finalizer already present
- **WHEN** an object already has the specified finalizer
- **THEN** `ensure_finalizer` SHALL be a no-op (no update call)

### Requirement: Finalizer helper remove_finalizer
The system SHALL provide a standalone function `remove_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` that removes a finalizer from an object. The function SHALL implement compare-and-swap retry logic to handle concurrent modifications.

#### Scenario: Finalizer present
- **WHEN** an object has the specified finalizer
- **THEN** `remove_finalizer` SHALL remove the finalizer from `obj.metadata.finalizers` and update the object via the client
- **THEN** if the update fails with a conflict (409), the function SHALL re-fetch the object and retry

#### Scenario: Finalizer not present
- **WHEN** an object does not have the specified finalizer
- **THEN** `remove_finalizer` SHALL be a no-op (no update call)
