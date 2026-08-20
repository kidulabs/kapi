## MODIFIED Requirements

### Requirement: Finalizer helper ensure_finalizer
The system SHALL provide a standalone function `ensure_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` that adds a finalizer to an object if not already present. The function SHALL implement compare-and-swap retry logic to handle concurrent modifications. The retry SHALL match on `ApiError::Conflict` instead of raw HTTP status code 409.

#### Scenario: Finalizer not present
- **WHEN** an object does not have the specified finalizer
- **THEN** `ensure_finalizer` SHALL add the finalizer to `obj.metadata.finalizers` and update the object via the client
- **THEN** if the update fails with `ClientError::Api(ApiError::Conflict { .. })`, the function SHALL re-fetch the object and retry

#### Scenario: Finalizer already present
- **WHEN** an object already has the specified finalizer
- **THEN** `ensure_finalizer` SHALL be a no-op (no update call)

#### Scenario: Non-conflict error is not retried
- **WHEN** the update fails with `ClientError::Api(ApiError::ObjectBeingDeleted { .. })`
- **THEN** the function SHALL NOT retry
- **THEN** the function SHALL return the error immediately

#### Scenario: Other 409 errors are not retried
- **WHEN** the update fails with `ClientError::Api(ApiError::NamespaceNotEmpty { .. })` or other non-Conflict 409 errors
- **THEN** the function SHALL NOT retry
- **THEN** the function SHALL return the error immediately

### Requirement: Finalizer helper remove_finalizer
The system SHALL provide a standalone function `remove_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` that removes a finalizer from an object. The function SHALL implement compare-and-swap retry logic to handle concurrent modifications. The retry SHALL match on `ApiError::Conflict` instead of raw HTTP status code 409.

#### Scenario: Finalizer present
- **WHEN** an object has the specified finalizer
- **THEN** `remove_finalizer` SHALL remove the finalizer from `obj.metadata.finalizers` and update the object via the client
- **THEN** if the update fails with `ClientError::Api(ApiError::Conflict { .. })`, the function SHALL re-fetch the object and retry

#### Scenario: Finalizer not present
- **WHEN** an object does not have the specified finalizer
- **THEN** `remove_finalizer` SHALL be a no-op (no update call)

#### Scenario: Non-conflict error is not retried
- **WHEN** the update fails with `ClientError::Api(ApiError::ObjectBeingDeleted { .. })`
- **THEN** the function SHALL NOT retry
- **THEN** the function SHALL return the error immediately

### Requirement: Controller matches on ApiError variants for control flow
The controller SHALL match on `ApiError` variants for control flow decisions instead of raw HTTP status codes. This provides semantic error handling and compile-time safety.

#### Scenario: Object not found during reconcile
- **WHEN** `client.get()` returns `Err(ClientError::Api(ApiError::NotFound { .. }))`
- **THEN** the controller SHALL log a warning and mark the item as done (no retry)
- **THEN** the controller SHALL NOT requeue the item

#### Scenario: Other errors during reconcile
- **WHEN** `client.get()` returns any other error
- **THEN** the controller SHALL log a warning with the error
- **THEN** the controller SHALL mark the item as failed (triggers workqueue backoff retry)

#### Scenario: CAS conflict during finalizer update
- **WHEN** `client.update()` returns `Err(ClientError::Api(ApiError::Conflict { .. }))`
- **THEN** the finalizer helper SHALL re-fetch the object and retry (up to max attempts)

#### Scenario: Object being deleted during finalizer update
- **WHEN** `client.update()` returns `Err(ClientError::Api(ApiError::ObjectBeingDeleted { .. }))`
- **THEN** the finalizer helper SHALL NOT retry
- **THEN** the finalizer helper SHALL return the error immediately
