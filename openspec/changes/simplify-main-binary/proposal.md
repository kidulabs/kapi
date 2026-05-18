## Why

The `main.rs` binary currently mixes bootstrapping concerns (tracing, port parsing) with application construction (store/eventbus instantiation, service wiring, router building). This makes it harder to test, embed, or reuse the application logic. As the only user-facing choice is which `ObjectStore` and `EventPublisher` implementation to use, the binary should be simplified to only handle those choices and delegate all construction to the library.

## What Changes

- **New**: `AppConfig` struct in a dedicated `config` module, holding `port`, `store`, and `event_bus` as required fields
- **New**: `create_app(config) -> Router` function in `lib.rs` for library-level app construction
- **New**: `run(config) -> Result<()>` async function in `lib.rs` for full server lifecycle (bind + serve)
- **Modified**: `main.rs` reduced to tracing init, port parsing, `AppConfig` construction, and a single `kapi::run(config)` call
- **Removed**: All `mod` declarations from `main.rs` (already declared in `lib.rs`)
- **New**: `src/config/mod.rs` module

## Capabilities

### New Capabilities
- `app-config`: Structured configuration for server startup, encapsulating port, store, and event bus choices
- `library-entry-points`: `create_app` and `run` functions as public API for embedding and testing

### Modified Capabilities
<!-- No existing spec-level requirements are changing. This is a refactoring. -->

## Impact

- **Affected files**: `src/main.rs`, `src/lib.rs`, new `src/config/mod.rs`
- **No breaking changes** to existing public APIs (`ObjectStore`, `EventPublisher`, routes, handlers)
- **No dependency changes**
- Tests continue to work unchanged (they already use lib types directly)
