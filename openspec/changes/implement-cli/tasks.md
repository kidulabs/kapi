## 1. Setup and Dependencies

- [x] 1.1 Add dependencies to `kapi-client/Cargo.toml`: reqwest (with json, stream features), tokio, serde, serde_json, thiserror, futures-util, eventsource-stream, async-trait
- [x] 1.2 Add dependencies to `kapi-cli/Cargo.toml`: clap (with derive feature), serde_yaml, tokio, anyhow, thiserror, clap_complete
- [x] 1.3 Verify workspace structure: `kapi-core` ← `kapi-client` ← `kapi-cli` dependency chain is correct
- [x] 1.4 Run `cargo check` to verify dependencies resolve correctly

## 2. kapi-client: Core Types and Error Handling

- [x] 2.1 Create `kapi-client/src/error.rs` with `ClientError` enum using thiserror: `HttpError`, `ApiError`, `SerializationError`, `StreamError`. Include HTTP status code and error message from server response
- [x] 2.2 Re-export all types from `kapi-core` in `kapi-client/src/lib.rs`: `StoredObject`, `ResourceKey`, `WatchEvent`, `ListResponse`, `ListOptions`, `ObjectMeta`, `WatchFilter`, `FieldSelector`, `LabelSelector`, `SystemMetadata`, `SchemaData`
- [x] 2.3 Add comments explaining error types and when each variant is returned
- [x] 2.4 Run `cargo check -p kapi-client` to verify compilation

## 3. kapi-client: HTTP Client Implementation

- [x] 3.1 Create `kapi-client/src/client.rs` with `KapiClient` struct containing reqwest::Client and base_url: String
- [x] 3.2 Implement `KapiClient::new(base_url: &str) -> Result<Self, ClientError>` constructor that initializes reqwest client
- [x] 3.3 Implement URL construction helper: `build_url(key: &ResourceKey, namespace: Option<&str>, path_suffix: &str) -> String` that constructs `/apis/{group}/{version}/{kind}` or `/apis/{group}/{version}/namespaces/{ns}/{kind}` based on namespace parameter
- [x] 3.4 Add comments explaining route-agnostic design: client accepts `namespace: Option<&str>`, constructs URL accordingly, does not validate scope
- [x] 3.5 Run `cargo check -p kapi-client` to verify compilation

## 4. kapi-client: CRUD Methods

- [x] 4.1 Implement `list(key: ResourceKey, namespace: Option<&str>, opts: ListOptions) -> Result<ListResponse, ClientError>`: sends GET request with query parameters (limit, continue, fieldSelector, labelSelector), parses JSON response
- [x] 4.2 Implement `get(key: ResourceKey, namespace: Option<&str>, name: &str) -> Result<StoredObject, ClientError>`: sends GET request, parses JSON response, handles 404 as error
- [x] 4.3 Implement `create(key: ResourceKey, namespace: Option<&str>, meta: ObjectMeta, spec: Value) -> Result<StoredObject, ClientError>`: sends POST request with JSON body containing metadata and spec, parses response
- [x] 4.4 Implement `update(namespace: Option<&str>, obj: StoredObject) -> Result<StoredObject, ClientError>`: sends PUT request with full StoredObject JSON, validates key/name match URL, handles 409 conflict
- [x] 4.5 Implement `delete(key: ResourceKey, namespace: Option<&str>, name: &str) -> Result<StoredObject, ClientError>`: sends DELETE request, parses response
- [x] 4.6 Add comments explaining each method's HTTP verb, URL construction, and error handling
- [x] 4.7 Run `cargo check -p kapi-client` to verify compilation

## 5. kapi-client: Status Subresource Methods

- [x] 5.1 Implement `get_status(key: ResourceKey, namespace: Option<&str>, name: &str) -> Result<Option<Value>, ClientError>`: sends GET request to `/status` endpoint, parses JSON response
- [x] 5.2 Implement `update_status(key: ResourceKey, namespace: Option<&str>, name: &str, status: Value) -> Result<StoredObject, ClientError>`: sends PUT request to `/status` endpoint with JSON body containing status field, parses response
- [x] 5.3 Add comments explaining status subresource endpoints
- [x] 5.4 Run `cargo check -p kapi-client` to verify compilation

## 6. kapi-client: Watch and Schema Methods

