## Why

P3 (Event Bus) is the next phase in the roadmap — enabling real-time watch semantics via SSE. Additionally, P2b left incomplete cleanup work (schema module files not deleted, ValidationError in wrong module) that must be resolved before P3 can cleanly integrate.

## What Changes

- **Implement EventBus** with per-kind `tokio::broadcast` channels, auto-created on first subscribe, with configurable capacity (default 1024)
- **Implement WatchStream** wrapper that handles lag by terminating the stream (client must re-sync), providing a clean `Stream<Item = WatchEvent>` API
- **Add dead channel cleanup** on publish — when `receiver_count() == 0`, remove the channel from the map
- **Fix P2b incomplete work**: delete `schema/types.rs`, `schema/service.rs`, `schema/handler.rs`; update `schema/mod.rs` to only declare `meta_schema`; move `ValidationError` to `object/types.rs`
- **Update roadmap**: add P10 (periodic cleanup future work), add roadmap hygiene tasks, correct false completions

## Impact

- New module: `src/event/bus.rs` — EventBus, WatchStream
- Deleted files: `src/schema/types.rs`, `src/schema/service.rs`, `src/schema/handler.rs`
- Modified: `src/schema/mod.rs`, `src/error.rs`, `src/object/types.rs`, `roadmap.md`
- No breaking changes to existing store or type APIs
