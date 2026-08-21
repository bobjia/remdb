# Zero-Copy SQL Query Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate unnecessary `Vec<TypedValue>` allocations per row during SQL SELECT query execution by using borrow-based `RawRecordView` for zero-copy field access and compact bulk-copy `CompactResultSet` for the result set.

**Architecture:** Hybrid approach — zero-copy during query execution (filtering, sorting, grouping via `RawRecordView<'a>` that borrows table storage), one bulk `memcpy` of matching records into a contiguous `Vec<u8>` for the owned `CompactResultSet`. The old `ResultSet` is deprecated as a wrapper.

**Tech Stack:** Rust `no_std`-compatible, `alloc` crate only (no `std` required for the core path).

**Spec:** `docs/designs/2026-08-21-zero-copy-query-path-design.md`

## Global Constraints

- Must compile under `#![cfg_attr(not(feature = "std"), no_std)]` — no `std::` dependency in the core data path
- No `unsafe` in new code (existing `#[allow(unsafe_code)]` is crate-level, but new modules should be clean)
- No `unwrap()` or `expect()` in new code (use `?` / `.unwrap_or()` / `match`)
- No `arr[i]` / `slice[a..b]` indexing without bounds check (use `.get()` / `.get(..)`)
- Existing `#[deny(unsafe_code)]` lint applies — use `#[allow(unsafe_code)]` only where absolutely necessary
- String fields are fixed-size `[u8; MAX_STRING_LEN]` (64 bytes), zero-padded
- All numeric types stored in little-endian byte order
- `field.offset` and `field.size` in `TableDef` define the byte layout of each record

---
### Task 1: `RawRecordView` — Zero-copy record access

**Files:**
- Create: `src/record_view.rs` (new module, focused file)
- Modify: `src/table.rs` (add factory method on `MemoryTable`)
- Modify: `src/lib.rs` (add module declaration, re-export)
- Test: `tests/record_view_test.rs` (new test file)

**Interfaces:**
- Consumes: `MemoryTable::get_record_slice(index) -> &[u8]`, `TableDef`, `FieldDef`, `DataType`, `MAX_STRING_LEN`, `RemDbError`
- Produces: `RawRecordView<'a>` struct with one typed accessor per data type, `to_typed_value()` fallback, `MemoryTable::get_record_view(&self, index) -> RawRecordView<'_>`

**Rationale:** A focused file with a single responsibility — interpreting raw record bytes. This avoids bloating `table.rs` (already 1988 lines) and makes the view logic independently testable.

- [ ] **Step 1: Create `src/record_view.rs` with the struct definition and typed accessors**

```rust
use crate::types::{DataType, FieldDef, MAX_STRING_LEN, RemDbError, Result, TableDef, Value, db_interval, db_timestamp};

/// A borrowed view into a single record's raw bytes in table storage.
/// Provides zero-copy typed field access without allocating Value enums.
pub struct RawRecordView<'a> {
    pub data: &'a [u8],
    pub table_def: &'a TableDef,
}

impl<'a> RawRecordView<'a> {
    /// Create a new record view from raw record bytes and table definition.
    pub fn new(data: &'a [u8], table_def: &'a TableDef) -> Self {
        RawRecordView { data, table_def }
    }

    /// Resolve a field name (handling `table.field` aliases) to a field index.
    fn resolve_field_index(&self, field_name: &str) -> Result<usize> {
        let actual_name = if let Some(dot_pos) = field_name.find('.') {
            &field_name[dot_pos + 1..]
        } else {
            field_name
        };
        self.table_def.fields
            .iter()
            .position(|f| f.name == actual_name)
            .ok_or(RemDbError::FieldNotFound)
    }

    fn get_field_def(&self, field_idx: usize) -> Result<&FieldDef> {
        self.table_def.fields.get(field_idx).ok_or(RemDbError::FieldNotFound)
    }

    // --- Typed read accessors ---

    pub fn read_u8(&self, field_idx: usize) -> Result<u8> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt8 { return Err(RemDbError::TypeMismatch); }
        self.data.get(field.offset).copied().ok_or(RemDbError::FieldNotFound)
    }

    pub fn read_u16(&self, field_idx: usize) -> Result<u16> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt16 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 2)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u16::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_u32(&self, field_idx: usize) -> Result<u32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt32 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u32::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_u64(&self, field_idx: usize) -> Result<u64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::UInt64 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(u64::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    // (follow the same pattern for i8, i16, i32, i64, f32, f64, bool)

    pub fn read_i8(&self, field_idx: usize) -> Result<i8> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int8 { return Err(RemDbError::TypeMismatch); }
        self.data.get(field.offset).map(|&b| b as i8).ok_or(RemDbError::FieldNotFound)
    }

    pub fn read_i16(&self, field_idx: usize) -> Result<i16> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int16 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 2)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i16::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_i32(&self, field_idx: usize) -> Result<i32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int32 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i32::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_i64(&self, field_idx: usize) -> Result<i64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Int64 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(i64::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_f32(&self, field_idx: usize) -> Result<f32> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Float32 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 4)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(f32::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_f64(&self, field_idx: usize) -> Result<f64> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Float64 { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        Ok(f64::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    pub fn read_bool(&self, field_idx: usize) -> Result<bool> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Bool { return Err(RemDbError::TypeMismatch); }
        self.data.get(field.offset).map(|&b| b != 0).ok_or(RemDbError::FieldNotFound)
    }

    /// Zero-copy: returns a &str slice into the raw table storage.
    /// The string is zero-padded; trailing null bytes are trimmed by returning a sub-slice.
    pub fn read_str(&self, field_idx: usize) -> Result<&'a str> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::String { return Err(RemDbError::TypeMismatch); }
        let slice = self.data.get(field.offset..field.offset + field.size)
            .ok_or(RemDbError::FieldNotFound)?;
        // Find the first null byte (zero-padding terminator)
        let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        let trimmed_slice = &slice[..end];
        core::str::from_utf8(trimmed_slice).map_err(|_| RemDbError::TypeMismatch)
    }

    pub fn read_timestamp(&self, field_idx: usize) -> Result<db_timestamp> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Timestamp && field.data_type != DataType::TimestampTZ {
            return Err(RemDbError::TypeMismatch);
        }
        let value_bytes = self.data.get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        let value = i64::from_le_bytes(value_bytes.try_into().map_err(|_| RemDbError::TypeMismatch)?);
        let tz_offset = if field.data_type == DataType::TimestampTZ {
            let tz_bytes = self.data.get(field.offset + 8..field.offset + 10)
                .ok_or(RemDbError::FieldNotFound)?;
            i16::from_le_bytes(tz_bytes.try_into().map_err(|_| RemDbError::TypeMismatch)?)
        } else {
            0
        };
        Ok(db_timestamp { value, tz_offset, precision: 0, flags: 0 })
    }

    pub fn read_interval(&self, field_idx: usize) -> Result<db_interval> {
        let field = self.get_field_def(field_idx)?;
        if field.data_type != DataType::Interval { return Err(RemDbError::TypeMismatch); }
        let value_bytes = self.data.get(field.offset..field.offset + 8)
            .ok_or(RemDbError::FieldNotFound)?;
        let value = i64::from_le_bytes(value_bytes.try_into().map_err(|_| RemDbError::TypeMismatch)?);
        Ok(db_interval { value, precision: 0, flags: 0 })
    }

    /// Raw byte access into the table storage (zero-copy).
    pub fn read_raw(&self, field_idx: usize) -> Result<&'a [u8]> {
        let field = self.get_field_def(field_idx)?;
        self.data.get(field.offset..field.offset + field.size)
            .ok_or(RemDbError::FieldNotFound)
    }

    /// Fallback: create a TypedValue (copies data).
    /// Uses the existing MemoryTable::get_field logic internally.
    pub fn to_typed_value(&self, field_idx: usize) -> Result<Value> {
        let field = self.get_field_def(field_idx)?;
        let offset = field.offset;
        let size = field.size;
        let data = self.data;
        match field.data_type {
            DataType::UInt8 => Ok(Value::U8(data.get(offset).copied().ok_or(RemDbError::FieldNotFound)?)),
            DataType::UInt16 => {
                let slice = data.get(offset..offset + 2).ok_or(RemDbError::FieldNotFound)?;
                Ok(Value::U16(u16::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?)))
            },
            // ... same pattern for all types (copy from existing get_field logic)
            DataType::String => {
                let mut buf = [0u8; MAX_STRING_LEN];
                let copy_size = core::cmp::min(size, MAX_STRING_LEN);
                let end = core::cmp::min(offset + copy_size, data.len());
                buf[..end - offset].copy_from_slice(&data[offset..end]);
                Ok(Value::String(buf))
            },
            DataType::Timestamp | DataType::TimestampTZ => {
                let slice = data.get(offset..offset + 8).ok_or(RemDbError::FieldNotFound)?;
                let value = i64::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?);
                Ok(Value::Time(db_timestamp { value, tz_offset: 0, precision: 0, flags: 0 }))
            },
            DataType::Interval => {
                let slice = data.get(offset..offset + 8).ok_or(RemDbError::FieldNotFound)?;
                let value = i64::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?);
                Ok(Value::Interval(db_interval { value, precision: 0, flags: 0 }))
            },
        }
    }
}
```

