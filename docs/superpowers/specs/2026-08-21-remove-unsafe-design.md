# Remove `unsafe` from remdb — Design Spec

Date: 2026-08-21
Status: Draft

## Goal

Eliminate every `unsafe` keyword from the `remdb` library crate. Achieve `#![forbid(unsafe_code)]` on the main crate. Inherently unsafe code (C FFI, platform syscalls, no_std global allocator) moves to satellite crates, each with `#![allow(unsafe_code)]` and a documented justification.

## Scope

Every module in `src/`. The `remdb-macros` proc-macro crate is not in scope (it generates Rust code, and its single `unwrap()` call is in a proc-macro error-reporting path where panicking is standard).

## Satellite Crates

Four new crates under `crates/` in the workspace. Each carries `#![allow(unsafe_code)]` and a module-level doc comment explaining why.

### `crates/remdb-c-api`

- **Source:** `src/c_api.rs` moved verbatim
- **Deps:** `remdb` (main crate), `libc`
- **Unsafe justification:** `extern "C"` functions are inherently unsafe — C callers don't follow Rust's safety guarantees. The `remdb` crate drops the `c-api` feature.
- **Exports:** `extern "C" fn remdb_init()`, `remdb_insert()`, `remdb_query()`, etc.

### `crates/remdb-alloc`

- **Source:** `src/memory/allocator.rs` — the `GlobalAllocator` + `StaticAllocator` + no_std `Mutex`
- **Deps:** none (no_std-compatible)
- **Unsafe justification:** `unsafe impl GlobalAlloc` is required by the trait. The no_std `Mutex` uses `UnsafeCell` (unavoidable in no_std without `parking_lot`).
- **Exports:** `StaticAllocator`, `init_global_allocator()`, `GlobalAllocator`
- **Usage:** no_std users add `remdb-alloc` and annotate `#[global_allocator]`. The main crate no longer provides a global allocator.

### `crates/remdb-platform-posix`

- **Source:** `src/platform/posix.rs` moved
- **Deps:** `remdb` (for the `Platform` trait), `libc`
- **Unsafe justification:** POSIX syscall wrappers require raw pointer I/O (`read()`/`write()` take `*const c_void`).
- **Exports:** `PosixPlatform`, `get_posix_platform()`

### `crates/remdb-platform-baremetal`

- **Source:** `src/platform/baremetal.rs` moved
- **Deps:** `remdb` (for the `Platform` trait)
- **Unsafe justification:** Stub implementations for bare-metal targets use raw pointer memcpy/memset.
- **Exports:** `BaremetalPlatform`, `get_baremetal_platform()`

## Main Crate Transformations

### T1: Platform trait — safe signatures

**File: `src/platform/mod.rs`**

The `Platform` trait method signatures change from raw pointers to slices:

| Current | New |
|---------|-----|
| `fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize)` | `fn memcpy(&self, dest: &mut [u8], src: &[u8])` |
| `fn file_write(&self, h: FileHandle, buf: *const u8, sz: usize)` | `fn file_write(&self, h: FileHandle, buf: &[u8])` |
| `fn file_read(&self, h: FileHandle, buf: *mut u8, sz: usize)` | `fn file_read(&self, h: FileHandle, buf: &mut [u8])` |
| `fn crc32(&self, data: *const u8, size: usize) -> u32` | `fn crc32(&self, data: &[u8]) -> u32` |
| `fn memset(&self, dest: *mut u8, value: u8, size: usize)` | `fn memset(&self, dest: &mut [u8], value: u8)` |

`FileHandle` changes from `*const u8` to `usize` (opaque handle). The satellite platform crates convert internally.

`spin_lock`/`spin_unlock` are removed from the trait. The main crate uses `parking_lot::SpinLock` instead.

The custom `OnceLock` (no_std) is replaced by `core::sync::OnceLock` (stable since 1.70). The `pub fn get_timestamp()` etc. wrapper functions become safe — they call through the trait which now has safe methods.

**Unsafe eliminated:** ~21 sites in `platform/`.

