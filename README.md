# kapi

Kubernetes-apiserver-inspired API server in Rust.

## Workspace Structure

- `kapi-core/` — Shared types (`StoredObject`, `ResourceKey`, `WatchEvent`, etc.)
- `kapi-server/` — The API server
- `kapi-client/` — HTTP client library
- `kapi-cli/` — Command-line interface (see [kapi-cli/README.md](kapi-cli/README.md))
- `kapi-controller/` — Controller-runtime SDK (placeholder)