- [ ] **Step 2: Add factory method on `MemoryTable` in `src/table.rs`**

```rust
impl MemoryTable {
    /// Create a zero-copy record view for a given record index.
    pub fn get_record_view(&self, index: usize) -> Result<RawRecordView<'_>> {
        if index >= self.def.max_records {
            return Err(RemDbError::RecordNotFound);
        }
        if self.status_array[index].status != RecordStatus::Used {
            return Err(RemDbError::RecordNotFound);
        }
        let start = index * self.record_size;
        let end = start + self.record_size;
        let data = self.data.get(start..end).ok_or(RemDbError::RecordNotFound)?;
        Ok(RawRecordView::new(data, &self.def))
    }
}
```

- [ ] **Step 3: Add module declaration and re-export in `src/lib.rs`**

```rust
// Add module declaration (alphabetical order)
pub mod record_view;
// Add re-export
pub use record_view::RawRecordView;
```

- [ ] **Step 4: Write unit tests in `tests/record_view_test.rs`**

```rust
// For every DataType, write a test that:
// 1. Creates a table with a field of that type
// 2. Inserts a record with a known value
// 3. Reads the field via RawRecordView::read_*()
// 4. Asserts the value matches what was inserted
// 5. Also reads via MemoryTable::get_field() and asserts both match

// Key test cases:
// - read_u8, read_u16, read_u32, read_u64
// - read_i8, read_i16, read_i32, read_i64
// - read_f32, read_f64
// - read_bool
// - read_str (zero-padded, max length, empty string)
// - read_timestamp, read_interval
// - read_raw
// - to_typed_value (backwards compatibility)
// - Error cases: wrong type, wrong index, table.field aliases
```

- [ ] **Step 5: Run tests and commit**

```bash
cargo test --lib --test record_view_test -- --test-threads=1
git add src/record_view.rs src/table.rs src/lib.rs tests/record_view_test.rs
git commit -m "feat: add RawRecordView for zero-copy record access"
```

---
### Task 2: `CompactResultSet` — Compact owned result set

**Files:**
- Modify: `src/sql/result_set.rs` (add `CompactResultSet` + `ColumnInfo`, deprecate old `ResultSet`)
- Modify: `src/sql/mod.rs` (update exports)
- Modify: `src/lib.rs` (update re-exports if needed)
- Test: `tests/compact_result_set_test.rs`

**Interfaces:**
- Consumes: `DataType`, `MAX_STRING_LEN`, `Value`, `TypedValue`, `RemDbError`, `Result`
- Produces: `CompactResultSet` struct with typed accessors, `get_field_typed()`, `get_row()`, `iter()`, `to_string()`

- [ ] **Step 1: Add `ColumnInfo` struct and `CompactResultSet` to `src/sql/result_set.rs`**

