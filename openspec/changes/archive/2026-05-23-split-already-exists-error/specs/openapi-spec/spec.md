## MODIFIED Requirements

### Requirement: Response codes documented for all operations
All dynamic paths SHALL document appropriate HTTP response codes:
- POST: 201 (Created), 404 (NotFound for unregistered kind), 409 (Conflict for version mismatch), 409 (AlreadyExists for duplicate), 422 (SchemaValidation)
- GET (item): 200 (OK), 404 (NotFound)
- PUT: 200 (OK), 404 (NotFound), 409 (Conflict for version mismatch), 422 (SchemaValidation)
- DELETE: 200 (OK), 404 (NotFound), 409 (SchemaHasObjects for Schema deletion)
- GET (list): 200 (OK)

#### Scenario: POST documents error responses
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the POST operation documents 404, 409 (Conflict), 409 (AlreadyExists), and 422 response schemas referencing `AppError`

#### Scenario: POST documents AlreadyExists response
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the POST operation includes a 409 response with `code: "AlreadyExists"` and `details` containing `kind` and `name` fields
