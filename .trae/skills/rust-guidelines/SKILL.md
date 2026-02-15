---
name: rust-guidelines
description: Large-scale Rust project engineering best practices for 100k+ LOC, multi-crate workspaces, and team collaboration
---

# Rust Large-Scale Project Engineering Guidelines

**Applicable to**: Multi-crate workspaces, 100k+ lines of code, 10+ person teams

## 1. Project Architecture & Modularization

### 1.1 Cargo Workspace (Mandatory for large projects)

```
# Root Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/core",          # Core domain models
    "crates/infrastructure",# Infrastructure (DB, MQ, Cache)
    "crates/application",   # Application service layer
    "crates/interfaces",    # HTTP/GRPC/CLI interface layer
    "crates/migration",     # Database migrations
]
default-members = ["crates/application"]
```

**Key points**:
- Always declare `resolver = "2"` to avoid feature conflicts
- Centralize dependency versions in `workspace.dependencies`
- Use `path` dependencies between crates, replace with version numbers before publishing

### 1.2 Module Boundaries & Visibility

- **Interface isolation**: Each crate provides `lib.rs` as facade, use `pub use` to re-export public APIs. **Prohibit** deep references to internal modules from other crates
- **Internal modules**: Set non-public modules as `pub(crate)` or private. Never use `pub mod` to leak implementation details
- **Feature gates**: Use `#[cfg(feature = "...")]` for critical modules to avoid unnecessary dependency bloat

## 2. Core Coding Practices

### 2.1 Error Handling (Production Standard)

**Absolutely prohibited**: Using `.unwrap()`, `.expect()`, `panic!` in library code (except for unrecoverable failures).

**Mandatory approach**:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Infrastructure failure: {0}")]
    Infrastructure(#[from] std::io::Error),
    #[error("Third-party service timeout")]
    Timeout,
}

pub type CoreResult<T> = Result<T, CoreError>;
```

**Key points**:
- Use `thiserror` for domain errors, use `anyhow` only in binary/test entry points for aggregation
- **Context attachment**: `.map_err(|e| CoreError::Infrastructure(e).context("Failed to read config file"))`
- Error propagation must use `?`, **prohibit** manual `match` propagation

### 2.2 Memory & Ownership

**Critical rule**: Abuse of `clone()` causes memory explosion. Direct rejection of such PRs.

**Mandatory reference-first strategy**:

```rust
// WRONG: Batch owns data, causing repeated copies
struct Batch { msgs: Vec<String> }

// CORRECT: Batch stores references, near-zero overhead
struct Batch<'a> { msgs: Vec<&'a str> }
```

**Key points**:
- **Reference lifetimes**: Master non-intrusive lifetime annotations, don't use `clone()` to escape borrow checker
- **Cow pattern**: Use `std::borrow::Cow` for "might modify, might not" data
- **Smart pointers**: Use `Arc` only for thread sharing, `Rc` for single-threaded sharing, **do not overuse**
- **Memory analysis**: Run `cargo bench` + `heaptrack` weekly to detect memory leaks

## 3. Concurrency & Async

### 3.1 Async Runtime Selection

| Scenario | Required Approach | Notes |
|----------|-------------------|-------|
| High-concurrency I/O (Web/Gateway) | `tokio` + multi-threaded scheduler | Threads = CPU cores x2, tune with load testing |
| Embedded/Latency-sensitive | `tokio` + `current_thread` | Single thread, avoid cross-core sync overhead |
| Lightweight tools/Teaching | `async-std` | **Not recommended for large projects**, ecosystem lags |

**Key points**:
- **CPU-intensive tasks**: Must use `tokio::task::spawn_blocking` to offload to thread pool, **never** compute fibonacci directly in `async` functions
- **Blocking prohibition**: Forbid `std::thread::sleep`, sync `std::fs`, sync locks; replace with `tokio::time::sleep`, `tokio::fs`
- **Rate limiting**: Use `tokio::sync::Semaphore` to limit concurrent DB connections/file handles

### 3.2 Async Cancellation & Graceful Shutdown

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();
let child_token = token.child_token();

tokio::select! {
    _ = worker.run(child_token) => {},
    _ = shutdown_signal() => {
        token.cancel();  // Broadcast cancellation signal
    }
}
```

