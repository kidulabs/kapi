## 1. CLI Structure

- [ ] 1.1 Add `api create` subcommand to CLI with required flags (group, version, kind)
- [ ] 1.2 Add optional flags (scope, status)

## 2. Skeleton Generation

- [ ] 2.1 Implement api/<group>/<version>/<kind>.rs file creation
- [ ] 2.2 Generate WidgetSpec skeleton struct with correct derives and kapi attributes
- [ ] 2.3 Generate WidgetStatus skeleton struct when --status flag is provided
- [ ] 2.4 Add example fields to skeleton structs

## 3. Kapifile Update

- [ ] 3.1 Implement Kapifile parsing and update logic
- [ ] 3.2 Add resource entry to Kapifile with kind, version, scope, has_status

## 4. Validation

- [ ] 4.1 Add validation to prevent creating API for existing kind
- [ ] 4.2 Validate scope is "Namespaced" or "Cluster"

## 5. Testing

- [ ] 5.1 Test `kapibuild api create` without status flag
- [ ] 5.2 Test `kapibuild api create` with status flag
- [ ] 5.3 Test duplicate API creation returns error
- [ ] 5.4 Run `cargo clippy -p kapibuild` to check for linting issues
