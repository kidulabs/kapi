# kapi-cli

Command-line interface for the kapi API server.

## Usage

The `kapi` CLI provides verb-first commands for managing resources:

```bash
# List all schemas
kapi get Schema

# Register a schema from a file
kapi apply -f schema.json

# Create or update an object
kapi apply -f widget.json

# Apply from stdin
echo '{"kind":"Widget","apiVersion":"example.io/v1","metadata":{"name":"my-widget"},"spec":{"color":"blue"}}' | kapi apply -f -

# Get a specific object
kapi get Widget my-widget

# List objects with label selector
kapi get Widget -l app=nginx

# List objects across all namespaces
kapi get Widget -A

# Edit an object in your editor
kapi edit Widget my-widget

# Delete an object
kapi delete Widget my-widget

# Watch for changes
kapi watch Widget

# Watch for changes across all namespaces
kapi watch Widget -A

# Get status subresource
kapi status get Widget my-widget

# Update status subresource
kapi status apply Widget my-widget -f status.json
```

### Output Formats

Use `-o` to change output format:

```bash
kapi get Widget -o json
kapi get Widget -o yaml
kapi get Widget -o table  # default
```

### Namespace Handling

- Namespaced kinds default to the `default` namespace
- Use `-n` to specify a namespace: `kapi get Widget -n production`
- Use `-A` or `--all-namespaces` to watch/list across all namespaces: `kapi get Widget -A`
- Cluster-scoped kinds ignore `-n` with a warning

### Case Insensitivity

Kind names are case-insensitive:

```bash
kapi get widget      # works
kapi get Widget      # works
kapi get WIDGET      # works
```

## Configuration

The CLI reads configuration from `~/.kapi/config.yaml`:

```yaml
server: http://localhost:8080
```

Override the config path with the `KAPI_CONFIG` environment variable:

```bash
KAPI_CONFIG=/path/to/config.yaml kapi get Schema
```

If no config file exists, the CLI defaults to `http://localhost:8080`.

## Shell Completions

Generate shell completions for bash, zsh, fish, or powershell:

```bash
# Bash
kapi completions bash > ~/.local/share/bash-completion/completions/kapi

# Zsh
kapi completions zsh > ~/.zfunc/_kapi

# Fish
kapi completions fish > ~/.config/fish/completions/kapi.fish

# PowerShell
kapi completions powershell > kapi.ps1
```