**Key points**:
- Each long-running task must hold a `CancellationToken`, periodically check `.is_cancelled()`
- Resource cleanup (DB connections, file locks) must be implemented in `Drop` or use `ScopeGuard`

## 4. Testing & Quality Gates

### 4.1 Three-Layer Testing System

1. **Unit tests**: `#[cfg(test)]` within modules, mock external dependencies
2. **Integration tests**: `tests/` directory, test public APIs between crates
3. **End-to-end tests**: Separate test project, call real services (daily builds)

**Mandatory coverage**:
- Doc tests: `cargo test --doc`, all public APIs must include `# Examples`
- Fuzzing: `cargo fuzz`, mandatory for parsing modules
- Benchmarking: `criterion`, mandatory for performance-sensitive functions

### 4.2 CI Quality Gates

```yaml
# GitHub Actions / GitLab CI mandatory steps
- run: cargo fmt --check
- run: cargo clippy --workspace -- -D warnings
- run: cargo test --workspace --locked
- run: cargo deny check  # License & dependency security
- run: cargo audit      # Vulnerability scanning
- run: cargo miri test  # Detect unsafe UB (nightly toolchain)
```

**Key points**:
- **Hard red line**: Clippy warnings = build failure, no `allow` bypasses (unless core team review)
- **Dependency audit**: Run `cargo outdated` weekly, dependencies over 6 months old need upgrades

## 5. Performance Engineering

### 5.1 Compile-Time Optimization

```toml
[profile.release]
opt-level = 3          # Full optimization
lto = "fat"            # Full link-time optimization, mandatory
codegen-units = 1      # Merge codegen units, improve runtime performance
strip = "symbols"      # Remove symbol table, reduce binary size
```

**Key points**: Large projects **must** set `lto = "fat"` and `codegen-units = 1`, performance improvement can exceed 20%.

### 5.2 Performance Analysis & Flame Graphs

```bash
# Performance sampling
sudo perf record -g -F 99 target/release/your_app
sudo perf script | stackcollapse-perf | flamegraph.pl > flame.svg

# Rust native tool
cargo install flamegraph
cargo flamegraph --bin your_app
```

**Key points**: Performance optimization **must not rely on guessing**. Every major change must include before/after flame graph comparison.

## 6. Observability & Operations

### 6.1 Structured Logging (Deprecate `log`)

**Mandatory**: Use `tracing` instead of `log`.

```rust
use tracing::{info, error, instrument};

#[instrument(skip(password))]
pub fn login(username: &str, password: &str) -> CoreResult<()> {
    info!("User login attempt");  // Automatically attaches span fields (username)
    // ...
}
```

**Key points**:
- Every async entry function must have `#[instrument]`, automatically trace call chain
- Log level: `ERROR` only for incidents requiring on-call, business exceptions use `WARN`

### 6.2 Metrics & Health Checks

- **Metrics exposure**: Use `metrics` + `prometheus`, export QPS, latency, error rate, memory
- **Liveness/Readiness probes**: Implement `GET /health` and `GET /ready`, required for K8s
- **Panic handling**: `std::panic::set_hook`, write panic info to separate file and report

## 7. Code Review Checklist

Every PR must self-check:

- [ ] Did you introduce `unwrap()`? Is there sufficient justification for unrecoverable error?
- [ ] Did you add unit tests/integration tests?
- [ ] Did performance-sensitive code include benchmark data?
- [ ] Was `unsafe` reviewed by 2+ people with `// SAFETY:` comments?
- [ ] Were dependencies checked with `cargo deny`?

## Quick Reference

| Dimension | Core Tools/Patterns | Key Command |
|-----------|---------------------|-------------|
| Project Management | Cargo Workspace | `cargo new --lib crates/xxx` |
| Error Handling | `thiserror` + `anyhow` | `#[derive(Error)]` |
| Async Runtime | `tokio` | `#[tokio::main]` |
| Structured Concurrency | `CancellationToken` | `token.cancel()` |
| Parallel Computing | `rayon` / `spawn_blocking` | `.par_iter()` |
| Dependency Security | `cargo deny` / `cargo audit` | `cargo deny check` |
| Performance Analysis | `flamegraph` / `criterion` | `cargo flamegraph` |
| Unsafe Detection | `miri` | `cargo miri test` |
| Logging & Tracing | `tracing` | `#[instrument]` |