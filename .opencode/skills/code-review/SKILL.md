---
name: code-review
description: "CRITICAL: Comprehensive Rust code review. Triggers on: code review, review code, review PR, review changes, /review, check code, review this, review my code, review implementation"
globs: ["**/*.rs"]
---

# Rust Code Review

> Comprehensive review covering correctness, security, crate hygiene, OpenSpec alignment, comment quality, testing, error handling, resource management, API design, concurrency, observability, performance, cognitive complexity, architectural layering, documentation coherence, and naming clarity.

## Review Dimensions

Run all sixteen dimensions. Report findings grouped by severity (CRITICAL / WARNING / SUGGESTION).

---

## 1. Correct Rust Primitives

Verify the code uses appropriate Rust types and idioms for the job.

### Type Selection

| Check | Flag if | Prefer |
|-------|---------|--------|
| Fixed-size collections | `Vec` for known fixed size | Arrays `[T; N]` or smallvec |
| Single-owner shared data | `Rc`/`Arc` unnecessarily | Plain ownership or references |
| Mutable shared state | `Mutex` for bool/usize/counter | `AtomicBool`/`AtomicUsize` |
| String building | `+` concatenation in loops | `format!`, `write!`, or `String::with_capacity` |
| Optional values | `Option` wrapping when `Default` suffices | Default trait or sentinel |
| Error types | Ad-hoc `String` errors | `thiserror` domain errors or `anyhow` for apps |
| Byte buffers | `Vec<u8>` for fixed protocol messages | `[u8; N]` or `SmallVec` |
| Trait objects | `Box<dyn Trait>` when `impl Trait` works | Static dispatch / generics |
| Interior mutability | `RefCell` in single-thread only when `Cell` suffices | `Cell` for `Copy` types |

### Pattern Selection

| Check | Flag if | Prefer |
|-------|---------|--------|
| Error propagation | `match` + manual unwrap | `?` operator |
| Lazy init | `lazy_static!` or `once_cell` (on Rust >= 1.70/1.80) | `std::sync::OnceLock` / `LazyLock` |
| Channel | `std::sync::mpsc` when crossbeam needed | `crossbeam::channel` |
| Mutex | `std::sync::Mutex` when contention likely | `parking_lot::Mutex` |
| Fallible conversion | `as` casts losing data silently | `TryFrom` / `TryInto` |
| Debug formatting | Manual `fmt::Debug` impl when `#[derive(Debug)]` works | Derive macro |
| Iterator | Index-based loops | `.iter()`, `.enumerate()`, iterator combinators |
| Collection lookup | Linear scan for repeated lookups | `HashMap` / `BTreeMap` |

### Misused Primitives

Flag these specifically:
- `unsafe` to escape borrow checker without understanding the root cause
- `.clone()` to satisfy the compiler instead of restructuring ownership
- `unwrap()` on values that can legitimately be `None`/`Err`
- `Vec<Box<T>>` when `Vec<T>` works (unnecessary indirection)
- `String` where `&str` or `Cow<str>` avoids allocation
- `Rc<RefCell<T>>` when ownership can be clarified
- `Arc<Mutex<T>>` for data that is write-once then read-many (use `Arc<OnceLock<T>>` or `Arc<RwLock<T>>`)

---

## 2. Crate Hygiene

Assess third-party dependencies for quality, maintenance, and risk.

### Crate Risk Assessment

For each dependency in `Cargo.toml` or `use` statements:

| Signal | Risk Level | Action |
|--------|-----------|--------|
| < 1000 downloads, no recent commits | HIGH | Flag: "Consider std alternative or more popular crate" |
| No `README.md` or empty docs | HIGH | Flag: "Crate lacks documentation" |
| Known deprecated ecosystem crate | HIGH | Flag with replacement (see table below) |
| Single-maintainer, no CI | MEDIUM | Flag: "Single-maintainer dependency risk" |
| Duplicate functionality with another dep | MEDIUM | Flag: "Consolidate: X and Y overlap" |
| Pre-1.0 API (breaking changes likely) | MEDIUM | Note: "Pre-1.0: pin version, expect breakage" |
| Feature not enabled but code paths use it | MEDIUM | Flag missing feature flag |

### Deprecated / Replaced Crates

| Replace This | Use Instead |
|--------------|-------------|
| `lazy_static` | `std::sync::OnceLock` (>= 1.70) |
| `once_cell` (Lazy) | `std::sync::LazyLock` (>= 1.80) |
| `failure` | `thiserror` + `anyhow` |
| `error-chain` | `thiserror` + `anyhow` |
| `chrono` (new code) | `time` crate (consider maintenance status) |
| `serde_json` for streaming | `serde_json` with `Stream` or `simd-json` for perf |
| `reqwest` blocking | `reqwest` async or `ureq` for sync |
| `diesel` (new projects) | Evaluate `sqlx` for async-native needs |

### Prefer std Over Crate

Flag when a crate is used for something available in std:
- Custom base64 → use `STANDARD` from `base64` only if needed; check if std covers the case
- Custom hex encoding → check if std suffices before pulling a crate
- Custom URL parsing → use `url` crate only when needed; simple cases may not need it
- Custom path manipulation → `std::path::Path` / `PathBuf`

**Rule:** Every external dependency is a maintenance burden. Justify non-std crates.

---

## 3. OpenSpec Alignment

Verify implementation matches the OpenSpec artifacts for the current change.

### Alignment Checks

1. **Locate the active change** — run `openspec list --json` or check `openspec/changes/`
2. **Load specs** — read the delta specs for the change
3. **Load design** — read `design.md` if it exists
4. **Load tasks** — read `tasks.md` to see what was planned

### What to Check

| Check | How |
|-------|-----|
| Requirements coverage | Each requirement in delta specs has corresponding implementation |
| Design decisions followed | Implementation matches architecture/approach in design.md |
| Scenario coverage | Scenarios described in specs are handled in code |
| Task completion | Planned tasks in tasks.md are implemented |
| Scope creep | Code exists that is NOT in specs (unplanned additions) |
| Naming alignment | Types/functions match terminology from specs |
| Error handling alignment | Error types/cases match spec'd error scenarios |
| API surface alignment | Public API matches what specs describe |

### Reporting

- If specs exist but implementation diverges: CRITICAL
- If design.md decisions are contradicted: WARNING
- If scope creep detected (code beyond specs): WARNING
- If tasks are incomplete: CRITICAL
- If no specs exist for the change: NOTE — recommend creating specs

---

## 4. Security Review

Flag patterns that create security vulnerabilities.

### Critical Security Patterns