- [x] 6.1 Implement `watch(key: ResourceKey, filter: WatchFilter) -> Result<impl Stream<Item = WatchEvent>, ClientError>`: sends GET request with `?watch=true` and filter query parameters, parses SSE stream using eventsource-stream, yields WatchEvent objects
- [x] 6.2 Implement `list_schemas() -> Result<Vec<StoredObject>, ClientError>`: sends GET request to `/apis/kapi.io/v1/Schema`, returns list of schema objects
- [x] 6.3 Add comments explaining SSE parsing and schema resolution use case
- [x] 6.4 Run `cargo check -p kapi-client` to verify compilation

## 7. kapi-client: Testing

- [x] 7.1 Add unit tests for URL construction helper (cluster-scoped vs namespace-scoped)
- [x] 7.2 Add unit tests for error parsing (HTTP errors, API errors)
- [x] 7.3 Run `cargo test -p kapi-client` to verify all tests pass
- [x] 7.4 Run `cargo clippy -p kapi-client` and fix any warnings

## 8. kapi-cli: Configuration and Setup

- [x] 8.1 Create `kapi-cli/src/config.rs` with `Config` struct containing `server: String`
- [x] 8.2 Implement config loading: read from `~/.kapi/config.yaml` (or `KAPI_CONFIG` env var), parse YAML, fallback to default `http://localhost:8080`
- [x] 8.3 Add comments explaining config precedence: flag > env > config > default
- [x] 8.4 Create `kapi-cli/src/main.rs` with clap derive structure: `Cli` struct with subcommands enum
- [x] 8.5 Define subcommands: `Get`, `Apply`, `Delete`, `Watch`, `Status` (with `Get` and `Apply` sub-subcommands), `Completions`
- [x] 8.6 Add global flags: `--namespace/-n`, `--output/-o` (with values: table, json, yaml)
- [x] 8.7 Run `cargo check -p kapi-cli` to verify compilation

## 9. kapi-cli: Schema Resolution

- [x] 9.1 Create `kapi-cli/src/resolver.rs` with `SchemaResolver` struct
- [x] 9.2 Implement `resolve_kind(client: &KapiClient, kind: &str) -> Result<ResourceKey, CliError>`: queries `list_schemas()`, searches for matching `targetKind`, returns full `ResourceKey` with group/version/kind
- [x] 9.3 Implement ambiguity detection: if multiple schemas have same kind, return error with list of matches
- [x] 9.4 Implement `group/kind` syntax parsing: if input contains `/`, split and match on group and kind
- [x] 9.5 Add special error for schema not found: "No schema found for kind '{kind}'. Use 'kapi get Schema' to list available kinds"
- [x] 9.6 Add comments explaining short name resolution flow and caching strategy (in-memory per command)
- [x] 9.7 Run `cargo check -p kapi-cli` to verify compilation

## 10. kapi-cli: Output Formatting

- [x] 10.1 Create `kapi-cli/src/output.rs` with output formatter functions
- [x] 10.2 Implement `format_table(obj: &StoredObject, scope: &str) -> String`: formats single object as table row with NAME, [NAMESPACE], AGE columns
- [x] 10.3 Implement `format_table_list(items: &[StoredObject], scope: &str) -> String`: formats list as table with headers
- [x] 10.4 Implement `format_json(obj: &StoredObject) -> String`: formats as pretty-printed JSON
- [x] 10.5 Implement `format_yaml(obj: &StoredObject) -> String`: converts to YAML using serde_yaml
- [x] 10.6 Implement AGE calculation: compute relative time from `system.createdAt` (e.g., "2m", "1h", "3d")
- [x] 10.7 Implement watch event table format: EVENT_TYPE NAME [NAMESPACE] AGE
- [x] 10.8 Add comments explaining resource-specific column selection based on scope
- [x] 10.9 Run `cargo check -p kapi-cli` to verify compilation

## 11. kapi-cli: Command Implementations