### T2: `MemoryTable` — raw pointers to slices

**File: `src/table.rs`**

```rust
// Before
pub struct MemoryTable {
    pub data_start: NonNull<u8>,
    pub status_array: NonNull<RecordHeader>,
    pub free_slots: NonNull<usize>,
    pub record_size: usize,
    ...
}

// After
pub struct MemoryTable {
    data: Box<[u8]>,
    status_array: Box<[RecordHeader]>,
    free_slots: Vec<usize>,
    record_size: usize,
    ...
}
```

Allocation in `MemoryTable::new()` changes from `allocator::alloc()` to `vec![0u8; size].into_boxed_slice()`. All subsequent access uses slice indexing:

- `fn slot_range(&self, slot: usize) -> Range<usize>` — computes `slot * record_size .. (slot+1) * record_size`
- `fn get_record(&self, slot: usize) -> &[u8]` — `&self.data[self.slot_range(slot)]`
- `fn get_record_mut(&mut self, slot: usize) -> &mut [u8]` — same, mutable
- `fn get_status(&self, slot: usize) -> &RecordHeader` — `&self.status_array[slot]`
- `fn get_status_mut(&mut self, slot: usize) -> &mut RecordHeader` — same, mutable

Methods that currently take `*const u8` change to `&[u8]`:
- `insert(&mut self, record_data: &[u8]) -> Result<usize>`
- `update(&mut self, id: usize, record_data: &[u8]) -> Result<()>`
- `get_by_id(&self, id: usize, dest: &mut [u8]) -> Result<()>`
- `validate_constraints(&self, record_data: &[u8]) -> Result<()>`
- `iterate<F>(&self, f: F)` where `F: FnMut(usize, &[u8]) -> bool`

`get_field` / `set_field` — read field values from `&[u8]` via `copy_from_slice` + `from_le_bytes` instead of raw pointer reads.

**Unsafe eliminated:** ~42 sites in `table.rs`.

**Downstream effect:** Every caller that currently gets a `*const u8` and reads fields now gets a `&[u8]` and uses safe `get_field()`. This eliminates ~152 unsafe sites in `sql/query_executor.rs` and ~30 in `ha/` and `pubsub/`.

### T3: `Value` union → safe enum

**File: `src/types.rs`**

```rust
#[derive(Copy, Clone)]
pub enum Value {
    U8(u8), U16(u16), U32(u32), U64(u64),
    Int8(i8), Int16(i16), Int32(i32), Int64(i64),
    Float32(f32), Float64(f64),
    Bool(bool),
    Timestamp(db_timestamp),
    Interval(db_interval),
    String([u8; MAX_STRING_LEN]),
}
```

`TypedValue` is removed — `Value` is self-describing via the variant. All `impl PartialEq`, `impl Hash`, `impl PartialOrd`, `impl Debug` for `Value` are derived or written safely (no union field access).

The `FieldDef::default_value` field changes from `Option<Value>` (union) to `Option<Value>` (enum) — same type name, different layout.

**Binary format breakage:** `Value` grows from 16 bytes to 17 bytes (1 discriminant byte + 16 data). Record layouts change. Snapshot files, WAL logs, and replication payloads from the old format are incompatible.

**Migration:** The `save_snapshot`/`restore_snapshot` code writes a new format version. The `SNAPSHOT_VERSION` constant is bumped from 1 to 2. The `LogItem` serialization is updated to match the new enum layout.

**Unsafe eliminated:** ~6 sites in `types.rs` + ~20 sites in `sql/query_executor.rs` (union field access: `value.u32`, `value.float64`, etc.) + downstream in `ha/` and `transaction.rs`.

### T4: Global state — `static mut` to `OnceLock<Mutex>`

**Files: `src/lib.rs`, `src/transaction.rs`, `src/pubsub/mod.rs`**

