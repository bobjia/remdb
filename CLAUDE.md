# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

remdb is a lightweight embedded in-memory database for resource-constrained systems, supporting `no_std` environments. It provides memory tables, indexing (hash/BTree/TTree/SortedArray), ACID transactions with MVCC, WAL logging, snapshot/restore, SQL query support, time-series data, UDP-based pub/sub, and HA master-slave replication.

## Commands

### Build & Check

```bash
cargo build --features "std posix pubsub ha"   # default features
cargo check --tests --no-default-features       # no_std compilation check
cargo check --no-default-features --features=baremetal  # baremetal check
```

### Test

```bash
cargo test --lib                                                        # core lib tests only (recommended first)
cargo test --lib --features "pubsub ha"                                 # lib tests with optional features
cargo test [--features "pubsub ha"]                                     # full test suite (may fail on integration tests)
cargo test --test <test_name> -- [test_filter]                          # single integration test file
cargo test --lib <test_fn>                                              # single test function
cargo test --lib --features "pubsub ha" -- --test-threads=1             # serial execution (for flaky tests)
```

### Run Examples

```bash
cargo run --example basic_usage
cargo run --example sql_query
cargo run --example test_remdb_server master sync
cargo run --example test_remdb_server slave sync <master_ip> <master_port>
```

### Benchmarks

```bash
cargo bench
```

### Feature Flags

| Feature | Dependencies | Description |
|---------|-------------|-------------|
| `std` | `rand`, `socket2` | Standard library support |
| `posix` | `std` | POSIX platform support |
| `baremetal` | (none) | Bare-metal platform support |
| `pubsub` | `std` | UDP-based pub/sub |
| `ha` | `pubsub` | Master-slave replication |
| `c-api` | (none) | C API exports |
| `debug` | (none) | Debug mode |

## Architecture

### Crate Structure

The workspace has two crates:
- **`remdb`** (root): The database library
- **`remdb-macros`** (`remdb-macros/`): Proc-macro crate providing `table!`, `database!`, and `#[derive(MemdbTable)]` macros

### Core Modules

- **`src/types.rs`** — Core data types: `DataType` enum (UInt8–Float64, Bool, Timestamp, String, Interval), `Value` union, `FieldDef`, `TableDef`, `RecordHeader` (with MVCC fields), `RemDbError` enum, `RecordStatus`. The `db_timestamp` and `db_interval` structs implement a precision-aware time system (0–9 precision levels).

- **`src/table.rs`** — `MemoryTable`: fixed-size record storage with per-record `RecordHeader` (status, MVCC version info, lock state). Uses a free-slot stack for O(1) insertion. Supports iteration, update, deletion, and time-window queries.

- **`src/index.rs`** — Index implementations:
  - `PrimaryIndex`: Hash-based primary key index (O(1) lookup)
  - `AnySecondaryIndex` (enum over `SortedArray`, `BTree`, `TTree`, `Hash`): Secondary index types. BTree uses order 4, TTree uses order 3.

- **`src/transaction.rs`** — ACID transaction support with MVCC. `TransactionManager` (global `TX_MANAGER`) manages active transactions, isolation levels (ReadUncommitted through Serializable), and WAL logging. `LogManager` handles WAL file I/O with checkpointing.

- **`src/config.rs`** — `DbConfig` struct with all configuration (tables, memory, WAL, HA, pubsub, time-series defaults). `MemoryAllocator` trait for pluggable allocation. Compile-time config validation via `validate_config()`.

- **`src/lib.rs`** — `RemDb` struct (the main database handle), `DdlExecutor` trait (runtime DDL), `init_global_db()`, snapshot save/restore, incremental snapshot, SQL query entry point, low-power mode, and batch time-series writes.

- **`src/sql/`** — SQL support:
  - `query_parser.rs`: SQL parser (SELECT, INSERT, UPDATE, DELETE, CREATE TABLE/INDEX, CREATE TIMESERIES TABLE)
  - `query_executor.rs`: Query execution engine with aggregation (COUNT, SUM, AVG, MIN, MAX), time functions (TO_ISO8601, TO_CHAR, TO_EPOCH), math functions, JOIN support, ORDER BY, GROUP BY, LIMIT, WHERE
  - `result_set.rs`: Query result representation

