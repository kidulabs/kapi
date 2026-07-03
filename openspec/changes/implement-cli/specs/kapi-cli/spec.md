## ADDED Requirements

### Requirement: CLI provides verb-first commands
The CLI SHALL provide the following verb-first commands: `get`, `apply`, `delete`, `watch`, `status get`, `status apply`, and `completions`. Schema and Namespace SHALL be treated as regular kinds with no special subcommands.

#### Scenario: Get command
- **WHEN** user runs `kapi get Widget my-widget`
- **THEN** CLI resolves `Widget` to full key and retrieves the object

#### Scenario: Apply command
- **WHEN** user runs `kapi apply -f widget.json`
- **THEN** CLI creates or updates the object from the file

#### Scenario: Delete command
- **WHEN** user runs `kapi delete Widget my-widget`
- **THEN** CLI deletes the object

#### Scenario: Watch command
- **WHEN** user runs `kapi watch Widget`
- **THEN** CLI streams watch events for the kind

#### Scenario: Status get command
- **WHEN** user runs `kapi status get Widget my-widget`
- **THEN** CLI retrieves the status subresource

#### Scenario: Status apply command
- **WHEN** user runs `kapi status apply Widget my-widget -f status.json`
- **THEN** CLI updates the status subresource from the file

#### Scenario: Completions command
- **WHEN** user runs `kapi completions bash`
- **THEN** CLI outputs bash shell completions

### Requirement: CLI resolves short names to full keys
The CLI SHALL resolve short kind names (e.g., `Widget`) to full `group/version/kind` by querying the server's schema list. If the short name is ambiguous (multiple schemas with same kind), the CLI SHALL error and request disambiguation. The CLI SHALL support `group/kind` syntax (e.g., `example.io/Widget`) as an escape hatch.

#### Scenario: Short name resolution
- **WHEN** user runs `kapi get Widget my-widget`
- **THEN** CLI queries schemas, finds `Widget` → `example.io/v1/Widget`, and makes request

#### Scenario: Ambiguous short name
- **WHEN** two schemas have kind `Widget` and user runs `kapi get Widget`
- **THEN** CLI errors with "ambiguous kind 'Widget', use full path: example.io/v1/Widget"

#### Scenario: Group/kind syntax
- **WHEN** user runs `kapi get example.io/Widget my-widget`
- **THEN** CLI resolves to `example.io/v1/Widget` (latest version in that group)

#### Scenario: Schema not found
- **WHEN** user runs `kapi get Wdiget` (typo)
- **THEN** CLI errors with "No schema found for kind 'Wdiget'. Use 'kapi get Schema' to list available kinds"

### Requirement: CLI supports namespace flag
The CLI SHALL support `-n/--namespace` flag to scope commands to a specific namespace. For namespaced kinds, the default namespace SHALL be `"default"`. For cluster-scoped kinds, the `-n` flag SHALL be ignored with a warning to stderr.

#### Scenario: Namespaced command with explicit namespace
- **WHEN** user runs `kapi get Widget my-widget -n production`
- **THEN** CLI retrieves object from `production` namespace

#### Scenario: Namespaced command without namespace
- **WHEN** user runs `kapi get Widget my-widget`
- **THEN** CLI retrieves object from `default` namespace

#### Scenario: Cluster-scoped command with namespace flag
- **WHEN** user runs `kapi get Schema -n default`
- **THEN** CLI warns "Schema is cluster-scoped, ignoring --namespace" and lists all schemas

### Requirement: CLI supports output formats
The CLI SHALL support `-o/--output` flag with values `table` (default), `json`, and `yaml`. Table output SHALL use resource-specific columns:
- Namespaced objects: NAME, NAMESPACE, AGE
- Cluster-scoped objects: NAME, AGE
- Schema: NAME, AGE
- Namespace: NAME, AGE

Watch command SHALL use same formats with table showing: EVENT_TYPE NAME [NAMESPACE] AGE.

#### Scenario: Table output (default)
- **WHEN** user runs `kapi get Widget`
- **THEN** CLI displays table with NAME, NAMESPACE, AGE columns

#### Scenario: JSON output
- **WHEN** user runs `kapi get Widget my-widget -o json`
- **THEN** CLI displays full object as formatted JSON

