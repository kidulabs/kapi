## MODIFIED Requirements

### Requirement: get_validator returns cached or lazily compiled validator
The `get_validator(key: &ResourceKey)` method SHALL:
1. Compute the cache key as `"{kind}.{group}"` from the provided `ResourceKey`
2. Check the cache for an existing validator
3. On cache hit, return the cached validator
4. On cache miss, fetch the Schema from the store using `schema_key()` and name `"{kind}.{group}"`
5. Parse the fetched Schema's data into `SchemaData`
6. Compile `schema_data.json_schema`
7. Insert the compiled validator into the cache
8. Return the compiled validator

#### Scenario: Cache hit returns cached validator
- **WHEN** `get_validator` is called for a key whose validator exists in the cache
- **THEN** the cached validator is returned without any store access

#### Scenario: Cache miss compiles and caches validator
- **WHEN** `get_validator` is called for a key whose validator is not in the cache but whose Schema exists in the store
- **THEN** the Schema is fetched from the store, compiled, inserted into the cache, and the compiled validator is returned

#### Scenario: Cache miss with no Schema in store returns NotFound
- **WHEN** `get_validator` is called for a key whose Schema does not exist in the store
- **THEN** the method returns `Err(AppError::NotFound { what: "schema", identifier: schema_name })`

#### Scenario: Cache miss with uncompilable stored schema returns StoredSchemaCompilationFailed
- **WHEN** `get_validator` is called for a key whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** the method returns `Err(AppError::StoredSchemaCompilationFailed { schema_name, reason })`
