## ADDED Requirements

### Requirement: Typed `ensure_finalizer` helper

The `kapi-controller` crate SHALL provide a generic async function
`ensure_finalizer<T: TypedResource>(client: &KapiClient, obj: &T, finalizer: &str)`
in `kapi_controller::finalizer`. The function SHALL convert `obj` to a
`StoredObject` via `TypedResource::to_stored_object` and then delegate to
the existing CAS-retry `ensure_finalizer` logic. The CAS-retry behaviour
SHALL be identical to the raw `&StoredObject` variant: idempotent add,
re-fetch on 409 Conflict, up to 5 attempts, 10 ms backoff between retries.
The raw `&StoredObject` helper SHALL remain available unchanged.

#### Scenario: Finalizer already present on typed resource

- **WHEN** a typed resource's metadata already contains the requested
  finalizer name and a caller invokes the typed `ensure_finalizer`
- **THEN** the helper SHALL return `Ok(())` without calling
  `client.update`

#### Scenario: Finalizer absent on typed resource

- **WHEN** a typed resource's metadata does not contain the requested
  finalizer name and a caller invokes the typed `ensure_finalizer`
- **THEN** the helper SHALL call `client.update` with the finalizer
  appended to `metadata.finalizers` and return `Ok(())`

#### Scenario: CAS conflict during ensure

- **WHEN** `client.update` returns 409 Conflict while adding a finalizer
- **THEN** the helper SHALL re-fetch the object via `client.get` and
  retry, up to a total of 5 attempts

### Requirement: Typed `remove_finalizer` helper

The `kapi-controller` crate SHALL provide a generic async function
`remove_finalizer<T: TypedResource>(client: &KapiClient, obj: &T, finalizer: &str)`
in `kapi_controller::finalizer`. The function SHALL convert `obj` to a
`StoredObject` via `TypedResource::to_stored_object` and then delegate to
the existing CAS-retry `remove_finalizer` logic. The CAS-retry behaviour
SHALL be identical to the raw `&StoredObject` variant: idempotent remove,
re-fetch on 409 Conflict, up to 5 attempts, 10 ms backoff between retries.
The raw `&StoredObject` helper SHALL remain available unchanged.

#### Scenario: Finalizer absent on typed resource during remove

- **WHEN** a typed resource's metadata does not contain the requested
  finalizer name and a caller invokes the typed `remove_finalizer`
- **THEN** the helper SHALL return `Ok(())` without calling
  `client.update`

#### Scenario: Finalizer present on typed resource during remove

- **WHEN** a typed resource's metadata contains the requested finalizer
  name and a caller invokes the typed `remove_finalizer`
- **THEN** the helper SHALL call `client.update` with the finalizer
  removed from `metadata.finalizers` and return `Ok(())`

#### Scenario: CAS conflict during remove

- **WHEN** `client.update` returns 409 Conflict while removing a
  finalizer
- **THEN** the helper SHALL re-fetch the object via `client.get` and
  retry, up to a total of 5 attempts

#### Scenario: Remove finalizer on resource marked for deletion

- **WHEN** a typed resource has `system.deletion_timestamp` set and its
  metadata contains the requested finalizer, and a caller invokes the
  typed `remove_finalizer`
- **THEN** the helper SHALL remove the finalizer via `client.update` and
  return `Ok(())`

### Requirement: Serialization failure propagation

The typed helpers SHALL propagate any error returned by
`TypedResource::to_stored_object` as the function's returned error. The
error SHALL be convertible into `Box<dyn std::error::Error + Send + Sync>`
via the standard `?` operator, matching the return type of the raw
helpers.

#### Scenario: `to_stored_object` serialization failure

- **WHEN** `to_stored_object` fails on a typed resource whose spec or
  status cannot be serialised
- **THEN** the typed `ensure_finalizer` / `remove_finalizer` SHALL
  return `Err` containing the `TypedError::Serialization` variant
  without calling `client.update`

### Requirement: Raw finalizer helpers preserved

The existing `ensure_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str)`
and `remove_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str)`
functions SHALL remain available in `kapi_controller::finalizer` with their
current signatures and behaviour. The addition of typed overloads SHALL
NOT alter the behaviour of calls that pass a `StoredObject` directly.

#### Scenario: Existing raw caller

- **WHEN** a caller passes a `StoredObject` to `ensure_finalizer` or
  `remove_finalizer`
- **THEN** the raw helper SHALL be resolved and behaviour SHALL be
  identical to the pre-change implementation

### Requirement: kapibuild controller generator uses typed helpers

The kapibuild controller generator template (`kapibuild/src/controller_generate.rs`)
SHALL emit reconcile code that calls the typed `ensure_finalizer` /
`remove_finalizer` with the typed resource directly, without materialising
an intermediate `StoredObject` binding.

#### Scenario: Generated controller uses typed helper

- **WHEN** kapibuild generates a controller source file
- **THEN** the generated reconcile function SHALL call
  `finalizer::ensure_finalizer(&ctx.client, resource, FINALIZER_NAME)`
  and `finalizer::remove_finalizer(&ctx.client, resource, FINALIZER_NAME)`
  where `resource` is the typed resource, without a preceding
  `to_stored_object` conversion