- **`src/memory/`** — Memory management: `allocator.rs` (static memory allocator for `no_std`), `pool.rs` (fixed-size block memory pool)

- **`src/platform/`** — Platform abstraction layer: `mod.rs` (trait definitions), `posix.rs` (POSIX file I/O, timers), `baremetal.rs` (stub implementations)

- **`src/monitor.rs`** — `DbMetrics` and `HealthStatus` for runtime monitoring, with pubsub publishing of metrics/health events.

- **`src/pubsub/`** — UDP-based publish/subscribe with NACK-based retransmission, TTL ringbuffer, pre-defined topics (WAL, tables, metrics, health).

- **`src/ha/`** — High availability: `manager.rs` (HA state machine), `replication.rs` (WAL-based replication), `heartbeat.rs` (failure detection), `role.rs` (master/slave role management).

- **`src/time_series/`** — Time-series data support: `table.rs` (partitioned storage), `index.rs` (time-based indexing), `compression.rs` (delta, run-length, delta-delta), `partition.rs` (time-window partitioning), `lifecycle.rs` (TTL-based retention).

### Key Design Patterns

1. **Compile-time configuration via macros**: The `table!` and `database!` macros generate fixed-size table definitions at compile time, enabling `no_std` compatibility and predictable memory usage.

2. **Runtime DDL via `DdlExecutor` trait**: Tables and indexes can be created dynamically at runtime, using `Box::leak` to convert runtime strings to `'static` references.

3. **MVCC in `RecordHeader`**: Each record carries `create_tx_id`, `delete_tx_id`, and `next_version_ptr` for multi-version concurrency control, plus a spinlock for thread safety.

4. **WAL with checkpointing**: `LogManager` writes `LogItem` entries to WAL files with CRC32 checksums, supports replay for recovery, and periodic checkpointing.

5. **Platform abstraction**: All platform-dependent operations (file I/O, memory operations, timing) go through `crate::platform` function pointers, allowing swap between POSIX and bare-metal.

### Test Files

Tests are in `tests/` as integration tests. Key test files:
- `table_test.rs` — Core table operations
- `transaction_test.rs` — ACID transactions
- `sql_query_test.rs` — SQL parsing and execution
- `sql_parse_test.rs` — SQL parser edge cases
- `wal_test.rs` — WAL logging and recovery
- `ha_test.rs` — Master-slave replication
- `pubsub_test.rs` — Pub/sub messaging
- `time_series_*_test.rs` — Time-series operations
- `dynamic_ddl_test.rs` — Runtime DDL
- `memory_test.rs` — Memory allocator
- `c_api_tests.c` — C API tests

### `no_std` Compatibility

The crate uses `#![cfg_attr(not(feature = "std"), no_std)]` and the `extern crate alloc` pattern. String formatting uses `alloc::format!` instead of `std::format!`. The `remdb-macros` proc-macro crate is always `std`-dependent (proc macros require it), but the main library compiles without std.


## Lints — Enforced via Cargo.toml

These are configured in `[workspace.lints]` in `Cargo.toml`:

| Lint | Level | Scope |
|------|-------|-------|
| `unsafe_code` | `deny` (workspace), `allow` (crate) | Crate fundamentally requires unsafe for raw memory management |
| `clippy::unwrap_used` | `deny` (workspace), `allow` (crate) | Use `?`, `.unwrap_or()`, `.unwrap_or_else()`, or `match` |
| `clippy::expect_used` | `deny` (workspace), `allow` (crate) | Same as `unwrap_used` |
| `clippy::indexing_slicing` | `deny` (workspace), `allow` (crate) | No `arr[i]` or `slice[a..b]`; use `.get()` / `.get(..)` |

The crate-level `allow` is a temporary measure for the existing codebase. As code is refactored, narrow the `allow` to specific modules or remove it entirely. New modules should follow the rules without exemptions.

The `panic`, `todo`, and `unimplemented` rustc lints are not available in the current Rust toolchain.

