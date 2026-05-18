## 1. Create config module

- [x] 1.1 Create `src/config/mod.rs` with `AppConfig` struct containing `port: u16`, `store: Arc<dyn ObjectStore>`, `event_bus: Arc<dyn EventPublisher>`. Add doc comments on the struct and each field explaining purpose and usage.
- [x] 1.2 Add `pub mod config;` and `pub use config::AppConfig;` to `src/lib.rs`

## 2. Add library entry points

- [x] 2.1 Implement `pub fn create_app(config: &AppConfig) -> axum::Router` in `src/lib.rs` (meta-schema compilation, ObjectService, AppState, build_router). Add doc comments on the function, its parameters, and return value.
- [x] 2.2 Implement `pub async fn run(config: AppConfig) -> anyhow::Result<()>` in `src/lib.rs` (calls create_app, binds TcpListener, axum::serve). Add doc comments describing the full lifecycle.
- [x] 2.3 Re-export `ObjectStore` trait and `EventPublisher` trait from `lib.rs` root for user convenience. Add doc comments on re-exports explaining where the full types live.

## 3. Simplify main.rs

- [x] 3.1 Remove all `mod` declarations from `src/main.rs`
- [x] 3.2 Replace all crate imports with `use kapi::...` imports
- [x] 3.3 Rewrite `main` function: tracing init, port parsing, AppConfig construction, call `kapi::run(config).await`. Add a brief module-level doc comment explaining the binary's role.
- [x] 3.4 Verify `main.rs` is under 20 lines of code

## 4. Verify and test

- [x] 4.1 Run `cargo check` to verify compilation
- [x] 4.2 Run `cargo test` to verify all existing tests pass
- [x] 4.3 Run `cargo run` to verify server starts and serves requests

## 5. Roadmap audit and update

- [x] 5.1 Compare `roadmap.md` Module Tree section against actual `src/` directory after refactoring — verify it reflects new `config/` module and simplified `main.rs`
- [x] 5.2 Update `roadmap.md` Module Tree section to show `src/config/mod.rs` and updated `main.rs` description
- [x] 5.3 Review `roadmap.md` Design Decisions table — add entry for AppConfig-driven binary construction if not present
- [x] 5.4 Review `roadmap.md` Backlog P7 (Application Wiring) — mark T47–T50 as needing update to reflect new `create_app`/`run` pattern, or add new tasks if the old ones are now stale
- [x] 5.5 Verify all roadmap checkbox states still match codebase after changes