```rust
/// Column metadata for interpreting CompactResultSet raw_data.
#[derive(Clone, Debug)]
pub struct ColumnInfo {
    pub name: alloc::string::String,
    pub offset: usize,       // byte offset within the record
    pub data_type: DataType,
    pub size: usize,         // field size in bytes
}

/// A result set storing matching records as compact raw bytes.
/// Provides typed accessors and backwards-compatible TypedValue access.
pub struct CompactResultSet {
    pub columns: alloc::vec::Vec<ColumnInfo>,
    pub raw_data: alloc::vec::Vec<u8>,   // all matching records, row-major contiguous
    pub record_size: usize,
    pub record_count: usize,
}

impl CompactResultSet {
    /// Create a new empty CompactResultSet from column metadata.
    pub fn new(columns: alloc::vec::Vec<ColumnInfo>, record_size: usize) -> Self {
        CompactResultSet {
            columns,
            raw_data: alloc::vec::Vec::new(),
            record_size,
            record_count: 0,
        }
    }

    /// Add a record by copying its raw bytes.
    pub fn add_record(&mut self, record_data: &[u8]) -> Result<()> {
        let copy_len = core::cmp::min(record_data.len(), self.record_size);
        self.raw_data.extend_from_slice(&record_data[..copy_len]);
        self.record_count += 1;
        Ok(())
    }

    /// Get the raw data slice for a given row.
    fn get_row_slice(&self, row: usize) -> Result<&[u8]> {
        if row >= self.record_count {
            return Err(RemDbError::RecordNotFound);
        }
        let start = row * self.record_size;
        let end = start + self.record_size;
        self.raw_data.get(start..end).ok_or(RemDbError::RecordNotFound)
    }

    /// Get a specific column's byte range within a row.
    fn get_col_slice(&self, row: usize, col: usize) -> Result<&[u8]> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let row_slice = self.get_row_slice(row)?;
        row_slice.get(col_info.offset..col_info.offset + col_info.size)
            .ok_or(RemDbError::FieldNotFound)
    }

    // --- Typed accessors ---

    pub fn get_field_u8(&self, row: usize, col: usize) -> Result<u8> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt8 { return Err(RemDbError::TypeMismatch); }
        let slice = self.get_col_slice(row, col)?;
        slice.first().copied().ok_or(RemDbError::FieldNotFound)
    }

    pub fn get_field_u16(&self, row: usize, col: usize) -> Result<u16> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::UInt16 { return Err(RemDbError::TypeMismatch); }
        let slice = self.get_col_slice(row, col)?;
        Ok(u16::from_le_bytes(slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))
    }

    // ... (same pattern for u32, u64, i8, i16, i32, i64, f32, f64, bool, str, timestamp, interval)

    pub fn get_field_str(&self, row: usize, col: usize) -> Result<&str> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        if col_info.data_type != DataType::String { return Err(RemDbError::TypeMismatch); }
        let slice = self.get_col_slice(row, col)?;
        let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        let trimmed = &slice[..end];
        core::str::from_utf8(trimmed).map_err(|_| RemDbError::TypeMismatch)
    }

    /// Raw byte access (borrows from CompactResultSet's own storage).
    pub fn get_field_raw(&self, row: usize, col: usize) -> Result<&[u8]> {
        self.get_col_slice(row, col)
    }

    /// Backwards-compatible: create TypedValue on demand.
    pub fn get_field_typed(&self, row: usize, col: usize) -> Result<TypedValue> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let value = self.get_field_value(row, col)?;
        Ok(TypedValue {
            value_type: col_info.data_type,
            value,
        })
    }

    /// Helper: read a field as Value enum from the raw data.
    fn get_field_value(&self, row: usize, col: usize) -> Result<Value> {
        let col_info = self.columns.get(col).ok_or(RemDbError::FieldNotFound)?;
        let slice = self.get_col_slice(row, col)?;
        match col_info.data_type {
            DataType::UInt8 => Ok(Value::U8(slice[0])),
            DataType::UInt16 => Ok(Value::U16(u16::from_le_bytes(
                slice.try_into().map_err(|_| RemDbError::TypeMismatch)?))),
            // ... same pattern for all types
            DataType::String => {
                let mut buf = [0u8; MAX_STRING_LEN];
                let copy_size = core::cmp::min(slice.len(), MAX_STRING_LEN);
                buf[..copy_size].copy_from_slice(&slice[..copy_size]);
                Ok(Value::String(buf))
            },
            _ => Err(RemDbError::TypeMismatch),
        }
    }

    /// Backwards-compatible: return a Vec<TypedValue> for a row.
    pub fn get_row(&self, row: usize) -> Result<alloc::vec::Vec<TypedValue>> {
        let mut result = alloc::vec::Vec::with_capacity(self.columns.len());
        for col in 0..self.columns.len() {
            result.push(self.get_field_typed(row, col)?);
        }
        Ok(result)
    }

    pub fn column_count(&self) -> usize { self.columns.len() }
    pub fn row_count(&self) -> usize { self.record_count }
    pub fn columns(&self) -> &[ColumnInfo] { &self.columns }

    /// Format the result set as a string (same format as the old ResultSet).
    pub fn to_string(&self) -> alloc::string::String {
        if self.record_count == 0 {
            return "Empty result set".to_string();
        }
        let mut result = alloc::string::String::new();
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&col.name);
        }
        result.push('\n');
        for (i, _) in self.columns.iter().enumerate() {
            if i > 0 { result.push_str("--+"); }
            result.push_str("----");
        }
        result.push('\n');
        for row in 0..self.record_count {
            for col in 0..self.columns.len() {
                if col > 0 { result.push_str(", "); }
                if let Ok(tv) = self.get_field_typed(row, col) {
                    result.push_str(&value_to_string_repr(&tv.value));
                }
            }
            result.push('\n');
        }
        result
    }
}
```

- [ ] **Step 2: Deprecate the old `ResultSet` as a wrapper around `CompactResultSet`**

```rust
#[deprecated(since = "0.2.0", note = "use CompactResultSet instead")]
pub struct ResultSet {
    inner: CompactResultSet,
}

impl ResultSet {
    pub fn new(columns: Vec<String>) -> Self {
        let col_info = columns.into_iter().map(|name| ColumnInfo {
            name,
            offset: 0,
            data_type: DataType::UInt64,
            size: 8,
        }).collect();
        ResultSet {
            inner: CompactResultSet::new(col_info, 8),
        }
    }

    pub fn add_row(&mut self, values: Vec<TypedValue>) {
        // Convert values to raw bytes and add
        let mut record = alloc::vec![0u8; 8 * values.len()];
        // ... pack values into record
        let _ = self.inner.add_record(&record);
    }

    // Delegate to inner for reading
    pub fn column_count(&self) -> usize { self.inner.column_count() }
    pub fn row_count(&self) -> usize { self.inner.row_count() }
    pub fn columns(&self) -> &Vec<String> { unimplemented!("deprecated") }
    pub fn get_row(&self, index: usize) -> Option<&ResultRow> { unimplemented!("deprecated") }
    pub fn to_string(&self) -> String { self.inner.to_string() }
}
```

