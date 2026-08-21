# Zero-Copy SQL Query Path for remdb

**Date:** 2026-08-21
**Status:** Draft

## Motivation

The current SQL SELECT query path performs multiple unnecessary copies of record data:

1. Each record's raw bytes (`&[u8]`) are converted to `Vec<TypedValue>` (one allocation per field per row)
2. Matching rows are collected in `Vec<Vec<TypedValue>>` (double indirection, fragmentation)
3. The final `ResultSet` stores yet another copy of the row data

For a database targeting `no_std` / resource-constrained systems, this overhead is significant — both in memory pressure and query throughput.

## Design: Hybrid Approach

Zero-copy **during query execution** (filtering, sorting, grouping), compact bulk-copy **for the result set**.

### New Types

#### `RawRecordView<'a>` — Zero-copy record access during query execution

```rust
/// A borrowed view into a single record in table storage.
/// Created during table iteration; valid only while the table is unmodified.
pub struct RawRecordView<'a> {
    data: &'a [u8],
    table_def: &'a TableDef,
}

impl<'a> RawRecordView<'a> {
    /// Read a typed field directly from raw bytes — no Value enum allocation.
    pub fn read_u8(&self, field_idx: usize) -> Result<u8>;
    pub fn read_u16(&self, field_idx: usize) -> Result<u16>;
    pub fn read_u32(&self, field_idx: usize) -> Result<u32>;
    pub fn read_u64(&self, field_idx: usize) -> Result<u64>;
    pub fn read_i8(&self, field_idx: usize) -> Result<i8>;
    pub fn read_i16(&self, field_idx: usize) -> Result<i16>;
    pub fn read_i32(&self, field_idx: usize) -> Result<i32>;
    pub fn read_i64(&self, field_idx: usize) -> Result<i64>;
    pub fn read_f32(&self, field_idx: usize) -> Result<f32>;
    pub fn read_f64(&self, field_idx: usize) -> Result<f64>;
    pub fn read_bool(&self, field_idx: usize) -> Result<bool>;
    /// Zero-copy: returns a slice into the table's storage.
    pub fn read_str(&self, field_idx: usize) -> Result<&'a str>;
    pub fn read_timestamp(&self, field_idx: usize) -> Result<db_timestamp>;
    pub fn read_interval(&self, field_idx: usize) -> Result<db_interval>;

    /// Raw byte access (zero-copy, for any type).
    pub fn read_raw(&self, field_idx: usize) -> Result<&'a [u8]>;

    /// Fallback: create a TypedValue enum (copies data).
    pub fn to_typed_value(&self, field_idx: usize) -> Result<TypedValue>;
}
```

#### `CompactResultSet` — Owned result set with compact storage

```rust
/// A result set storing matching records as compact raw bytes.
/// Provides typed accessors and backwards-compatible TypedValue access.
pub struct CompactResultSet {
    columns: Vec<ColumnInfo>,
    raw_data: Vec<u8>,   // all matching records, row-major contiguous
    record_size: usize,
    record_count: usize,
}

pub struct ColumnInfo {
    name: String,
    offset: usize,       // byte offset within the record
    data_type: DataType,
    size: usize,         // field size in bytes
}

impl CompactResultSet {
    /// Returns a byte slice into the result set's own storage (lifetime of &self).
    pub fn get_field_raw(&self, row: usize, col: usize) -> Result<&[u8]>;

    /// Typed accessors (no Value enum allocation).
    pub fn get_field_u8(&self, row: usize, col: usize) -> Result<u8>;
    pub fn get_field_u16(&self, row: usize, col: usize) -> Result<u16>;
    pub fn get_field_u32(&self, row: usize, col: usize) -> Result<u32>;
    pub fn get_field_u64(&self, row: usize, col: usize) -> Result<u64>;
    pub fn get_field_f32(&self, row: usize, col: usize) -> Result<f32>;
    pub fn get_field_f64(&self, row: usize, col: usize) -> Result<f64>;
    pub fn get_field_bool(&self, row: usize, col: usize) -> Result<bool>;
    pub fn get_field_str(&self, row: usize, col: usize) -> Result<&str>;
    pub fn get_field_timestamp(&self, row: usize, col: usize) -> Result<db_timestamp>;
    pub fn get_field_interval(&self, row: usize, col: usize) -> Result<db_interval>;

    /// Backwards-compatible: create TypedValue on demand.
    pub fn get_field_typed(&self, row: usize, col: usize) -> Result<TypedValue>;
    pub fn get_row(&self, row: usize) -> Result<Vec<TypedValue>>;

    pub fn column_count(&self) -> usize;
    pub fn row_count(&self) -> usize;
    pub fn columns(&self) -> &[ColumnInfo];
    pub fn iter(&self) -> CompactResultSetIter<'_>;
}
```

### Data Flow (Before vs After)

#### Before (current)

```
table.iterate() → &[u8] per record
  → for each record: evaluate_condition() → read field bytes → create TypedValue
  → if match: push Vec<TypedValue> to matched_rows: Vec<Vec<TypedValue>>
  → sort: iterate matched_rows, extract sort keys from TypedValue rows
  → limit: truncate
  → for each result row: copy into ResultSet (another Vec<TypedValue>)
```

**Allocations per scan:** N rows × M fields × 1 `TypedValue` + N `Vec<TypedValue>` + 1 `Vec<Vec<TypedValue>>`

#### After (proposed)