| Pattern | Risk | Severity |
|---------|------|----------|
| `unsafe` without `SAFETY` comment | UB, memory corruption | CRITICAL |
| `unwrap()` / `expect()` on untrusted input | Panic DoS | CRITICAL |
| `static mut` without synchronization | Data races | CRITICAL |
| `transmute` without size/layout validation | UB, type confusion | CRITICAL |
| Deserialization without validation | Injection, corruption | CRITICAL |
| SQL query with string concatenation | SQL injection | CRITICAL |
| Command execution with unsanitized input | Command injection | CRITICAL |
| Path construction from user input without validation | Path traversal | CRITICAL |
| Hardcoded secrets/credentials | Credential leak | CRITICAL |
| `impl Send` / `impl Sync` without verification | Data races across threads | CRITICAL |

### Warning-Level Security Patterns

| Pattern | Risk | Severity |
|---------|------|----------|
| Weak randomness (`rand` without `CryptoRng`) for security | Predictable values | WARNING |
| Insufficient error context in logs | Information leak or missing audit trail | WARNING |
| Missing input length/size limits | Resource exhaustion | WARNING |
| Blocking I/O in async context | DoS via thread starvation | WARNING |
| Holding locks across `.await` | Deadlocks, priority inversion | WARNING |
| Missing timeout on network/IO operations | Resource hang | WARNING |
| Unbounded collections from untrusted input | Memory exhaustion | WARNING |
| `ToString` / `Display` of sensitive data in logs | Secret leakage | WARNING |
| TOCTOU patterns (check then use without atomicity) | Race conditions | WARNING |

### Security Checklist

- [ ] No `unsafe` without `// SAFETY:` comment
- [ ] No `unwrap()` on external/untrusted data
- [ ] No string interpolation in SQL/commands
- [ ] No hardcoded secrets (use env vars or config)
- [ ] Input validation at trust boundaries
- [ ] Proper error handling (no panic propagation from user input)
- [ ] No sensitive data in log output
- [ ] Cryptographic operations use proper random sources

---

## 5. Comment Quality

This is the most nuanced dimension. Comments are a maintenance liability when they can become wrong without anyone noticing.

### The Falsifiability Principle

**A comment is dangerous if it can become false without the code changing.**

Comments about external conditions, other crates' behavior, or transient limitations are the most dangerous because they can become wrong when:
- A dependency updates and fixes a "limitation"
- A compiler version changes behavior
- A std library adds a feature we said didn't exist
- An external API changes its contract

### Comment Anti-Patterns (Flag and Remove/Rewrite)

| Anti-Pattern | Example | Why Dangerous | Fix |
|-------------|---------|---------------|-----|
| Crate limitation assumption | `// We can't use X::method() because it doesn't support Y` | Crate may add Y support; comment becomes lie | State the positive reason: `// We use Z because we need Y behavior` |
| Version-locked assumption | `// As of Rust 1.70, there is no std way to...` | New Rust versions add features; comment goes stale | State what we need: `// We need X behavior, implemented via Z` |
| Bug assumption | `// This crate has a bug where...` | Bug gets fixed; workaround becomes dead code and comment becomes lie | File issue, add regression test, comment with `// WORKAROUND: see issue #N` |
| Obvious restatement | `// Increment counter` before `counter += 1` | Noise; signals nothing code doesn't say | Remove |
| Historical accident | `// This used to be X but we changed it` | Irrelevant once changed; clutters | Use git history for "used to be"; comment should describe current state |
| Speculative future | `// This might break if...` | Vague fear; no action item | Either handle the case or add a test that validates the assumption |
| TODO without owner | `// TODO: fix this later` | No accountability; forgotten forever | `// TODO(@owner): fix by <date> — tracked in issue #N` |
| Disputed correctness | `// This is correct because the docs say...` | Docs change; link rot | Reference specific version: `// Per docs v1.2.3 §section: ...` or cite the invariant we uphold |

### Comment Requirements (What MUST Be Commented)

| Must Comment | Why |
|-------------|-----|
| `unsafe` blocks | `// SAFETY:` explaining invariants upheld |
| Non-obvious `unwrap()`/`expect()` | Why this value is guaranteed to be `Some`/`Ok` |
| Workarounds with issue links | `// WORKAROUND: <issue-url> — <what we're working around>` |
| Complex business logic | What invariant or rule this implements (not how the code works) |
| Deliberate non-idiomatic code | Why the non-obvious approach is necessary |
| Lock ordering | What order locks must be acquired and why |
| `unsafe impl Send/Sync` | What invariant makes this thread-safe |
| Magic numbers / constants | What the value represents |
| Error conversions | Why this error type maps to that one |
| Algorithmic complexity choices | Why O(n^2) is acceptable here (small N, etc.) |

### The Comment Test

For each comment, ask:
1. **Does the code already say this?** → Remove the comment.
2. **Would this comment become wrong if a dependency updated?** → Rewrite to state the positive reason we chose this approach, not the negative property of the alternative.
3. **Does this comment explain WHY, not WHAT?** → Good. Keep it.
4. **Could someone reading only this comment (without code) be misled?** → Rewrite or remove.

### Good Comment Pattern

```rust
// BAD (falsifiable — depends on external crate behavior that may change):
// We don't use serde_yaml because it doesn't support merge keys.
// So we parse manually.

// GOOD (states our positive requirement):
// We need YAML merge key support for config inheritance.
// Current approach: custom parser handles << merge syntax.
// If a dependency supports this natively, we can switch (see issue #N).
```

```rust
// BAD (falsifiable — asserts a limitation that may be fixed):
// HashMap doesn't preserve insertion order, so we use BTreeMap.
// (Reader thinks: "just use IndexMap")

// GOOD (states our actual requirement):
// We need deterministic iteration order for config key serialization.
// BTreeMap provides sorted-order iteration.
```

---

## 6. Testing & Verification

Code without tests is a liability. Verify that new code is testable and adequately tested.

### Test Coverage Checks

| Check | Flag if | Expectation |
|-------|---------|-------------|
| New public functions | No unit tests | At least happy path + one error path |
| Error paths | Only happy path tested | Error branches must have tests |
| Edge cases | Empty inputs, boundaries, overflow untested | Boundary conditions tested |
| Complex logic branches | Conditional complexity > 3 without tests | Each branch has a test case |
| State machines / transitions | State changes untested | Each valid transition tested |
| Parsing / serialization | No round-trip tests | Serialize → deserialize → assert equal |

### Testability Review

| Anti-Pattern | Why Bad | Fix |
|-------------|---------|-----|
| Direct filesystem/DB access in business logic | Can't test without real resources | Inject via trait or dependency |
| `std::env::var()` called deep in logic | Can't set env in tests | Inject config struct |
| `SystemTime::now()` embedded in logic | Non-deterministic tests | Inject clock / `fn now() -> Instant` |
| No return value (only side effects) | Can't assert behavior | Return result or use builder pattern |
| God structs with 10+ fields | Impossible to construct in tests | Split into smaller types with constructors |
| `println!` for output | Can't capture in tests | Return formatted string or use logging facade |

