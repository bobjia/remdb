# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

remdb is an embedded in-memory database designed for resource-constrained embedded systems with `no_std` support. Key features include predictable memory usage, high performance, ACID transactions, SQL query support, time-series data, vector database capabilities, and high-availability replication.

## Build Commands

```bash
# Build the project
cargo build

# Build in release mode
cargo build --release

# Build for no_std (baremetal)
cargo build --no-default-features --features=baremetal

# Build with specific features
cargo build --features "pubsub ha"
```

## Testing Commands

```bash
# Run core library tests
cargo test --lib

# Run tests with specific features
cargo test --lib --features "pubsub ha"

# Run all tests
cargo test

# Run a single test
cargo test --lib test_name

# Check compilation for baremetal
cargo check --no-default-features --features=baremetal
```

## Linting & Formatting

```bash
# Format code
cargo fmt

# Run clippy (warnings as errors)
cargo clippy -- -D warnings

# Check all targets with all features
cargo clippy --all-targets --all-features -- -D warnings
```

## Running Examples

```bash
# API examples
cargo run --example basic_usage
cargo run --example sql_query
cargo run --example vector_example
cargo run --example time_series

# HA examples (requires ha feature)
cargo run --example test_remdb_server master sync
cargo run --example test_remdb_server slave sync <master_ip> <master_port>
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `std` | Standard library support |
| `posix` | POSIX platform support |
| `baremetal` | Bare metal/no_std support |
| `pubsub` | UDP-based pub/sub messaging |
| `ha` | High availability (master-slave replication), depends on `pubsub` |
| `log` | Logging support |
| `c-api` | C language API |
| `wal-compression-lz4` | LZ4 WAL compression |
| `wal-compression-zstd` | ZSTD WAL compression |

Default features: `std`, `posix`, `ha`, `pubsub`, `c-api`, `log`

## Architecture

### Core Components

**Memory Management (`src/memory/`)**
- Custom allocator for predictable memory usage
- Fixed-size block memory pool
- Supports both static and dynamic allocation
- Works without heap in baremetal mode

**Table Layer (`src/table.rs`)**
- `MemoryTable`: Core in-memory table with row storage
- `RecordRef`: Zero-copy record access
- Supports insert, delete, update, and scan operations
- Free slot management for O(1) insert

**Index System (`src/index.rs`, `src/index/`)**
- Primary index: Hash-based O(1) lookup
- Secondary indexes: BTree, TTree, Hash, SortedArray
- Vector indexes: HNSW, HNSW_SQ, HNSW_BQ, IVF, IVF_FLAT, IVF_PQ
- Thread-safe with spin locks

**Transaction System (`src/transaction.rs`)**
- ACID transaction support
- Begin/commit/rollback semantics
- Read-committed isolation level

**SQL Engine (`src/sql/`)**
- Parser (`query_parser.rs`): SQL parsing
- Executor (`query_executor.rs`): Query execution
- Operations (`operations/`): DDL, DML, SELECT, expression evaluation
- Functions (`functions/`): Aggregate, math, string, time, JSON functions

**Time Series (`src/time_series/`)**
- Specialized storage for time-ordered data
- Compression algorithms
- Partitioning and lifecycle management
- Pre-aggregation support

**High Availability (`src/ha/`)**
- Master-slave replication
- Sync and async replication modes
- Heartbeat-based failure detection
- Automatic failover

**Pub/Sub (`src/pubsub/`)**
- UDP-based reliable messaging
- NACK-based retransmission
- Supports unicast, broadcast, multicast

### Three Ways to Define Tables

1. **Macro-based (`remdb::table!`)**: Compile-time table definition for embedded scenarios
2. **Derive macro (`#[derive(MemdbTable)]`)**: DDL-based table generation from inline or file
3. **Runtime DDL (`DdlExecutor` trait)**: Dynamic table creation via `create_table()` or SQL

### Key Types

- `RemDb`: Main database instance
- `MemoryTable`: Table storage
- `Value`: Dynamic value type for SQL results
- `DataType`: Schema type definitions
- `FieldDef`: Field metadata
- `TableDef`: Table schema definition

## Important Patterns

### Error Handling

The codebase uses `Result<T, RemDbError>` for fallible operations. Avoid `.unwrap()` in library code; use `?` for error propagation.

### Memory Safety

- Uses `NonNull<u8>` for raw pointer handling
- Spin locks (`lock: u32`) for synchronization
- Platform abstraction via `src/platform/` for POSIX vs baremetal

### Testing

Tests use `serial_test` crate because many tests share global state. Test configuration is in `[tool.cargo.test]` with 16MB stack size and single-threaded execution.

### Conditional Compilation

Features gates are used extensively:
```rust
#[cfg(feature = "std")]
#[cfg(feature = "ha")]
#[cfg(feature = "log")]
```

## Common Tasks

### Adding a New SQL Function

1. Add implementation in `src/sql/functions/` (appropriate module)
2. Register in `src/sql/functions/mod.rs`
3. Add parser support in `query_parser.rs` if needed
4. Add tests in `tests/` directory

### Adding a New Index Type

1. Implement `SecondaryIndex` trait in `src/index.rs`
2. Add to `IndexType` enum in `src/types.rs`
3. Update index builder in `src/index/builder.rs`
4. Add tests

### Adding a New Data Type

1. Add to `DataType` enum in `src/types.rs`
2. Implement storage in `MemoryTable` (get/set methods)
3. Update SQL parser and executor
4. Add `Value` variant if needed

## Module Dependencies

```
lib.rs
├── config.rs      (configuration types)
├── types.rs       (core type definitions)
├── table.rs       (depends on types, index, platform)
├── index.rs       (depends on types, platform)
├── transaction.rs (depends on table)
├── sql/           (depends on table, types, index)
├── time_series/   (depends on table)
├── ha/            (depends on pubsub, transaction)
├── pubsub/        (depends on platform)
├── memory/        (standalone, platform-agnostic)
└── platform/      (platform abstraction)
```

## Release Profile

The project uses aggressive size optimization:
- `opt-level = "z"` (optimize for size)
- `lto = true`
- `codegen-units = 1`
- `panic = "abort"`