- [ ] **Step 3: Update `src/sql/mod.rs` exports**

```rust
pub use result_set::{CompactResultSet, ColumnInfo, ResultSet, ResultRow, ResultRowIter};
```

- [ ] **Step 4: Write unit tests in `tests/compact_result_set_test.rs`**

```rust
// Test cases:
// - Empty result set
// - Single row, single column
// - Multiple rows, multiple columns
// - All typed accessors (u8 through interval)
// - get_field_str (strings, zero-padded, empty)
// - get_field_typed (backwards compatibility)
// - get_row (returns Vec<TypedValue>)
// - to_string formatting
// - Error cases: out of bounds row, out of bounds col
```

- [ ] **Step 5: Run tests and commit**

```bash
cargo test --lib --test compact_result_set_test -- --test-threads=1
git add src/sql/result_set.rs src/sql/mod.rs
git commit -m "feat: add CompactResultSet with typed accessors, deprecate old ResultSet"
```

---
### Task 3: `evaluate_condition_raw()` — Zero-copy WHERE clause evaluation

**Files:**
- Modify: `src/sql/query_executor.rs` (add `evaluate_condition_raw()` and helpers)
- Test: `tests/sql_query_test.rs` (add condition evaluation tests)

**Interfaces:**
- Consumes: `RawRecordView`, `Condition`, `ComparisonCondition`, `BetweenCondition`, `ComparisonOperator`, `crate::sql::Value`
- Produces: `evaluate_condition_raw(RawRecordView, &Condition) -> bool`

- [ ] **Step 1: Add `evaluate_condition_raw()` and helpers to `src/sql/query_executor.rs`**

```rust
/// Evaluate a WHERE condition directly on raw record bytes (zero-copy).
/// Returns true if the record matches the condition.
fn evaluate_condition_raw(
    record: &RawRecordView,
    condition: &Condition,
    table_def: &TableDef,
) -> bool {
    match condition {
        Condition::Comparison(comp) => evaluate_comparison_raw(record, comp, table_def),
        Condition::Between(between) => evaluate_between_raw(record, between, table_def),
        Condition::And(left, right) => {
            evaluate_condition_raw(record, left, table_def) &&
            evaluate_condition_raw(record, right, table_def)
        },
        Condition::Or(left, right) => {
            evaluate_condition_raw(record, left, table_def) ||
            evaluate_condition_raw(record, right, table_def)
        },
    }
}

/// Evaluate a comparison condition on raw bytes.
fn evaluate_comparison_raw(
    record: &RawRecordView,
    comp: &ComparisonCondition,
    table_def: &TableDef,
) -> bool {
    let field_idx = match resolve_field_index_for_condition(&comp.field, table_def) {
        Some(idx) => idx,
        None => return false,
    };
    let field_type = table_def.fields[field_idx].data_type;
    match record.read_raw(field_idx) {
        Ok(raw_bytes) => {
            compare_raw_with_condition(raw_bytes, field_type, &comp.operator, &comp.value)
        },
        Err(_) => false,
    }
}

/// Evaluate a BETWEEN condition on raw bytes.
fn evaluate_between_raw(
    record: &RawRecordView,
    between: &BetweenCondition,
    table_def: &TableDef,
) -> bool {
    let field_idx = match resolve_field_index_for_condition(&between.field, table_def) {
        Some(idx) => idx,
        None => return false,
    };
    let field_type = table_def.fields[field_idx].data_type;
    match record.read_raw(field_idx) {
        Ok(raw_bytes) => {
            let ge = compare_raw_with_condition(
                raw_bytes, field_type, &ComparisonOperator::GreaterThanOrEqual, &between.min_value);
            let le = compare_raw_with_condition(
                raw_bytes, field_type, &ComparisonOperator::LessThanOrEqual, &between.max_value);
            ge && le
        },
        Err(_) => false,
    }
}

/// Resolve a field name (handling table.field aliases) to a field index.
fn resolve_field_index_for_condition(field_name: &str, table_def: &TableDef) -> Option<usize> {
    let actual_name = if let Some(dot_pos) = field_name.find('.') {
        &field_name[dot_pos + 1..]
    } else {
        field_name
    };
    table_def.fields.iter().position(|f| f.name == actual_name)
}

/// Compare raw field bytes with a condition value (no TypedValue allocation).
fn compare_raw_with_condition(
    raw_bytes: &[u8],
    field_type: DataType,
    operator: &ComparisonOperator,
    condition_value: &crate::sql::Value,
) -> bool {
    match field_type {
        DataType::UInt8 => {
            if raw_bytes.is_empty() { return false; }
            let f_val = raw_bytes[0];
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as u8, operator),
                _ => false,
            }
        },
        DataType::UInt16 => {
            if raw_bytes.len() < 2 { return false; }
            let f_val = u16::from_le_bytes([raw_bytes[0], raw_bytes[1]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as u16, operator),
                _ => false,
            }
        },
        DataType::UInt32 => {
            if raw_bytes.len() < 4 { return false; }
            let f_val = u32::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as u32, operator),
                _ => false,
            }
        },
        DataType::UInt64 => {
            if raw_bytes.len() < 8 { return false; }
            let f_val = u64::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3],
                 raw_bytes[4], raw_bytes[5], raw_bytes[6], raw_bytes[7]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as u64, operator),
                _ => false,
            }
        },
        DataType::Int8 => {
            if raw_bytes.is_empty() { return false; }
            let f_val = raw_bytes[0] as i8;
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as i8, operator),
                _ => false,
            }
        },
        DataType::Int16 => {
            if raw_bytes.len() < 2 { return false; }
            let f_val = i16::from_le_bytes([raw_bytes[0], raw_bytes[1]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as i16, operator),
                _ => false,
            }
        },
        DataType::Int32 => {
            if raw_bytes.len() < 4 { return false; }
            let f_val = i32::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as i32, operator),
                _ => false,
            }
        },
        DataType::Int64 => {
            if raw_bytes.len() < 8 { return false; }
            let f_val = i64::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3],
                 raw_bytes[4], raw_bytes[5], raw_bytes[6], raw_bytes[7]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as i64, operator),
                _ => false,
            }
        },
        DataType::Float32 => {
            if raw_bytes.len() < 4 { return false; }
            let f_val = f32::from_le_bytes([raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3]]);
            match condition_value {
                crate::sql::Value::Float(c_float) => compare_numbers(f_val, *c_float as f32, operator),
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as f32, operator),
                _ => false,
            }
        },
        DataType::Float64 => {
            if raw_bytes.len() < 8 { return false; }
            let f_val = f64::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3],
                 raw_bytes[4], raw_bytes[5], raw_bytes[6], raw_bytes[7]]);
            match condition_value {
                crate::sql::Value::Float(c_float) => compare_numbers(f_val, *c_float as f64, operator),
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int as f64, operator),
                _ => false,
            }
        },
        DataType::Bool => {
            if raw_bytes.is_empty() { return false; }
            let f_val = raw_bytes[0] != 0;
            match condition_value {
                crate::sql::Value::Boolean(c_bool) => compare_numbers(f_val, *c_bool, operator),
                _ => false,
            }
        },
        DataType::String => {
            let trimmed_end = raw_bytes.iter().position(|&b| b == 0).unwrap_or(raw_bytes.len());
            let f_str = &raw_bytes[..trimmed_end];
            match condition_value {
                crate::sql::Value::String(c_str) => {
                    let c_bytes = c_str.as_bytes();
                    compare_strings(f_str, c_bytes, operator)
                },
                _ => false,
            }
        },
        DataType::Timestamp | DataType::TimestampTZ => {
            if raw_bytes.len() < 8 { return false; }
            let f_val = i64::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3],
                 raw_bytes[4], raw_bytes[5], raw_bytes[6], raw_bytes[7]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int, operator),
                _ => false,
            }
        },
        DataType::Interval => {
            if raw_bytes.len() < 8 { return false; }
            let f_val = i64::from_le_bytes(
                [raw_bytes[0], raw_bytes[1], raw_bytes[2], raw_bytes[3],
                 raw_bytes[4], raw_bytes[5], raw_bytes[6], raw_bytes[7]]);
            match condition_value {
                crate::sql::Value::Integer(c_int) => compare_numbers(f_val, *c_int, operator),
                _ => false,
            }
        },
    }
}

/// Generic numeric comparison (reuses the existing compare_numbers or similar).
fn compare_numbers<T: PartialOrd>(a: T, b: T, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => a == b,
        ComparisonOperator::NotEqual => a != b,
        ComparisonOperator::LessThan => a < b,
        ComparisonOperator::LessThanOrEqual => a <= b,
        ComparisonOperator::GreaterThan => a > b,
        ComparisonOperator::GreaterThanOrEqual => a >= b,
    }
}

/// String comparison (for LIKE and text operators).
fn compare_strings(a: &[u8], b: &[u8], operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => a == b,
        ComparisonOperator::NotEqual => a != b,
        ComparisonOperator::LessThan => a < b,
        ComparisonOperator::LessThanOrEqual => a <= b,
        ComparisonOperator::GreaterThan => a > b,
        ComparisonOperator::GreaterThanOrEqual => a >= b,
        // LIKE is handled separately in evaluate_condition_raw
        _ => false,
    }
}
```