### Test Quality

| Check | Severity |
|-------|----------|
| Tests that assert nothing meaningful (`assert!(true)`) | WARNING |
| Tests with hardcoded paths or env-dependent setup | WARNING |
| Test names that don't describe the scenario | SUGGESTION |
| Missing property-based tests for complex transformations (use `proptest`) | SUGGESTION |
| Test code duplication across test functions | SUGGESTION |
| Tests that depend on execution order | WARNING |
| `#[ignore]` tests without issue reference | WARNING |

### What Must Be Tested

- All public API functions (at minimum happy path)
- All error variants in domain types
- Serialization/deserialization round-trips
- Boundary conditions (empty, zero, max, negative)
- Concurrency-sensitive code (use `loom` or stress tests)
- State transitions in state machines

---

## 7. Error Handling Depth

Beyond "don't unwrap" — verify the error architecture is sound.

### Error Taxonomy

| Check | Flag if | Prefer |
|-------|---------|--------|
| Error type design | Single `String` error for all cases | Enum with variants per failure mode |
| Error context | `.map_err(\|e\| e.to_string())` losing context | `.context("what failed")` (anyhow) or `#[source]` (thiserror) |
| Error granularity | One catch-all `Error` variant | Separate variants for distinct failures |
| Error naming | `Error` without domain prefix | `ConfigError`, `NetworkError`, `ParseError` |
| Display impl | Unhelpful messages (`"error"` or `"failed"`) | Actionable: `"config file not found: {path}"` |

### Error Propagation

| Pattern | Flag if | Fix |
|---------|---------|-----|
| `unwrap()` in library code | Any library/boundary code | `?` or explicit handling |
| Silent error swallowing | `let _ = fallible_fn();` | Log, propagate, or explicitly ignore with comment |
| Error type mismatch | Manual `From` impls for every conversion | `thiserror` `#[from]` or `#[source]` |
| Boxing all errors | `Box<dyn Error>` in internal APIs | Concrete error enum for internal, `Box<dyn Error>` only at boundary |
| Catching `Result` with `_ =>` | Match arm discards error info | Bind the error, log context |
| Retrying without backoff | Retry loop with no delay | Exponential backoff with jitter |
| Retry on non-transient errors | Retrying auth failures, validation errors | Distinguish transient vs permanent |

### Error Checklist

- [ ] Domain errors are enums, not strings
- [ ] Error messages are actionable (tell the user what to do)
- [ ] Error context preserved through propagation chains
- [ ] Transient vs permanent errors are distinguished where retries exist
- [ ] No `.unwrap()` at service/library boundaries
- [ ] Errors include enough context to debug without reproduction

---

## 8. Resource Management

Verify resources are properly acquired, scoped, and released — especially on error paths.

### RAII Compliance

| Check | Flag if | Fix |
|-------|---------|-----|
| File handles | `File::open()` without scoped drop or `?` propagation | RAII — `File` drops automatically, but check error paths |
| Temp files | Created but not cleaned up on error | Use `tempfile` crate or explicit cleanup in error path |
| DB connections | Borrowed and held across function boundaries | Scope to smallest necessary block |
| Lock guards | Held longer than needed | Drop guard before expensive computation |
| Child processes | Spawned without `wait()` or timeout | Always await/kill with timeout |
| Network connections | Opened but not closed on error | Use connection pools or ensure Drop |

### Resource Leak Patterns

| Pattern | Risk | Severity |
|---------|------|----------|
| `Mutex::lock()` then early return without explicit unlock | Usually fine (guard drops), but check if guard is moved | WARNING |
| Opening files in a loop without limiting concurrent handles | FD exhaustion | WARNING |
| Connection pool without max size / timeout | Resource exhaustion under load | WARNING |
| `Arc` cycles preventing Drop | Memory leak | CRITICAL |
| Detached tasks holding resources | Resources held indefinitely | WARNING |
| `mem::forget()` without comment | Intentional leak or bug | CRITICAL |
| `Box::leak()` without comment | Permanent allocation | CRITICAL unless intentional |
| `Rc`/`Arc` stored in self-referential structures | Prevents Drop | CRITICAL |

### Error Path Cleanup

The most common resource leak: resources acquired before a `?` that returns early.

```rust
// BAD — temp file created, but if parse fails, temp file leaks on disk
let temp = NamedTempFile::new()?;
write_data(&temp)?;
let result = parse_file(temp.path())?;  // if this fails, temp persists until process exit
// (NamedTempFile handles this, but manual temp paths don't)

// GOOD — explicit cleanup or use RAII types that clean on drop
```

### Resource Checklist

- [ ] All acquired resources have a clear release path (RAII or explicit)
- [ ] Error paths don't leak resources (files, connections, locks, memory)
- [ ] Connection pools have max size and idle timeout
- [ ] No `mem::forget` / `Box::leak` without explicit justification comment
- [ ] Lock scope is minimal — not held across I/O or expensive computation
- [ ] Child processes always awaited with timeout

---

## 9. API Design

Review public API surface for ergonomics, forward-compatibility, and correctness.

### Lifetime Ergonomics

| Check | Flag if | Fix |
|-------|---------|-----|
| Overly restrictive lifetimes | `fn process<'a>(x: &'a T, y: &'a T)` when lifetimes are independent | Use separate lifetime params |
| Unnecessary `'static` bounds | `T: 'static` when not needed | Remove bound or use `T: 'a` |
| Lifetime elision missed | Explicit lifetimes that compiler could infer | Remove explicit annotations |
| Returning references to self | `fn get(&self) -> &T` when ownership is clearer | Return owned type or use `Cow` |

### Trait Design

| Check | Flag if | Fix |
|-------|---------|-----|
| Excessive trait bounds | `fn foo<T: Clone + Debug + Send + Sync + 'static>(...)` | Reduce to only what's needed |
| Missing `Send`/`Sync` | Async types that should be Send but aren't | Check inner types, add bounds or use `Arc` |
| Unsealed public traits | Trait can be implemented by downstream in unexpected ways | Use sealed trait pattern if extensibility is unwanted |
| `Into<T>` vs `From<T>` | Implementing both when one suffices | Only implement `From`, `Into` is auto-derived |
| Blanket impl conflicts | Generic impl overlapping with specific impl | Restructure with wrapper types |

### Forward Compatibility

| Check | Flag if | Fix |
|-------|---------|-----|
| Public struct with all public fields | Can't add fields without breaking | Use `#[non_exhaustive]` or builder pattern |
| Public enum without `#[non_exhaustive]` | Adding variant is breaking | Add `#[non_exhaustive]` |
| Public function with 5+ params | Hard to extend without breaking | Use builder pattern or options struct |
| Tuple struct as public API | Field order is part of API | Use named struct |
| Breaking change in minor version | Removed/renamed public items | Deprecate first, then remove in next major |