```rust
// Before
static mut DB_INSTANCE: Option<RemDb> = None;
pub static mut TX_MANAGER: TransactionManager = TransactionManager::new();
static mut PUB_SUB_INSTANCE: Option<PubSub> = None;

// After
use parking_lot::Mutex;

static DB_INSTANCE: OnceLock<Mutex<Option<RemDb>>> = OnceLock::new();
static TX_MANAGER: OnceLock<Mutex<TransactionManager>> = OnceLock::new();
static PUB_SUB_INSTANCE: OnceLock<Mutex<Option<PubSub>>> = OnceLock::new();
```

Access pattern:
```rust
// Before (unsafe)
let db = unsafe { DB_INSTANCE.as_mut().unwrap() };

// After (safe)
let mut guard = DB_INSTANCE.get_or_init(|| Mutex::new(None)).lock();
let db = guard.as_mut().unwrap();
```

`parking_lot::Mutex` is used because:
- Works on no_std with `alloc` (no `std::sync` dependency)
- No poison mechanism (no `.lock()` → `.unwrap()` needed)
- Smaller code size than `std::sync::Mutex`

**`unsafe impl Send` / `unsafe impl Sync`:** All are removed. `RemDb`, `TransactionManager`, `PubSub` no longer contain raw pointer fields — they hold `Box`, `Vec`, `parking_lot::Mutex`, and `OnceLock` types, all of which implement `Send`/`Sync` safely.

**`RemDb::begin_transaction()` etc.:** These are currently `pub unsafe fn`. They become safe — the `TX_MANAGER` access is through the `Mutex`.

**Unsafe eliminated:** ~39 sites in `lib.rs`, ~42 sites in `transaction.rs`, ~10 sites in `pubsub/mod.rs`.

### T5: Index nodes — raw pointers to `Box`

**File: `src/index.rs`**

```rust
// Before
pub struct BTreeNode {
    pub children: [Option<NonNull<BTreeNode>>; BTREE_ORDER + 1],
    ...
}
pub struct BTreeIndex {
    pub root: Option<NonNull<BTreeNode>>,
    pub nodes: NonNull<BTreeNode>,  // pre-allocated pool
    pub free_nodes: Option<NonNull<BTreeNode>>,
}

// After
pub struct BTreeNode {
    pub children: [Option<Box<BTreeNode>>; BTREE_ORDER + 1],
    ...
}
pub struct BTreeIndex {
    pub root: Option<Box<BTreeNode>>,
    // No pool, no free list — each node is Box::new(...)
}
```

The pre-allocated node pool (currently `nodes: NonNull<BTreeNode>`) is removed. Nodes are allocated individually via `Box::new()`. This is a small allocation overhead increase — for typical configs with `max_records` in the hundreds, the difference is negligible.

Same transformation applies to `TTreeIndex` and `PrimaryIndex` (hash table).

`PrimaryIndex` currently stores items in a raw pointer array. This becomes `Vec<Option<PrimaryIndexItem>>` or `Box<[Option<PrimaryIndexItem>]>`.

**Unsafe eliminated:** ~40 sites in `index.rs`.

### T6: `MemoryPool` — raw pointer linked list to `Vec`

**File: `src/memory/pool.rs`**

```rust
// Before
pub struct MemoryPool {
    start_ptr: NonNull<u8>,
    block_size: usize,
    total_blocks: usize,
    free_list: Option<NonNull<u8>>,  // intrusive linked list
}

// After
pub struct MemoryPool {
    storage: Vec<u8>,
    block_size: usize,
    free_indices: Vec<usize>,
}
```

- **Allocate:** `self.free_indices.pop()`, access via `&mut self.storage[idx * bs .. (idx+1) * bs]`
- **Free:** `self.free_indices.push(idx)`
- **No raw pointer arithmetic**

**Unsafe eliminated:** ~6 sites in `pool.rs`.

### T7: TTL ringbuffer — raw pointers to slices

**File: `src/pubsub/ttl_ringbuffer.rs`**

Same pattern as MemoryPool — raw pointer buffer (`self.buffer.add(slot_idx)`) becomes `&mut [u8]` slice indexing.

```rust
// Before
pub struct TtlRingBuffer {
    buffer: *mut u8,
    ...
}

// After
pub struct TtlRingBuffer {
    buffer: Box<[u8]>,
    ...
}
```