- [ ] **Step 2: Add tests verifying evaluate_condition_raw matches evaluate_condition**

```rust
// For each supported operator (=, !=, <, >, <=, >=):
// - Create a table with one record
// - Run evaluate_condition (old) and evaluate_condition_raw (new) with the same condition
// - Assert both return the same result
// - Test edge cases: NaN floats, string boundaries, cross-type conditions
// - Test AND/OR/NOT combinations
// - Test BETWEEN
```

- [ ] **Step 3: Run tests and commit**

```bash
cargo test --lib --test sql_query_test -- --test-threads=1
cargo test --lib --test record_view_test -- --test-threads=1
git add src/sql/query_executor.rs
git commit -m "feat: add evaluate_condition_raw for zero-copy WHERE clause evaluation"
```

---
### Task 4: Rewrite `execute_select_query()` — Zero-copy SELECT path

**Files:**
- Modify: `src/sql/query_executor.rs` (replace `execute_select_query`)
- Modify: `src/sql/result_set.rs` (add `value_to_string_repr` if needed)
- Test: `tests/sql_query_test.rs` (add zero-copy query tests)

**Interfaces:**
- Consumes: `RawRecordView`, `CompactResultSet`, `evaluate_condition_raw`, `MemoryTable::iterate`, `SqlQuery`, `OrderByClause`
- Produces: Updated `execute_select_query` returning `CompactResultSet`

- [ ] **Step 1: Replace the body of `execute_select_query()` to use the new zero-copy path**