### Visibility

| Check | Flag if | Fix |
|-------|---------|-----|
| `pub` on items only used internally | Unnecessary API surface | Remove `pub`, use `pub(crate)` or private |
| `pub fn` with `pub` fields exposing invariants | Invariants can be violated | Make fields private, provide validated constructors |
| Internal helper types exposed | Leaks implementation details | Restrict visibility |

### API Checklist

- [ ] `#[non_exhaustive]` on public structs and enums intended to grow
- [ ] Builder pattern for functions with 4+ parameters
- [ ] Minimal trait bounds — only what's actually needed
- [ ] No unnecessary `pub` — restrict visibility to minimum needed
- [ ] Owned types returned when caller needs ownership; references when they don't
- [ ] Sealed traits where downstream implementation would be incorrect

---

## 10. Concurrency Deep-Dive

Verify thread safety, lock discipline, and async correctness.

### Lock Discipline

| Check | Flag if | Fix |
|-------|---------|-----|
| Undocumented lock ordering | Multiple locks acquired in different orders across codebase | Document global lock ordering; add comment at each acquisition site |
| Lock held across `.await` | `MutexGuard` lives across await point | Drop guard before await; use `async`-aware mutex if needed |
| Lock held during I/O | Expensive I/O inside `lock()` block | Do I/O outside, lock only for state update |
| Nested locks without ordering guarantee | Potential deadlock | Establish and document lock hierarchy |
| `parking_lot::Mutex` with `Send` but not `Sync` type inside | Compile error or UB | Ensure inner type is `Send` |
| `RwLock` with write-heavy workload | Write starvation or overhead | Use `Mutex` for write-heavy; `RwLock` for read-heavy |

### Atomic Ordering

| Pattern | Flag if | Correct Ordering |
|---------|---------|-----------------|
| Simple flag / counter | `SeqCst` when not needed | `Relaxed` for counters, `Acquire`/`Release` for flags |
| Publish-subscribe pattern | `Relaxed` for data + flag | `Release` on write, `Acquire` on read |
| Counter for metrics | `SeqCst` | `Relaxed` is sufficient |
| Memory barrier needed | No fence between dependent ops | `fence(Acquire)` / `fence(Release)` |
| `AtomicPtr` usage | No provenance documentation | Document what the pointer points to and lifetime |

### Async Correctness

| Pattern | Risk | Severity |
|---------|------|----------|
| `spawn()` without `JoinHandle` handling | Dropped task, resources leak | WARNING |
| `block_on()` inside async runtime | Deadlock | CRITICAL |
| `std::thread::sleep()` in async fn | Blocks executor thread | CRITICAL — use `tokio::time::sleep` |
| `std::fs::*` in async fn | Blocking I/O on executor | CRITICAL — use `tokio::fs` or `spawn_blocking` |
| Unbounded `mpsc` channel | Memory exhaustion under backpressure | Use bounded channel with `try_send` or backpressure |
| `select!` without timeout | Hangs indefinitely | Add timeout branch |
| `tokio::task::spawn_local` on wrong thread | Panic | Use only on LocalSet threads |

### Send / Sync

| Pattern | Flag if | Fix |
|---------|---------|-----|
| `unsafe impl Send for X` | No SAFETY comment proving thread safety | Add `// SAFETY:` with invariant proof |
| `unsafe impl Sync for X` | No SAFETY comment | Add `// SAFETY:` with invariant proof |
| `Rc` in `Send` type | Compile error or hidden UB | Use `Arc` instead |
| Raw pointer in shared type | No sync guarantee | Use `AtomicPtr` or `Mutex<*const T>` with docs |

### Concurrency Checklist

- [ ] Lock ordering documented and consistent
- [ ] No locks held across `.await` points
- [ ] No blocking I/O on async executor threads
- [ ] Atomic orderings justified (not defaulting to `SeqCst` everywhere)
- [ ] `unsafe impl Send/Sync` has SAFETY comments
- [ ] Spawned tasks are awaited or have cleanup on drop
- [ ] Channels are bounded where backpressure matters

---

## 11. Observability

Code must be debuggable in production. Verify logging, metrics, and error context are adequate.

### Logging

| Check | Flag if | Fix |
|-------|---------|-----|
| `println!` / `eprintln!` in library/service code | No structured logging | Use `tracing` / `log` facade |
| Sensitive data in log output | Passwords, tokens, PII logged | Redact or mask; use `secrecy` crate |
| Missing context in error logs | `log::error!("failed")` without details | `log::error!(?err, "operation X failed for entity Y")` |
| Log level misuse | `info!` for debug data, `error!` for expected conditions | `debug!` for details, `info!` for milestones, `error!` for failures |
| No correlation ID / request ID | Can't trace request through logs | Use `tracing::Span` with request ID |

### Error Context in Logs

```rust
// BAD — no context, impossible to debug from logs alone
log::error!("database connection failed");

// GOOD — actionable context
log::error!(
    host = %db_host,
    port = db_port,
    attempts = retry_count,
    "database connection failed after {retry_count} attempts"
);
```

### Metrics & Health

| Check | Flag if | Fix |
|-------|---------|-----|
| Long-running operations without progress indicators | Can't tell if hung or slow | Add span/tracing with duration |
| Service entry points without request logging | Can't audit traffic | Log request start with key params |
| No error rate tracking | Can't detect regressions in production | Add metrics counter for error paths |
| Health check missing | Can't monitor liveness | Implement health endpoint |

### Debug Trait

| Check | Flag if | Fix |
|-------|---------|-----|
| Public types without `Debug` derive | Can't debug-print in logs | `#[derive(Debug)]` |
| `Debug` impl that leaks secrets | Tokens/passwords visible in debug output | Custom `Debug` impl that redacts |
| Missing `Display` for public error types | `{:?}` only, no user-friendly message | Implement `Display` |

### Observability Checklist

- [ ] No `println!`/`eprintln!` in service code — use `tracing`/`log`
- [ ] No sensitive data in log output
- [ ] Error logs include enough context to diagnose without reproduction
- [ ] Public types derive `Debug` (with redaction for secrets)
- [ ] Public error types implement `Display` with actionable messages
- [ ] Key operations have tracing spans for request tracking

---

## 12. Performance Hotspots

Flag unnecessary allocations and algorithmic inefficiency in likely hot paths.

### Allocation Patterns

| Check | Flag if | Fix |
|-------|---------|-----|
| `Vec::new()` + push in loop | Known size available | `Vec::with_capacity(n)` |
| `format!()` in hot loop | String allocated per iteration | Pre-allocate or use `write!` to buffer |
| `.to_string()` on `&str` in loop | Unnecessary allocation per iteration | Use `&str` or `Cow<str>` |
| `.collect::<Vec<_>>()` then iterate | Could chain iterators | Chain without collecting |
| `String` + `+` in loop | O(n^2) concatenation | `String::with_capacity` + `push_str`, or `format!` |
| `Box::new()` in loop | Allocation per iteration | Pool or pre-allocate |
| `clone()` inside loop body | Copying data that could be borrowed | Pass `&T` instead |

