## Why

kapi currently has no client library or CLI tool, making it difficult to interact with the API server programmatically or from the command line. Users must craft raw HTTP requests or use generic tools like `curl`. Implementing a client library and CLI provides a natural interface for managing schemas, objects, and namespaces, enabling both human operators and automation workflows.

## What Changes

- Implement `kapi-client` HTTP client library with reqwest-based wrappers for all CRUD operations, watch (SSE streaming), and status subresource operations
- Implement `kapi-cli` with verb-first commands: `get`, `apply`, `delete`, `watch`, `status get`, `status apply`, `completions`
- Add short name resolution: CLI resolves `Widget` → `example.io/v1/Widget` via schema queries
- Add label selector support (`-l/--label-selector`) for `get` and `watch` commands
- Add auto-pagination for list operations with `--limit` escape hatch
- Add YAML configuration file support with env var override
- Add shell completion generation (bash/zsh/fish/powershell)

## Capabilities

### New Capabilities

- `kapi-client`: HTTP client library wrapping all kapi-server endpoints (CRUD, watch, status) with explicit low-level methods
- `kapi-cli`: Command-line interface with verb-first commands, short name resolution, output formatting (table/json/yaml), and configuration management

### Modified Capabilities

None. This is entirely new functionality.

## Impact

- **New crates**: `kapi-client` and `kapi-cli` (currently empty placeholders)
- **Dependencies**: `kapi-client` depends on `kapi-core` for shared types; `kapi-cli` depends on `kapi-client`
- **External crates**: `reqwest` (HTTP client), `clap` (CLI parsing), `serde_yaml` (YAML config/output), `eventsource-stream` (SSE parsing)
- **API surface**: No changes to kapi-server API; client wraps existing endpoints
- **Configuration**: New config file at `~/.kapi/config.yaml` (or path from `KAPI_CONFIG` env var)

## Non-goals

- `--dry-run` flag for mutating commands (deferred to v2)
- `--server` CLI flag (use config/env for v1)
- Watch resume with `--from-version` (deferred to v2)
- `fieldSelector` support (only `metadata.name` exists, covered by `get <name>`)
- Custom table columns or `--output wide` (deferred to v2)
- Auth/authentication support (deferred to future work)

## Future Work

- Watch resume capability (depends on server-side watch resume implementation)
- `--dry-run` for `apply` and `delete` commands
- `--server` flag for one-off server specification
- Custom table columns and `--output wide` format
- Auth/authentication integration in config file
- Schema resolution caching to disk with TTL