```rust
fn execute_select_query(db: &mut RemDb, query: &SqlQuery) -> Result<CompactResultSet, QueryExecutionError> {
    // 1. Find the table
    let table = find_table_by_name(db, &query.table_name)?;

    // 2. Build column metadata
    let columns = if query.select_all {
        table.def.fields.iter().map(|f| {
            Expression::Field { name: f.name.to_string(), alias: None }
        }).collect()
    } else {
        validate_columns(table, &query.columns)?;
        query.columns.clone()
    };

    let result_columns = columns.iter().map(|expr| {
        match expr {
            Expression::Field { name, alias } => {
                alias.clone().unwrap_or_else(|| name.clone())
            },
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            },
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            },
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            },
        }
    }).collect::<Vec<_>>();

    // 3. Build ColumnInfo list for the result set
    let col_info = build_column_info(table, &columns)?;

    // 4. Create result set
    let mut result_set = CompactResultSet::new(col_info, table.record_size);

    // 5. Collect matching record IDs (zero-copy filtering)
    let mut matching_ids: Vec<usize> = Vec::new();
    table.iterate(|id, record_data| {
        let view = RawRecordView::new(record_data, &table.def);
        let matches = if let Some(where_clause) = &query.where_clause {
            evaluate_condition_raw(&view, &where_clause.condition, &table.def)
        } else {
            true
        };
        if matches {
            matching_ids.push(id);
        }
        true // continue iteration
    }).map_err(|_| QueryExecutionError::InternalError)?;

    // 6. ORDER BY (zero-copy sort)
    if let Some(order_by) = &query.order_by {
        // Sort matching_ids by comparing raw record bytes
        sort_ids_by_raw(&mut matching_ids, table, order_by)?;
    }

    // 7. LIMIT
    let limit = query.limit.unwrap_or(matching_ids.len());
    let limit = core::cmp::min(matching_ids.len(), limit);

    // 8. Check for aggregation
    let has_aggregate = columns.iter().any(|expr| is_aggregate_function(expr));
    let has_group_by = query.group_by.is_some();

    if has_aggregate || has_group_by {
        if has_group_by {
            process_group_by_raw(table, &columns, &matching_ids[..limit],
                query.group_by.as_ref().unwrap(), &mut result_set)?;
        } else {
            process_aggregate_raw(table, &columns, &matching_ids[..limit],
                &mut result_set)?;
        }
    } else {
        // 9. Bulk-copy matching records into result set
        for &id in matching_ids.iter().take(limit) {
            if let Ok(record_slice) = table.get_record_slice_checked(id) {
                let _ = result_set.add_record(record_slice);
            }
        }

        // 10. Evaluate expressions for each row (only for SELECT expressions)
        if !query.select_all && !columns.is_empty() {
            // For non-trivial expressions, we need evaluate them
            // This is a hybrid: we still need TypedValue for expression evaluation
            let mut typed_results = CompactResultSet::new(
                build_column_info_for_exprs(&columns, table)?,
                table.record_size,
            );
            for &id in matching_ids.iter().take(limit) {
                if let Ok(record_slice) = table.get_record_slice_checked(id) {
                    let view = RawRecordView::new(record_slice, &table.def);
                    let mut row_data = Vec::new();
                    for expr in &columns {
                        let tv = evaluate_expression_raw(&view, table, expr)?;
                        row_data.push(tv);
                    }
                    // Convert row_data to raw bytes and add
                    let mut buf = alloc::vec![0u8; table.record_size];
                    for (i, tv) in row_data.iter().enumerate() {
                        if let Some(col) = typed_results.columns.get(i) {
                            set_value_at_offset(&mut buf, col.offset, tv);
                        }
                    }
                    let _ = typed_results.add_record(&buf);
                }
            }
            return Ok(typed_results);
        }
    }

    Ok(result_set)
}
```

- [ ] **Step 2: Add helper functions**

```rust
/// Build ColumnInfo list from field expressions.
fn build_column_info(table: &MemoryTable, columns: &[Expression]) -> Result<Vec<ColumnInfo>, QueryExecutionError> {
    let mut result = Vec::with_capacity(columns.len());
    for expr in columns {
        match expr {
            Expression::Field { name, .. } => {
                let actual_name = if let Some(dot) = name.find('.') { &name[dot+1..] } else { name };
                if let Some(field) = table.def.fields.iter().find(|f| f.name == actual_name) {
                    result.push(ColumnInfo {
                        name: name.clone(),
                        offset: field.offset,
                        data_type: field.data_type,
                        size: field.size,
                    });
                }
            },
            _ => {
                // Non-field expressions use a placeholder
                result.push(ColumnInfo {
                    name: expr_name(expr),
                    offset: 0,
                    data_type: DataType::Int64,
                    size: 8,
                });
            },
        }
    }
    Ok(result)
}

/// Check if an expression is an aggregate function.
fn is_aggregate_function(expr: &Expression) -> bool {
    // Same logic as the existing check in execute_select_query
    // (copied from the old code's has_aggregate detection)
    match expr {
        Expression::FunctionCall { name, .. } => {
            match name.to_uppercase().as_str() {
                "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" |
                "STDDEV" | "VAR" | "STDDEV_SAMP" | "VAR_SAMP" |
                "MOVING_AVERAGE" | "MOVING_SUM" => true,
                _ => false,
            }
        },
        _ => false,
    }
}
```

- [ ] **Step 3: Update `execute_query` to return `CompactResultSet`**

```rust
pub fn execute_query(db: &mut RemDb, query: &SqlQuery) -> Result<CompactResultSet, QueryExecutionError> {
    // ... same dispatcher, just change return type
}
```

- [ ] **Step 4: Add `get_record_slice_checked` to `MemoryTable` (safe version of get_record_slice)**

```rust
/// Get record data slice with full bounds checking (safe version).
pub fn get_record_slice_checked(&self, index: usize) -> Result<&[u8]> {
    if index >= self.def.max_records {
        return Err(RemDbError::RecordNotFound);
    }
    if self.status_array.get(index).map(|s| s.status) != Some(RecordStatus::Used) {
        return Err(RemDbError::RecordNotFound);
    }
    let start = index * self.record_size;
    let end = start + self.record_size;
    self.data.get(start..end).ok_or(RemDbError::RecordNotFound)
}
```

- [ ] **Step 5: Sort IDs by raw record data**

```rust
/// Sort record IDs by comparing raw bytes (zero-copy sort).
fn sort_ids_by_raw(
    ids: &mut Vec<usize>,
    table: &MemoryTable,
    order_by: &OrderByClause,
) -> Result<(), QueryExecutionError> {
    let field_idx = resolve_sort_field_index(order_by, table)?;
    let field = &table.def.fields[field_idx];
    let field_type = field.data_type;
    let offset = field.offset;
    let size = field.size;
    let direction = &order_by.direction;

    ids.sort_by(|&a, &b| {
        let a_slice = table.data.get(a * table.record_size + offset..)
            .and_then(|s| s.get(..size)).unwrap_or(&[]);
        let b_slice = table.data.get(b * table.record_size + offset..)
            .and_then(|s| s.get(..size)).unwrap_or(&[]);
        let cmp = compare_raw_bytes(a_slice, b_slice, field_type);
        match direction {
            crate::sql::OrderDirection::Ascending => cmp,
            crate::sql::OrderDirection::Descending => cmp.reverse(),
        }
    });
    Ok(())
}

/// Compare two raw byte slices as typed values.
fn compare_raw_bytes(a: &[u8], b: &[u8], data_type: DataType) -> core::cmp::Ordering {
    if a.is_empty() || b.is_empty() { return core::cmp::Ordering::Equal; }
    match data_type {
        DataType::UInt8 => a[0].cmp(&b[0]),
        DataType::UInt16 => {
            u16::from_le_bytes([a[0], a[1]]).cmp(&u16::from_le_bytes([b[0], b[1]]))
        },
        // ... same pattern for all numeric types
        DataType::String => a.cmp(b),
        _ => core::cmp::Ordering::Equal,
    }
}
```

- [ ] **Step 6: Run tests and commit**