### Algorithmic Concerns

| Pattern | Complexity | Flag if |
|---------|-----------|---------|
| Linear search in loop | O(n*m) | Collection is large or loop runs often |
| Nested iteration | O(n^2) | Both collections > ~100 elements |
| Repeated `HashMap::get()` in loop | O(n) lookups | Could batch or restructure |
| Sorting when only min/max needed | O(n log n) | Use `Iterator::min()`/`max()` |
| `BTreeMap` for random access | O(log n) per access | Use `Vec` + sort if batch access |
| Regex compiled in function body | Recompiled per call | Compile once with `lazy_static` or `OnceLock` |

### Async Performance

| Pattern | Issue | Fix |
|---------|-------|-----|
| `spawn_blocking` for CPU-bound work | Blocks executor if not truly CPU-bound | Verify it's actually CPU-bound |
| Sequential `await` for independent operations | Serial when parallel possible | Use `tokio::join!` or `FuturesUnordered` |
| Large buffer allocations per request | GC-like pressure | Pool buffers or reuse |
| Unnecessary `Arc::clone()` in hot path | Atomic ref count bump | Borrow if lifetime allows |

### When NOT to Flag

- One-time initialization code (not a hot path)
- Test code (correctness > performance)
- Small collections (< ~100 elements) where O(n) is fine
- Code where readability clearly outweighs micro-optimization
- Without profiling evidence — flag as SUGGESTION, not WARNING, unless obviously O(n^2) on large data

### Performance Checklist

- [ ] `with_capacity` used where size is known
- [ ] No `.collect::<Vec<_>>()` when iterators could chain
- [ ] No string concatenation in loops
- [ ] No regex compiled inside hot functions
- [ ] Independent async operations run concurrently
- [ ] `clone()` eliminated from hot loops by borrowing

---

## 13. Cognitive Complexity & Readability

Code is read far more often than it is written. Prefer human-readable code over clever code. Flag functions that are hard to understand at a glance.

### Function Complexity Signals

| Signal | Threshold | Action |
|--------|-----------|--------|
| Function length | > 50 lines (excluding blank/comments) | WARNING — suggest extracting helper functions |
| Function length | > 100 lines | CRITICAL — must refactor |
| Nesting depth | > 3 levels of `if`/`match`/`loop` | WARNING — flatten with early returns or extract |
| Branch count | > 5 `if`/`match` arms in one function | WARNING — decompose into smaller functions |
| Parameter count | > 4 parameters | WARNING — use builder, config struct, or decompose |
| Return complexity | Multiple `Result<T, E>` layers or nested `Option<Result<...>>` | WARNING — simplify error types |
| Cyclomatic complexity | > 10 (rough estimate: branches + loops) | WARNING — decompose |

### Single Responsibility

A function should do **one thing** and do it well. Flag violations:

| Anti-Pattern | Description | Fix |
|-------------|-------------|-----|
| Mixed abstraction levels | High-level orchestration mixed with low-level details (e.g., HTTP parsing + business logic + DB writes in one function) | Extract each abstraction level into its own function |
| Multiple unrelated tasks | Function name requires "and" (e.g., `validate_and_save_and_notify`) | Split into `validate`, `save`, `notify` — compose at call site |
| Side effects + return value | Function both mutates state and returns computed value | Separate: one function for mutation, one for computation |
| Validation + transformation | Input validation mixed with data transformation | Validate first (early return), then transform in separate function |
| Error handling mixed with happy path | `match` arms with 20 lines each | Extract error handling into separate functions or use `?` early |
| Setup + work + teardown | Resource acquisition, processing, and cleanup all inlined | Use RAII types or extract setup/teardown; keep work in middle |

### Readability vs Cleverness

**Clever code is code that makes the reader think.** Flag these patterns:

| Clever Pattern | Why Hard to Read | Prefer |
|----------------|------------------|--------|
| Excessive iterator chaining | `.iter().filter().map().fold().collect()` in one expression | Break into named intermediate variables |
| Complex one-liners | `if let Some(x) = map.get(&k).filter(|v| v.len() > 3).map(|v| v[0]) { ... }` | Extract to named variables with clear types |
| Nested `match` with guards | `match` inside `match` with `if` guards | Flatten with early returns or extract to functions |
| Turbofish everywhere | `parse::<i32>()`, `collect::<Vec<_>>()` when type inference works | Let inference do its job; use turbofish only when needed |
| Overuse of `?` in complex expressions | `foo()?.bar()?.baz()?` on one line | Break into steps with named variables |
| Complex pattern matching | `Some(Ok(ref x)) if x.len() > 3 => ...` | Extract to helper function with clear name |
| Macro abuse | Custom macros for simple patterns | Use functions or plain code |
| Type-level programming | Complex trait bounds, associated type projections | Add doc comments explaining the "why"; simplify if possible |

### Good Readability Patterns

**Prefer these when they make code clearer:**

```rust
// BAD — clever but hard to follow
let result = items.iter()
    .filter(|x| x.is_valid())
    .map(|x| transform(x))
    .fold(0, |acc, x| acc + x.weight());

// GOOD — readable with named steps
let valid_items: Vec<_> = items.iter().filter(|x| x.is_valid()).collect();
let transformed: Vec<_> = valid_items.iter().map(|x| transform(x)).collect();
let total_weight: u32 = transformed.iter().map(|x| x.weight()).sum();
```

```rust
// BAD — nested logic hard to follow
fn process(data: &Data) -> Result<Output> {
    if data.is_valid() {
        match data.kind {
            Kind::A => {
                if data.priority > 5 {
                    handle_high_priority_a(data)
                } else {
                    handle_low_priority_a(data)
                }
            }
            Kind::B => handle_b(data),
            _ => Err(Error::UnsupportedKind),
        }
    } else {
        Err(Error::InvalidData)
    }
}

// GOOD — early returns flatten the logic
fn process(data: &Data) -> Result<Output> {
    if !data.is_valid() {
        return Err(Error::InvalidData);
    }
    
    match data.kind {
        Kind::A if data.priority > 5 => handle_high_priority_a(data),
        Kind::A => handle_low_priority_a(data),
        Kind::B => handle_b(data),
        _ => Err(Error::UnsupportedKind),
    }
}
```

### When to Suggest Refactoring

**Suggest refactoring when:**

- Function name requires "and" to describe what it does
- Reader must scroll to see the whole function
- Nested `if`/`match` > 3 levels deep
- Function has multiple exit points with different error types
- Same pattern repeated 3+ times (extract helper)
- Function mixes abstraction levels (high-level orchestration + low-level details)
- Complex iterator chains > 5 operations
- Function does validation + transformation + side effects