**Unsafe eliminated:** ~15 sites in `ttl_ringbuffer.rs`.

## Summary of unsafe eliminated

| Transformation | Direct unsafe eliminated | Downstream unsafe eliminated |
|---|---|---|
| T1: Platform trait safe signatures | ~21 | — |
| T2: MemoryTable slices | ~42 | ~152 (sql/) + ~30 (ha/) |
| T3: Value enum | ~6 | ~20 (sql/) + ~10 (ha/ + transaction/) |
| T4: Global state OnceLock+Mutex | ~91 | — |
| T5: Index Box nodes | ~40 | — |
| T6: MemoryPool Vec | ~6 | — |
| T7: TTL ringbuffer slices | ~15 | — |
| **Total** | **~221** | **~212** |

All ~433 unsafe sites eliminated. The main `remdb` crate gets `#![forbid(unsafe_code)]`.

## Satellite crate total unsafe

| Crate | Unsafe sites | Justification |
|---|---|---|
| `remdb-c-api` | ~5 | `extern "C"` fn required by C ABI |
| `remdb-alloc` | ~10 | `unsafe impl GlobalAlloc`, no_std `UnsafeCell` Mutex |
| `remdb-platform-posix` | ~15 | POSIX syscall raw pointer I/O |
| `remdb-platform-baremetal` | ~10 | Raw pointer memcpy/memset stubs |
| **Total** | **~40** | All confined, documented, reviewed |

## Module structure after the change

```
remdb (main crate, #![forbid(unsafe_code)])
├── src/
│   ├── lib.rs
│   ├── types.rs           # Value is now safe enum
│   ├── table.rs           # No raw pointers
│   ├── index.rs           # Box nodes, no raw pointers
│   ├── transaction.rs     # OnceLock<Mutex<TX_MANAGER>>
│   ├── config.rs          # Already safe (no changes needed)
│   ├── monitor.rs         # Already safe (no changes needed)
│   ├── sql/               # All downstream unsafe eliminated
│   ├── memory/
│   │   ├── mod.rs         # Only pool.rs remains (Vec-based)
│   │   └── pool.rs        # Vec, no raw pointers
│   ├── platform/
│   │   └── mod.rs         # Safe trait, OnceLock<dyn Platform>
│   ├── time_series/       # Downstream unsafe eliminated
│   ├── pubsub/            # OnceLock+Mutex, Vec-based ringbuffer
│   └── ha/                # Downstream unsafe eliminated

crates/
├── remdb-c-api/           # #![allow(unsafe_code)]
├── remdb-alloc/           # #![allow(unsafe_code)]
├── remdb-platform-posix/  # #![allow(unsafe_code)]
└── remdb-platform-baremetal/ # #![allow(unsafe_code)]
```

## Build and test

```bash
# Workspace still builds as a whole
cargo build --features "std posix pubsub ha"

# no_std users supply their own allocator
cargo build --no-default-features --features=baremetal

# Lib tests must pass
cargo test --lib

# Clippy must pass
cargo clippy --lib -D warnings
```

## Open questions

1. **`extern crate alloc` and `alloc::vec!` in no_std** — The main crate already uses `extern crate alloc`. The `Box<[u8]>` allocations in `MemoryTable::new()` use `alloc::vec!` and `into_boxed_slice()`, which work on no_std as long as a `#[global_allocator]` is provided. This is the standard pattern.

2. **`parking_lot` on no_std** — `parking_lot` version 0.12+ supports no_std with `alloc`. The `Cargo.toml` needs `parking_lot = { version = "0.12", default-features = false, features = ["alloc"] }`.

3. **`core::sync::OnceLock`** — Stable since Rust 1.70. We're on 1.95. No issues.

4. **Snapshot/WAL format version bump** — `SNAPSHOT_VERSION` goes from 1 to 2. Old snapshots are rejected with `SnapshotFormatError`. Users must re-create their database from scratch. This is acceptable for a 0.x → 1.0 release.