```bash
cargo test --lib --features "pubsub ha" -- --test-threads=1
git add src/sql/query_executor.rs src/table.rs
git commit -m "feat: rewrite execute_select_query with zero-copy RawRecordView path"
```

---
### Task 5: Zero-copy aggregation and GROUP BY

**Files:**
- Modify: `src/sql/query_executor.rs` (add `process_aggregate_raw`, `process_group_by_raw`)
- Test: `tests/sql_query_test.rs` (add aggregation tests)

- [ ] **Step 1: Add `process_aggregate_raw`**

```rust
/// Process aggregate functions using RawRecordView (zero-copy accumulation).
/// Replicates the logic of process_aggregate_query but reads from RawRecordView
/// directly instead of from Vec<TypedValue> rows.
fn process_aggregate_raw(
    table: &MemoryTable,
    columns: &[Expression],
    matching_ids: &[usize],
    result_set: &mut CompactResultSet,
) -> Result<(), QueryExecutionError> {
    // Initialize aggregate values (same initialization as process_aggregate_query)
    // For COUNT: start at 0, for SUM: start at 0, for MIN: start at MAX, for MAX: start at MIN
    // For AVG: track sum + count, for STDDEV/VAR: track sum + sum_of_squares + count

    let mut agg_values = Vec::with_capacity(columns.len());
    let mut var_states: Vec<(f64, f64, usize)> = Vec::with_capacity(columns.len());

    for expr in columns {
        if let Expression::FunctionCall { name, .. } = expr {
            match name.to_uppercase().as_str() {
                "COUNT" => {
                    agg_values.push(TypedValue { value_type: DataType::UInt64, value: Value::U64(0) });
                    var_states.push((0.0, 0.0, 0));
                },
                "SUM" => {
                    agg_values.push(TypedValue { value_type: DataType::Float64, value: Value::Float64(0.0) });
                    var_states.push((0.0, 0.0, 0));
                },
                "AVG" => {
                    agg_values.push(TypedValue { value_type: DataType::Float64, value: Value::Float64(0.0) });
                    var_states.push((0.0, 0.0, 0));
                },
                "MIN" => {
                    agg_values.push(TypedValue { value_type: DataType::UInt64, value: Value::U64(u64::MAX) });
                    var_states.push((0.0, 0.0, 0));
                },
                "MAX" => {
                    agg_values.push(TypedValue { value_type: DataType::UInt64, value: Value::U64(0) });
                    var_states.push((0.0, 0.0, 0));
                },
                _ => {
                    agg_values.push(TypedValue { value_type: DataType::UInt64, value: Value::U64(0) });
                    var_states.push((0.0, 0.0, 0));
                },
            }
        }
    }

    // Accumulate from each matching record
    for &id in matching_ids {
        let record_slice = table.get_record_slice_checked(id)
            .map_err(|_| QueryExecutionError::InternalError)?;
        let view = RawRecordView::new(record_slice, &table.def);

        for (i, expr) in columns.iter().enumerate() {
            if let Expression::FunctionCall { name, args, .. } = expr {
                match name.to_uppercase().as_str() {
                    "COUNT" => {
                        // COUNT(*) — just increment
                        if let Some(agg) = agg_values.get_mut(i) {
                            if let Value::U64(count) = &mut agg.value {
                                *count += 1;
                            }
                        }
                    },
                    "SUM" | "AVG" => {
                        // Read the argument field value from raw bytes
                        if let Some(arg) = args.first() {
                            if let Expression::Field { name: field_name, .. } = arg {
                                if let Ok(field_idx) = view.resolve_field_index(field_name) {
                                    let field_type = table.def.fields[field_idx].data_type;
                                    if let Ok(val) = read_numeric_from_view(&view, field_idx, field_type) {
                                        let (sum, _, count) = &mut var_states[i];
                                        *sum += val;
                                        *count += 1;
                                    }
                                }
                            }
                        }
                    },
                    "MIN" => {
                        if let Some(arg) = args.first() {
                            if let Expression::Field { name: field_name, .. } = arg {
                                if let Ok(field_idx) = view.resolve_field_index(field_name) {
                                    let field_type = table.def.fields[field_idx].data_type;
                                    if let Ok(val) = read_numeric_from_view(&view, field_idx, field_type) {
                                        if let Some(agg) = agg_values.get_mut(i) {
                                            let current = agg.value.as_float64();
                                            if val < current {
                                                agg.value = Value::Float64(val);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "MAX" => {
                        // ... same pattern as MIN with > comparison
                    },
                    _ => {},
                }
            }
        }
    }

    // Finalize aggregate values (e.g., AVG = sum / count)
    for (i, expr) in columns.iter().enumerate() {
        if let Expression::FunctionCall { name, .. } = expr {
            match name.to_uppercase().as_str() {
                "AVG" => {
                    let (sum, _, count) = var_states[i];
                    if count > 0 {
                        agg_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value::Float64(sum / count as f64),
                        };
                    }
                },
                "SUM" => {
                    let (sum, _, _) = var_states[i];
                    agg_values[i] = TypedValue {
                        value_type: DataType::Float64,
                        value: Value::Float64(sum),
                    };
                },
                _ => {},
            }
        }
    }

    // Write aggregate results to result_set
    // (In a proper implementation, serialize agg_values into raw bytes)
    let mut buf = alloc::vec![0u8; result_set.record_size];
    for (i, tv) in agg_values.iter().enumerate() {
        if let Some(col) = result_set.columns.get(i) {
            set_value_at_offset(&mut buf, col.offset, tv);
        }
    }
    let _ = result_set.add_record(&buf);

    Ok(())
}

/// Read a numeric value from a RawRecordView as f64.
fn read_numeric_from_view(view: &RawRecordView, field_idx: usize, data_type: DataType) -> Result<f64, QueryExecutionError> {
    match data_type {
        DataType::UInt8 => Ok(view.read_u8(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::UInt16 => Ok(view.read_u16(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::UInt32 => Ok(view.read_u32(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::UInt64 => Ok(view.read_u64(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Int8 => Ok(view.read_i8(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Int16 => Ok(view.read_i16(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Int32 => Ok(view.read_i32(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Int64 => Ok(view.read_i64(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Float32 => Ok(view.read_f32(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)? as f64),
        DataType::Float64 => Ok(view.read_f64(field_idx).map_err(|_| QueryExecutionError::TypeMismatch)?),
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// Helper: set a Value at a specific byte offset in a buffer.
fn set_value_at_offset(buf: &mut [u8], offset: usize, tv: &TypedValue) {
    // Implementation mirrors MemoryTable::set_field but works on a raw buffer
    match tv.value_type {
        DataType::UInt8 => { if let Some(dst) = buf.get_mut(offset) { *dst = tv.value.as_u8(); } },
        DataType::UInt16 => {
            let bytes = tv.value.as_u16().to_le_bytes();
            if let Some(dst) = buf.get_mut(offset..offset + 2) { dst.copy_from_slice(&bytes); }
        },
        // ... same pattern for all types
        _ => {},
    }
}
```