**Do NOT suggest refactoring when:**

- Function is short (< 20 lines) and clear
- Extracting would create more confusion than it solves
- The "complexity" is inherent to the problem domain
- It's a test function (tests can be longer for clarity)
- It's a simple match statement (pattern matching is idiomatic)

### Cognitive Complexity Checklist

- [ ] No function > 50 lines without clear justification
- [ ] No nesting depth > 3 levels
- [ ] Function names describe ONE thing (no "and" in name)
- [ ] No excessive iterator chaining (> 5 operations in one expression)
- [ ] Early returns used to flatten nested logic
- [ ] Helper functions extracted for repeated patterns
- [ ] Abstraction levels separated (orchestration vs implementation)
- [ ] Complex expressions broken into named intermediate variables

---

## 14. Architectural Layering & Abstraction Leaks

Code should do what belongs at its layer. Flag abstraction leaks and layer violations that erode architectural integrity over time.

### Common Layer Violations

| Layer | Should Contain | Should NOT Contain |
|-------|---------------|-------------------|
| **Domain / Core** | Business rules, invariants, pure logic | DB queries, HTTP requests, file I/O, framework types, serialization formats |
| **Application / Use Case** | Orchestration, transaction boundaries, error mapping | SQL, HTTP status codes, JSON parsing, CLI argument handling |
| **Infrastructure / Adapter** | I/O, external service calls, persistence, serialization | Business logic, domain invariants, use case orchestration |
| **Presentation / API** | Request parsing, response formatting, auth checks | Business rules, data transformation beyond DTO mapping |

### Abstraction Leak Patterns

| Leak | Example | Why It's Wrong | Fix |
|------|---------|----------------|-----|
| **DB in domain** | `User` struct with `#[derive(sqlx::FromRow)]` | Domain type coupled to persistence | Separate domain `User` from DB row `UserRow`; map at boundary |
| **HTTP in domain** | Domain function returns `StatusCode` or `Response` | Business logic knows about HTTP | Return domain `Result<T, DomainError>`; map to HTTP at API layer |
| **JSON in domain** | Domain types with `#[serde(rename = "...")]` for API compat | Domain shape dictated by wire format | Separate DTO from domain type; transform at boundary |
| **CLI in core** | Core function takes `clap::ArgMatches` | Business logic depends on CLI framework | Parse args in CLI layer; pass plain values to core |
| **I/O in pure function** | Function reads env vars, files, or network | Not testable, not pure | Inject dependencies; pass config/clients as params |
| **Business rule in handler** | HTTP handler contains `if user.is_premium && order.total > 100` | Business logic scattered across handlers | Extract to domain service or use case; handler delegates |
| **Error type leak** | Infrastructure error types (`io::Error`, `sqlx::Error`) exposed in public API | Callers depend on implementation details | Define domain error enum; map infrastructure errors at boundary |
| **Config in deep code** | Function deep in call stack reads `env::var("FEATURE_FLAG")` | Config scattered, not testable | Parse config at startup; inject into services |

### Layer Integrity Checks

For each function, ask:

1. **Does this function know about things it shouldn't?**
   - Domain function knows about HTTP, SQL, JSON, file paths?
   - Infrastructure function contains business rules?
   - Handler contains business logic beyond DTO mapping?

2. **Is this function at the right layer?**
   - Should this be moved UP (closer to entry point)?
   - Should this be moved DOWN (closer to infrastructure)?
   - Is this orchestrating when it should be executing (or vice versa)?

3. **Are boundaries clean?**
   - Do layer boundaries have explicit mapping (domain ↔ DTO, domain ↔ DB row)?
   - Are errors mapped at boundaries (infra error → domain error → API error)?
   - Are dependencies injected (not hidden inside functions)?

### Direction of Violations

| Violation Type | Severity | Example |
|----------------|----------|---------|
| **Lower layer knows about upper layer** | CRITICAL | Domain type has `#[serde(...)]` for API |
| **Upper layer contains lower-layer details** | WARNING | Handler has SQL query or DB call |
| **Side effects in pure layer** | WARNING | Domain function does I/O |
| **Missing boundary mapping** | WARNING | Same type used in domain and infrastructure |
| **Error type leak across layers** | WARNING | `io::Error` in public API of library |
| **Config scattered in deep code** | SUGGESTION | `env::var()` called in nested function |

### Good Layering Patterns

```rust
// BAD — abstraction leak: domain knows about DB
#[derive(sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
}

impl User {
    pub fn validate_email(&self) -> bool {
        // business logic mixed with DB concerns
        self.email.contains('@')
    }
}

// GOOD — clean separation
// domain/user.rs
pub struct User {
    pub id: UserId,
    pub email: Email,  // validated type
}

// infrastructure/db.rs
#[derive(sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub email: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: UserId(row.id),
            email: Email::new(row.email).expect("valid in DB"),
        }
    }
}
```

```rust
// BAD — HTTP handler contains business logic
async fn create_order_handler(state: State, req: CreateOrderRequest) -> StatusCode {
    let user = get_user(state.db, req.user_id).await?;
    
    // Business rule leaked into handler
    if user.is_premium && req.total > 100 {
        apply_discount(&mut req);
    }
    
    save_order(state.db, req).await?;
    StatusCode::CREATED
}

// GOOD — handler delegates to use case
async fn create_order_handler(state: State, req: CreateOrderRequest) -> StatusCode {
    let cmd = CreateOrderCommand {
        user_id: req.user_id,
        items: req.items,
    };
    
    // Business logic in use case
    create_order(state.order_service, cmd).await?;
    StatusCode::CREATED
}
```

### Architectural Checklist

- [ ] Domain types have no framework/infra derives (`sqlx::FromRow`, `serde::Serialize` for API)
- [ ] Domain functions return domain types, not HTTP/DB/CLI types
- [ ] Business rules live in domain/use-case layer, not in handlers
- [ ] Infrastructure errors mapped to domain errors at boundaries
- [ ] Configuration parsed at startup, injected into services
- [ ] No I/O in domain/pure functions (dependencies injected)
- [ ] Clear boundary mapping (DTO ↔ domain, DB row ↔ domain)
- [ ] Each function at the right layer (not orchestrating when it should execute, or vice versa)

---

## 15. Documentation Coherence

Documentation that contradicts code is worse than no documentation — it lies to the reader. Verify that docs stay aligned with the current implementation, flag drift introduced by the change under review, and surface gaps where docs are missing.

### Scope of Documentation

Treat "documentation" broadly. Check every place where humans read prose about the system:

| Documentation Surface | Examples |
|----------------------|----------|
| Rustdoc (`///`, `//!`) | Public function/type/module doc comments |
| Code examples in doc comments | `/// # Example` blocks |
| README files | Crate-level and workspace-level READMEs |
| CHANGELOG | `CHANGELOG.md` entries for the current release |
| Domain docs | `docs/` directory, design docs, architecture docs, guides |
| OpenSpec artifacts | `design.md`, specs, `tasks.md` |
| Operational docs | Runbooks, deployment guides, configuration guides |
| CLI help text | `clap` about/help strings, flag descriptions |
| Error messages | User-facing strings that document behavior |

### Checks

#### 1. Code-Documentation Drift (CRITICAL / WARNING)

These are patterns where the code changed but the documentation did not.

| Check | Flag if | Severity |
|-------|---------|----------|
| Public API signature changed | Doc comment still describes the old signature, parameters, or return type | CRITICAL |
| Behavior changed | Doc comment still describes the old behavior ("returns X on success" but it now returns Y) | CRITICAL |
| Renamed symbol | Docs in other files still reference the old name | WARNING |
| Removed public item | README, guide, or doc comment elsewhere still describes the removed item | CRITICAL |
| Error variants changed | Doc comments or guides still list old error cases / omit new ones | WARNING |
| Default values changed | Docs still state the old default | WARNING |
| Feature flag added/removed | Docs don't mention the new flag, or still mention the removed one | WARNING |
| Configuration keys renamed | Config docs or `--help` text still uses old key names | WARNING |
| CLI flags renamed/removed | Help text or README usage examples still show old flags | CRITICAL |

**How to detect:** For every public item modified in the change, compare the diff to the surrounding doc comment. For items referenced by name elsewhere in the codebase, search for stale references.

#### 2. Doc Comment Correctness

| Check | Flag if | Severity |
|-------|---------|----------|
| `# Panics` section | Missing for a function that can legitimately panic; or describes panics that no longer occur | WARNING |
| `# Errors` section | Missing for a function returning `Result`; or lists error variants that don't exist / omits variants that do | WARNING |
| `# Safety` section | Missing for an `unsafe fn` | CRITICAL |
| `# Examples` section | Missing for a non-trivial public API; or example no longer compiles / demonstrates removed API | WARNING |
| Parameter documentation | `param` tags describe old semantics after a rename or type change | WARNING |

#### 3. README and Guide Staleness

| Check | Flag if | Severity |
|-------|---------|----------|
| Usage/install examples | Commands no longer work due to renamed binary, moved file, or changed flag | CRITICAL |
| Feature descriptions | Claims a feature exists that was removed, or omits a new feature | WARNING |
| Configuration examples | Example config files reference removed keys or miss new required keys | CRITICAL |
| Architecture diagrams | Describe modules or data flow that no longer matches the code | WARNING |
| Dependency on removed behavior | Guide relies on behavior the change just modified | CRITICAL |

#### 4. Missing Documentation (Gaps)

| Check | Flag if | Severity |
|-------|---------|----------|
| New public function/type/struct | No `///` doc comment explaining what it is and when to use it | WARNING |
| New public trait | No doc comment documenting contract, implementer obligations, and invariants | WARNING |
| New public module | No `//!` module-level doc explaining the module's purpose and contents | SUGGESTION |
| New error enum variant | No doc comment on the variant explaining when it occurs | WARNING |
| New public macro | No doc comment with usage example | WARNING |
| New CLI command or subcommand | No help text or man-page entry | WARNING |
| New configuration option | No mention in config docs or `--help` | WARNING |
| New invariants or non-obvious contracts | No comment explaining the rule (even if not public) | SUGGESTION |
| Non-obvious algorithm or formula | No doc comment explaining the "what" and "why" | WARNING |

#### 5. CHANGELOG Hygiene

| Check | Flag if | Severity |
|-------|---------|----------|
| Breaking change without CHANGELOG entry | Public API removed, renamed, or behavior-changed | CRITICAL |
| User-visible behavior change | No entry describing the change | WARNING |
| Misleading entry | CHANGELOG describes a change but the implementation differs | WARNING |
| Missing migration note | Breaking change without upgrade instructions | CRITICAL |

#### 6. Cross-Reference Integrity

| Check | Flag if | Severity |
|-------|---------|----------|
| Intra-doc links (`[`Type`]`) | Link target was renamed or removed | WARNING |
| External doc links (URLs) | Known-dead URLs (when verifiable) | SUGGESTION |
| `See also` references | Referenced function/module no longer exists | WARNING |
| Issue/PR references in docs | Referenced issue is closed but comment says "TODO" or "workaround" | SUGGESTION |

### How to Review Documentation in a Change

1. **Identify the change scope** — which public APIs, modules, configs, CLI flags, or behaviors were touched?
2. **Diff the code, then read the docs** — for each public-item change, read the surrounding doc comment and any cross-references.
3. **Search for references** — grep for the old names/signatures in `README.md`, `docs/`, `CHANGELOG.md`, and doc comments across the crate.
4. **Check for new public items** — every new `pub fn`, `pub struct`, `pub trait`, `pub enum`, `pub mod`, variant, or CLI flag needs documentation.
5. **Check examples** — verify that `# Example` blocks in changed items still compile and demonstrate current API.
6. **Check CHANGELOG** — breaking or user-visible changes must be logged.

### Documentation Checklist

- [ ] All public items modified in this change have updated doc comments
- [ ] No doc comment describes removed behavior, parameters, or return types
- [ ] `# Panics`, `# Errors`, `# Safety` sections are accurate and present where required
- [ ] `# Example` blocks still compile and use current API
- [ ] README and guides reflect current CLI flags, config keys, and features
- [ ] CHANGELOG has entries for breaking and user-visible changes
- [ ] New public items have doc comments (at minimum: what it is, when to use it)
- [ ] Intra-doc links resolve to live targets
- [ ] No stale cross-references to renamed or removed symbols

---

## 16. Naming Clarity & Collision

Names are the primary interface between code and its readers. A name that collides with a module path, overloads a generic term, or implies a false relationship creates confusion that compounds over time.

### Namespace Collision

A name is dangerous when it shadows or collides with a module, type, or constant already in scope.

| Check | Flag if | Why Dangerous | Fix |
|-------|---------|---------------|-----|
| Module path collision | A function/variable/type name matches a sibling module (e.g. `fn build_core()` when `crate::core` exists) | Readers parse "this thing from that module" instead of the intended meaning | Use a name that doesn't overlap with any in-scope module or type path |
| Type/variable shadow | A local variable or parameter has the same name as a type in scope | Reader confuses the value with the type, especially in generics or `impl` blocks | Rename the local to distinguish (`pool` vs `Pool`, `config` vs `Config`) |
| Crate name collision | A module or type name matches a dependency crate name | Import paths become ambiguous; `use crate::utils` vs `use utils::...` | Prefix or rename to disambiguate |
| Keyword-adjacent names | Name resembles a Rust keyword or std prelude item (`r#type`, `send`, `drop`, `clone`) | Reader momentarily thinks it's the built-in; tools may highlight confusingly | Add domain context: `send_request`, `drop_connection`, `clone_pool` |