- [x] 11.1 Implement `get` command: resolve kind, determine namespace (default to "default" for namespaced, None for cluster-scoped), call `client.list()` or `client.get()`, format output based on `-o` flag
- [x] 11.2 Implement namespace flag handling: if kind is cluster-scoped and `-n` is provided, warn to stderr and ignore
- [x] 11.3 Implement label selector flag: parse `-l` value, pass to client as `LabelSelector`
- [x] 11.4 Implement auto-pagination: follow `continue_token` in list responses until exhausted, collect all items
- [x] 11.5 Implement `--limit` flag: pass to client as `ListOptions.limit`
- [x] 11.6 Implement `apply` command: read file, parse JSON/YAML, resolve kind, GET current object (handle 404), merge changes (preserve system.*, replace spec, merge labels/annotations), call `client.create()` or `client.update()`, handle 409 conflict
- [x] 11.7 Implement `delete` command: resolve kind, call `client.delete()`, return success (no warning for finalizers)
- [x] 11.8 Implement `watch` command: resolve kind, build WatchFilter from label selector, call `client.watch()`, stream events with formatting (table/json/yaml)
- [x] 11.9 Implement `status get` command: resolve kind, call `client.get_status()`, format output
- [x] 11.10 Implement `status apply` command: read file, resolve kind, call `client.update_status()`, format output
- [x] 11.11 Add comments explaining apply merge semantics and conflict handling
- [x] 11.12 Run `cargo check -p kapi-cli` to verify compilation

## 12. kapi-cli: Shell Completions

- [x] 12.1 Implement `completions` command using `clap_complete`: generate completions for bash, zsh, fish, powershell
- [x] 12.2 Output completion script to stdout
- [x] 12.3 Add comments explaining how to install completions
- [x] 12.4 Run `cargo check -p kapi-cli` to verify compilation

## 13. kapi-cli: Error Handling

- [x] 13.1 Create `kapi-cli/src/error.rs` with `CliError` enum: `ConfigError`, `ResolutionError`, `ClientError`, `IoError`, `FormatError`
- [x] 13.2 Implement error formatting: context-rich messages to stderr (e.g., "Widget 'my-widget' not found in namespace 'default'")
- [x] 13.3 Implement exit code 1 on error
- [x] 13.4 Add special error messages for common cases: schema not found, object not found, conflict
- [x] 13.5 Add comments explaining error handling strategy
- [x] 13.6 Run `cargo check -p kapi-cli` to verify compilation

## 14. Integration Testing

- [x] 14.1 Start kapi-server in background for testing
- [x] 14.2 Test `kapi get Schema` lists registered schemas
- [x] 14.3 Test `kapi apply -f schema.json` registers a schema
- [x] 14.4 Test `kapi get Widget` with short name resolution
- [x] 14.5 Test `kapi apply -f widget.json` creates an object
- [x] 14.6 Test `kapi get Widget my-widget` retrieves the object
- [x] 14.7 Test `kapi apply -f widget-updated.json` updates the object
- [x] 14.8 Test `kapi delete Widget my-widget` deletes the object
- [x] 14.9 Test `kapi watch Widget` streams events
- [x] 14.10 Test `kapi get Widget -l app=nginx` filters by label
- [x] 14.11 Test `kapi get Widget -n production` uses specified namespace
- [x] 14.12 Test `kapi get Schema -n default` warns and ignores namespace
- [x] 14.13 Test `kapi status get Widget my-widget` retrieves status
- [x] 14.14 Test `kapi status apply Widget my-widget -f status.json` updates status
- [x] 14.15 Test error cases: schema not found, object not found, conflict
- [x] 14.16 Stop kapi-server

## 15. Documentation and Roadmap

- [x] 15.1 Check `docs/` directory for existing CLI documentation, update if necessary
- [x] 15.2 Update `roadmap.md`: mark "Implement kapi-client HTTP client library" as complete
- [x] 15.3 Update `roadmap.md`: mark "Implement kapi-cli with full command coverage" as complete
- [x] 15.4 Add README section for CLI usage examples (get, apply, delete, watch, status)
- [x] 15.5 Add README section for configuration file format
- [x] 15.6 Add README section for shell completion installation

## 16. Final Verification

- [x] 16.1 Run `cargo check --workspace` to verify all crates compile
- [x] 16.2 Run `cargo clippy --workspace` and fix all warnings
- [x] 16.3 Run `cargo test --workspace` to verify all tests pass
- [x] 16.4 Run `cargo build --release` to verify release build succeeds
- [x] 16.5 Test CLI binary manually with various commands
- [x] 16.6 Verify shell completions work in bash/zsh
- [x] 16.7 DO NOT auto-commit — user wants to review first