```
table.iterate() → &[u8] per record → RawRecordView
  → evaluate condition: read raw bytes directly via RawRecordView::read_u32() etc.
    (no TypedValue allocation for condition evaluation)
  → if match: record record_id (store usize, not Vec<TypedValue>)
  → collect matching record_ids in Vec<usize>
  → sort: extract sort keys from raw bytes via RawRecordView::read_*()
    (no TypedValue allocation for sort keys)
  → limit: truncate ID list
  → bulk-copy matching records: raw_data.extend_from_slice(&table.data[start..end])
  → return CompactResultSet { raw_data, record_size, record_count, columns }
```

**Allocations per scan:** 1 `Vec<usize>` (matching IDs) + 1 `Vec<u8>` (final data)

### Conditional Evaluation (Zero-Copy)

The WHERE clause evaluator gets a new code path that operates on `RawRecordView`:

```rust
fn evaluate_condition_raw(
    record: &RawRecordView,
    condition: &Condition,
    table_def: &TableDef,
) -> bool;
```

For each comparison operator (`=`, `!=`, `<`, `>`, `<=`, `>=`, `BETWEEN`, `IN`, `LIKE`):
- Read the field value directly from raw bytes using the typed accessor
- Compare directly — no `TypedValue` enum created
- For `LIKE` and string operations, the `read_str()` method returns `&str` directly

### Sorting (Zero-Copy Sort Keys)

Sorting uses a pair of `(sort_key_bytes, record_id)` instead of `(TypedValue, usize)`:

```rust
// During iteration, collect only what's needed for sorting
fn collect_sort_keys<'a>(
    record: &RawRecordView<'a>,
    order_by: &[OrderByClause],
) -> Vec<u8> {  // raw sort key bytes
    // extract only the ORDER BY fields as raw bytes
}

// Sort by comparing raw key bytes
fn compare_sort_keys(a: &[u8], b: &[u8], order_by: &[OrderByClause]) -> Ordering;
```

### Aggregation (Zero-Copy Accumulation)

Aggregation accumulates directly from `RawRecordView`:

```rust
fn aggregate_raw(
    records: &[usize],           // matching record IDs
    table: &MemoryTable,
    functions: &[AggregateExpr],
) -> Vec<TypedValue>;           // final aggregate values (inevitable copy)
```

Only the final aggregate values are allocated as `TypedValue` — during accumulation, we operate on raw bytes.

### Backward Compatibility

The existing `ResultSet` type is **deprecated** but kept as a thin wrapper:

```rust
#[deprecated(since = "0.2.0", note = "use CompactResultSet instead")]
pub struct ResultSet {
    inner: CompactResultSet,
}

impl ResultSet {
    pub fn new(columns: Vec<String>) -> Self { ... }
    pub fn add_row(&mut self, values: Vec<TypedValue>) { ... }
    // delegates to inner for reading
}
```

The `execute_query` function returns `CompactResultSet` directly. Existing callers using `ResultSet` will get deprecation warnings.

### File Changes

| File | Change |
|------|--------|
| `src/sql/result_set.rs` | Add `CompactResultSet` + `ColumnInfo`; deprecate old `ResultSet` |
| `src/table.rs` | Add `RawRecordView` struct and factory method on `MemoryTable` |
| `src/sql/query_executor.rs` | Rewrite SELECT path to use `RawRecordView` + `CompactResultSet`; add `evaluate_condition_raw()` |
| `src/sql/mod.rs` | Update exports |
| `src/lib.rs` | Update re-exports |
| `tests/sql_query_test.rs` | Add tests for new zero-copy accessors |
| `tests/sql_parse_test.rs` | Update if query types change |

### Test Plan

1. **Unit: `RawRecordView` field accessors** — For every `DataType`, verify that `read_u32()` etc. returns the same value as the existing `get_field() → TypedValue → as_u32()` chain.

2. **Unit: `evaluate_condition_raw()`** — Run the same conditions through both the old TypedValue-based evaluator and the new raw evaluator; assert identical results.

3. **Unit: `CompactResultSet`** — Verify all typed accessors, `get_field_typed()`, `get_row()`, edge cases (empty set, single row, many rows).

4. **Integration: Full SELECT queries** — Run all existing SELECT test cases through the new executor path; compare `CompactResultSet` output with expected `ResultSet` output.

5. **Performance benchmark** — `cargo bench` comparing:
   - Query throughput (queries/second)
   - Memory allocations per query (count)
   - Memory allocated per query (bytes)
   - For varying result set sizes (1, 10, 100, 1000 rows)

6. **Edge cases**:
   - NULL / zero-length fields
   - Strings at MAX_STRING_LEN boundary
   - Floating point NaN handling
   - ORDER BY on multiple columns
   - GROUP BY with aggregation
   - JOIN queries (primary focus: one-table SELECT)
   - Empty result sets
   - WHERE with AND/OR/NOT combinations

### Scope Boundaries

- **JOIN queries are out of scope** for this phase. The current JOIN implementation (in `execute_select_join_query`) creates its own intermediate row representations and would need a separate pass. JOIN continues to use the old path until a follow-up.
- **INSERT, UPDATE, DELETE** paths are unchanged (they already work on raw bytes directly).
- **Time-series tables** (`execute_select_timeseries_query`) are out of scope for this phase — the time-series path is a separate executor with its own data flow.
- **WAL and transaction logging** are unchanged.

### Migration Path

1. Add `CompactResultSet` and `RawRecordView` (new types, no breakage)
2. Add `evaluate_condition_raw()` (new functions, no breakage)
3. Rewrite `execute_select_query()` to use the new path, returning `CompactResultSet`
4. Deprecate `ResultSet` but keep it as a wrapper
5. Update `execute_query()` return type to `CompactResultSet`
6. Fix all internal callers
7. Run full test suite
8. Remove deprecated `ResultSet` in a future release

### Open Questions

- Should `CompactResultSet` implement `IntoIterator`?
- JOIN and time-series zero-copy: follow-up phases.