### Overloaded / Generic Terms

Words like `core`, `manager`, `handler`, `processor`, `service`, `engine`, `helper`, `util` carry almost no information. They force the reader to read the implementation to understand what the thing does.

| Generic Term | What It Fails to Convey | Better |
|-------------|------------------------|--------|
| `core` | Which layer? Which domain concept? Collides with module names | Name what it actually is: `reconcile_backend`, `validate_pool`, `run_pipeline` |
| `manager` | What does it manage? Lifecycle? State? Connections? | `PoolLifecycle`, `ConnectionRegistry`, `CacheInvalidator` |
| `handler` | Handles what? From where? | `StoragePoolReconciler`, `HttpRequestValidator`, `SignalHandler` |
| `processor` | Processes what input? What output? | `ManifestParser`, `EventRouter`, `BatchApplier` |
| `service` | Which bounded context? Which operation? | `PoolProvisioner`, `Authenticator`, `MetricsCollector` |
| `helper` / `util` | Helps with what? | Extract the single responsibility and name it |
| `data` / `info` / `context` | What data? What info? | `PoolStatus`, `NodeConfig`, `ReconcileContext` |

### Misleading Names

A name that implies a relationship, behavior, or guarantee that the code does not uphold.

| Check | Flag if | Example | Fix |
|-------|---------|---------|-----|
| False relationship | Name shares a word with a module/type but has no real connection | `reconcile_create_core` implies relation to `crate::core` module | Rename to reflect actual responsibility: `reconcile_create_backend` |
| Implied guarantee | Name promises something the code doesn't deliver | `validate_input` that only checks one field; `ensure_unique` that has a race | Either fulfill the guarantee or rename: `check_name_length`, `best_effort_unique` |
| Action mismatch | Verb doesn't match what the function does | `get_pool()` that also creates; `delete_pool()` that only marks deleted | `get_or_create_pool()`, `deactivate_pool()` |
| Scope understatement | Name describes one branch of a multi-branch function, hiding the others | `reconcile_create` that also handles update, no-op, and reactivation paths | Name the actual scope: `ensure_pool`, `reconcile_present`, `apply_spec` |
| Stale name | Name reflected old implementation; code was refactored but name wasn't | `parse_xml_config` after switching to TOML | Rename to match current format |

### Naming Consistency

The same concept should have the same name everywhere in the codebase.

| Check | Flag if | Example |
|-------|---------|---------|
| Synonym drift | Same concept called different names in different modules | `pool_id` here, `pool_name` there, `identifier` somewhere else |
| Suffix inconsistency | Similar things use different suffixes for the same pattern | `create_pool`, `pool_delete`, `pool_update_status` (verb position varies) |
| Type/function mismatch | Type is named `PoolConfig` but the function that loads it is `load_settings` | Function name should reference the type: `load_pool_config` |

### Naming Checklist

- [ ] No function/type/variable name collides with an in-scope module path
- [ ] No overloaded generic terms (`core`, `manager`, `handler`, `processor`, `service`, `helper`, `util`) without domain qualification
- [ ] Function name accurately describes what it does (verb matches action)
- [ ] No name implies a guarantee the code doesn't deliver
- [ ] Same concept uses the same name across all modules
- [ ] Suffix/verb position is consistent across sibling functions

---

## Review Output Format

```markdown
## Code Review: <scope>

### Summary
| Dimension | Findings |
|-----------|----------|
| Primitives | X critical, Y warnings |
| Crate Hygiene | X critical, Y warnings |
| OpenSpec Alignment | X critical, Y warnings |
| Security | X critical, Y warnings |
| Comments | X critical, Y warnings |
| Testing | X critical, Y warnings |
| Error Handling | X critical, Y warnings |
| Resource Management | X critical, Y warnings |
| API Design | X critical, Y warnings |
| Concurrency | X critical, Y warnings |
| Observability | X critical, Y warnings |
| Performance | X critical, Y warnings |
| Cognitive Complexity | X critical, Y warnings |
| Architectural Layering | X critical, Y warnings |
| Documentation | X critical, Y warnings |
| Naming | X critical, Y warnings |

### CRITICAL (Must Fix)
1. **[Dimension]** file.rs:42 — Description
   Fix: specific action

### WARNING (Should Fix)
1. **[Dimension]** file.rs:88 — Description
   Fix: specific action

### SUGGESTION (Nice to Fix)
1. **[Dimension]** file.rs:120 — Description
   Fix: specific action
```

### Severity Rules

- **CRITICAL**: Security vulnerabilities, UB, spec violations, panics on untrusted input, doc comments that contradict current behavior, README/config examples that no longer work, breaking changes without CHANGELOG entries
- **WARNING**: Suboptimal primitives, risky crate choices, falsifiable comments, missing "why" comments on complex logic, stale but non-blocking docs, missing doc comments on new public items, inaccurate `# Errors`/`# Panics`/`# Safety` sections, names that collide with in-scope module paths, overloaded generic terms (`core`, `manager`, `handler`, `processor`, `service`, `helper`)
- **SUGGESTION**: Style improvements, minor optimizations, comment clarity, missing module-level docs, stale doc URLs, naming consistency improvements (synonym drift, suffix inconsistency)

---

## Integration with Existing Skills

This review skill orchestrates checks from related skills. When deep-diving is needed:

| If finding is about | Reference skill |
|--------------------|----------------|
| `unsafe` / FFI / raw pointers | `unsafe-checker` |
| Ownership / borrow issues | `m01-ownership` |
| Error handling patterns | `m06-error-handling` |
| Concurrency issues | `m07-concurrency` |
| Anti-patterns | `m15-anti-pattern` |
| Performance concerns | `m10-performance` |
| Naming / style | `coding-guidelines` |

---

## DO NOT

- Do not approve code with `unsafe` blocks lacking `SAFETY` comments
- Do not approve comments that assert limitations of external crates
- Do not approve `.unwrap()` on untrusted/external input
- Do not approve dependencies without justification for non-std choices
- Do not approve implementation that contradicts OpenSpec design decisions
- Do not approve code where doc comments describe behavior that no longer matches the implementation
- Do not approve public API changes without corresponding doc comment updates
- Do not approve breaking changes without a CHANGELOG entry and migration note
- Do not flag comments that explain "why" — those are required
- Do not approve names that collide with in-scope module paths or crate names
- Do not approve generic terms (`core`, `manager`, `handler`, `processor`, `helper`) without domain qualification
- Do not flag TODO comments that have an owner and issue reference
- Do not suggest removing ALL comments — only remove falsifiable or obvious ones