#### Scenario: YAML output
- **WHEN** user runs `kapi get Widget my-widget -o yaml`
- **THEN** CLI displays full object as YAML

#### Scenario: Watch table output
- **WHEN** user runs `kapi watch Widget`
- **THEN** CLI streams table rows with EVENT_TYPE, NAME, NAMESPACE, AGE columns

### Requirement: CLI implements apply with kubectl-style merge
The `apply` command SHALL read a file containing `{ metadata: { name, labels?, annotations? }, spec: {...} }`. The CLI SHALL GET the current object (or get 404), preserve `system.*` fields, replace `spec` wholesale, and merge `labels`/`annotations` additively. On conflict (409), the CLI SHALL fail immediately with an error.

#### Scenario: Apply creates new object
- **WHEN** user runs `kapi apply -f widget.json` and object doesn't exist
- **THEN** CLI creates the object from the file

#### Scenario: Apply updates existing object
- **WHEN** user runs `kapi apply -f widget.json` and object exists
- **THEN** CLI GETs current object, merges changes from file, PUTs updated object

#### Scenario: Apply conflict
- **WHEN** user runs `kapi apply -f widget.json` and object was modified between GET and PUT
- **THEN** CLI errors with "conflict: object was modified, retry manually"

### Requirement: CLI supports label selectors
The CLI SHALL support `-l/--label-selector` flag on `get` and `watch` commands. The flag SHALL accept standard label selector syntax (e.g., `app=nginx`, `env!=prod`, `app=nginx,env=prod`).

#### Scenario: Get with label selector
- **WHEN** user runs `kapi get Widget -l app=nginx`
- **THEN** CLI retrieves only objects with label `app=nginx`

#### Scenario: Watch with label selector
- **WHEN** user runs `kapi watch Widget -l app=nginx`
- **THEN** CLI streams events only for objects with label `app=nginx`

### Requirement: CLI auto-paginates list operations
The CLI SHALL auto-paginate list operations by following `continue_token` until exhausted. The CLI SHALL expose `--limit` flag as an escape hatch to limit results per page.

#### Scenario: Auto-pagination
- **WHEN** user runs `kapi get Widget` and server returns paginated results
- **THEN** CLI follows `continue_token` and returns all results

#### Scenario: Limit flag
- **WHEN** user runs `kapi get Widget --limit 10`
- **THEN** CLI retrieves at most 10 results per page

### Requirement: CLI uses YAML configuration
The CLI SHALL read configuration from `~/.kapi/config.yaml` (or path from `KAPI_CONFIG` env var). The config file SHALL contain at minimum `server: <url>`. Precedence SHALL be: flag > env > config > default (`http://localhost:8080`).

#### Scenario: Config file exists
- **WHEN** `~/.kapi/config.yaml` contains `server: http://localhost:8080`
- **THEN** CLI uses that server URL

#### Scenario: Config override via env
- **WHEN** `KAPI_CONFIG=/path/to/config.yaml` is set
- **THEN** CLI reads config from that path

#### Scenario: No config file
- **WHEN** no config file exists and no env var is set
- **THEN** CLI defaults to `http://localhost:8080`

### Requirement: CLI provides context-rich errors
The CLI SHALL print errors to stderr with context: what operation failed, what resource was involved, and why. The CLI SHALL exit with code 1 on error. Schema-not-found errors SHALL include a hint to list schemas.

#### Scenario: Object not found
- **WHEN** user runs `kapi get Widget my-widget` and object doesn't exist
- **THEN** CLI errors to stderr: "Widget 'my-widget' not found in namespace 'default'"

#### Scenario: Schema not found
- **WHEN** user runs `kapi get Wdiget` (typo)
- **THEN** CLI errors to stderr: "No schema found for kind 'Wdiget'. Use 'kapi get Schema' to list available kinds"

#### Scenario: Exit code on error
- **WHEN** any command fails
- **THEN** CLI exits with code 1

### Requirement: CLI generates shell completions
The CLI SHALL provide a `completions <shell>` command that generates shell completions for bash, zsh, fish, and powershell using `clap_complete`.

#### Scenario: Generate bash completions
- **WHEN** user runs `kapi completions bash`
- **THEN** CLI outputs bash completion script to stdout

#### Scenario: Generate zsh completions
- **WHEN** user runs `kapi completions zsh`
- **THEN** CLI outputs zsh completion script to stdout