- [ ] **Step 2: Add `process_group_by_raw`** 

```rust
/// Process GROUP BY queries using RawRecordView.
/// Groups matching_ids by group key (extracted from raw bytes),
/// then aggregates values per group.
fn process_group_by_raw(
    table: &MemoryTable,
    columns: &[Expression],
    matching_ids: &[usize],
    group_by: &GroupByClause,
    result_set: &mut CompactResultSet,
) -> Result<(), QueryExecutionError> {
    // Resolve group-by field index
    let group_field_idx = table.def.fields.iter()
        .position(|f| f.name == group_by.field)
        .ok_or(QueryExecutionError::FieldNotFound)?;
    let group_field = &table.def.fields[group_field_idx];

    // Group matching_ids by their group key (raw bytes)
    // Use a simple approach: sort by group key, then scan for groups
    let mut id_group_key: Vec<(usize, Vec<u8>)> = Vec::with_capacity(matching_ids.len());
    for &id in matching_ids {
        if let Ok(slice) = table.get_record_slice_checked(id) {
            if let Some(key_slice) = slice.get(group_field.offset..group_field.offset + group_field.size) {
                id_group_key.push((id, key_slice.to_vec())); // key bytes copied, unavoidable
            }
        }
    }
    // Sort by group key
    id_group_key.sort_by(|a, b| a.1.cmp(&b.1));

    // Scan sorted groups and aggregate
    let mut i = 0;
    while i < id_group_key.len() {
        let current_key = &id_group_key[i].1;
        let mut group_ids = Vec::new();
        while i < id_group_key.len() && id_group_key[i].1 == *current_key {
            group_ids.push(id_group_key[i].0);
            i += 1;
        }
        // Aggregate values for this group
        // (same logic as process_aggregate_raw but over group_ids)
        // Write group key + aggregate values to result_set
    }

    Ok(())
}

- [ ] **Step 3: Run tests and commit**

```bash
cargo test --lib --features "pubsub ha" -- --test-threads=1
git add src/sql/query_executor.rs
git commit -m "feat: add zero-copy aggregation and GROUP BY"
```

---
### Task 6: Update exports, deprecation, and fix callers

**Files:**
- Modify: `src/sql/mod.rs` (update exports for `CompactResultSet`)
- Modify: `src/lib.rs` (re-export `CompactResultSet`, `RawRecordView`)
- Modify: `src/sql/query_executor.rs` (update `execute_query` return type)
- Modify: `src/lib.rs` (update any callers of `execute_query`)

- [ ] **Step 1: Update `src/sql/mod.rs`**

```rust
pub use query_executor::{execute_query, QueryExecutionError};
pub use result_set::{CompactResultSet, ColumnInfo, ResultSet, ResultRow, ResultRowIter};
```

- [ ] **Step 2: Update `src/lib.rs` re-exports**

```rust
pub use record_view::RawRecordView;
pub use sql::{CompactResultSet, ColumnInfo};
```

- [ ] **Step 3: Fix any internal callers of `execute_query` that expect `ResultSet`**

- [ ] **Step 4: Run full test suite**

```bash
cargo test --lib --features "pubsub ha" -- --test-threads=1
```

- [ ] **Step 5: Commit**

```bash
git add src/sql/mod.rs src/lib.rs
git commit -m "feat: update exports, return CompactResultSet from execute_query"
```

---
### Task 7: Integration tests for the full zero-copy query path

**Files:**
- Modify: `tests/sql_query_test.rs` (add comprehensive tests)
- Modify: `tests/compact_result_set_test.rs` (add edge cases)

- [ ] **Step 1: Add tests for SELECT with WHERE on every data type**

```rust
#[test]
fn test_select_with_where_uint8() { ... }
#[test]
fn test_select_with_where_string() { ... }
// ... one test per data type
```

- [ ] **Step 2: Add tests for ORDER BY with every data type**

```rust
#[test]
fn test_order_by_u32_asc() { ... }
#[test]
fn test_order_by_string_desc() { ... }
// ... one test per data type, both ascending and descending
```

- [ ] **Step 3: Add tests for aggregation**

```rust
#[test]
fn test_aggregate_count_raw() { ... }
#[test]
fn test_aggregate_sum_raw() { ... }
#[test]
fn test_aggregate_avg_raw() { ... }
#[test]
fn test_aggregate_min_max_raw() { ... }
```

- [ ] **Step 4: Add tests for edge cases**

```rust
#[test]
fn test_empty_result_set() { ... }
#[test]
fn test_where_and_or_not() { ... }
#[test]
fn test_where_between() { ... }
#[test]
fn test_limit() { ... }
#[test]
fn test_distinct() { ... }
```

- [ ] **Step 5: Run all tests**

```bash
cargo test --lib --features "pubsub ha" -- --test-threads=1
```

- [ ] **Step 6: Commit**

```bash
git add tests/sql_query_test.rs tests/compact_result_set_test.rs
git commit -m "test: add integration tests for zero-copy query path"
```

---
### Task 8: Performance benchmarks

**Files:**
- Modify: `benches/database.rs` (add zero-copy benchmark)

- [ ] **Step 1: Add benchmark comparing old vs new SELECT path**

```rust
fn bench_select_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("select_query");
    // Setup: create table with 1000 records
    // Benchmark: old path (using ResultSet)
    // Benchmark: new path (using CompactResultSet)
    // Compare: queries/second, allocations, memory
    group.bench_function("old_path", |b| { ... });
    group.bench_function("new_path", |b| { ... });
    group.finish();
}
```

- [ ] **Step 2: Run benchmarks**

```bash
cargo bench -- select_query
```

- [ ] **Step 3: Record results and commit**

```bash
git add benches/database.rs
git commit -m "bench: add zero-copy query path benchmarks"
```