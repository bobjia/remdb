# Plan: Modularize query_executor.rs

## Context

The `query_executor.rs` file has grown to **~9,423 lines**, making it difficult to maintain, test, and understand. The file mixes multiple concerns including DDL, DML, SELECT operations, expression evaluation, time series operations, and various utility functions.

The `operations/` directory already exists with placeholder modules (`ddl.rs`, `dml.rs`, `select.rs`, `timeseries.rs`, `expression.rs`), but they contain only stub implementations. The `functions/` directory demonstrates a good modularization pattern that we should follow.

**Goal**: Split `query_executor.rs` into focused modules following the existing directory structure, improving code organization, testability, and maintainability while preserving all existing functionality and `no_std` compatibility.

---

## Current Structure

```
remdb/src/sql/
├── query_executor.rs    (9,423 lines - TOO LARGE)
├── query_parser.rs
├── result_set.rs
├── mod.rs
├── functions/
│   ├── mod.rs
│   ├── aggregate.rs
│   ├── json.rs
│   ├── math.rs
│   ├── string.rs
│   └── time.rs
└── operations/
    ├── mod.rs           (placeholder re-exports)
    ├── ddl.rs           (partial implementation ~83 lines)
    ├── dml.rs           (placeholder)
    ├── select.rs        (placeholder)
    ├── timeseries.rs    (placeholder)
    └── expression.rs    (placeholder)
```

---

## Proposed Structure

```
remdb/src/sql/
├── query_executor.rs    (~200 lines - routing + error types only)
├── query_parser.rs
├── result_set.rs
├── mod.rs
├── error.rs             (NEW - QueryExecutionError)
├── utils.rs             (NEW - shared utilities)
├── functions/           (unchanged)
└── operations/
    ├── mod.rs           (updated re-exports)
    ├── ddl.rs           (~1,800 lines - CREATE/DROP TABLE/INDEX/DATABASE)
    ├── dml.rs           (~1,200 lines - INSERT/UPDATE/DELETE)
    ├── select.rs        (~1,800 lines - SELECT, JOIN, GROUP BY)
    ├── timeseries.rs    (~1,500 lines - time series operations)
    ├── expression.rs    (~1,200 lines - expression evaluation)
    ├── comparison.rs    (~500 lines - comparison functions, LIKE pattern)
    └── vector.rs        (~300 lines - vector distance operations)
```

---

## Implementation Steps

### Step 1: Extract Error Types
**File**: `remdb/src/sql/error.rs`

Move `QueryExecutionError` and its implementations from `query_executor.rs`:
- Lines 109-158 in current file

```rust
// error.rs
#[derive(Debug, Clone, PartialEq)]
pub enum QueryExecutionError { ... }

impl Display for QueryExecutionError { ... }
impl Error for QueryExecutionError { ... }
```

### Step 2: Extract Utility Functions
**File**: `remdb/src/sql/utils.rs`

Move shared utility functions:
- `parse_data_type_with_precision` (lines 33-105)
- `sort_rows` / `sort_rows_with_alias` (lines 8798+, 9150+)
- `check_memory_limit` / `estimate_memory_usage` (lines 953-990)
- `execute_with_timeout` (lines 985-1027)
- `get_field_value` / `get_field_value_from_condition` (lines 2150-2210, 9388+)
- `find_table_by_name` / `find_timeseries_table_by_name` (lines 495-560, 5587-5616)
- `validate_columns` / `validate_expression` (lines 5617-5684)

### Step 3: Populate operations/expression.rs
**File**: `remdb/src/sql/operations/expression.rs`

Move expression evaluation logic (~1,200 lines):
- `evaluate_expression` / `evaluate_expression_with_depth` (lines 3219-3440)
- `evaluate_binary_op` (lines 3635-4074)
- `evaluate_unary_op` (lines 3151-3216)
- `evaluate_vector_binary_op` (lines 3440-3635)
- `execute_function_call` (lines 4074-4144)
- `evaluate_expression_without_table` (lines 5684-5750+)
- `evaluate_expression_for_aggregate` (lines 1542-1630)

### Step 4: Populate operations/comparison.rs
**File**: `remdb/src/sql/operations/comparison.rs`

Move comparison logic (~500 lines):
- `compare_values` (lines 2210-2254)
- `compare_field_with_condition` (lines 7940-8130)
- `compare_numbers` / `compare_booleans` / `compare_strings` (lines 8130-8166)
- `like_pattern_match` (lines 8166-8798)
- `extract_indexed_condition` / `extract_index_operation` (lines 5007-5157)
- `IndexOperation` enum (lines 5061-5069)

### Step 5: Populate operations/vector.rs
**File**: `remdb/src/sql/operations/vector.rs`

Move vector operations (~300 lines):
- `calculate_vector_l2_distance` (lines 7853-7865)
- `calculate_vector_inner_product` (lines 7865-7876)
- `calculate_vector_cosine_similarity` (lines 7876-7899)
- `parse_vector_distance_expression` / `parse_vector_op` (lines 7899-7940)

### Step 6: Populate operations/ddl.rs
**File**: `remdb/src/sql/operations/ddl.rs`

Expand with remaining DDL operations (~1,800 lines total):
- `execute_create_table_query` (lines 4145-4759)
- `execute_create_index_query` (lines 4924-5007)
- `execute_create_time_series_table_query` (lines 7298-7440)
- `execute_show_tables_query` (lines 4871-4924)
- `execute_show_index_build_status_query` (lines 4759-4871)
- `execute_describe_query` (lines 5892-6108)

### Step 7: Populate operations/dml.rs
**File**: `remdb/src/sql/operations/dml.rs`

Move DML operations (~1,200 lines):
- `execute_insert_query` (lines 6108-6594)
- `execute_update_query` (lines 6693-6853)
- `execute_delete_query` (lines 6594-6693)
- `set_field_value` / `set_field_value_with_depth` (lines 6853-7298)
- `execute_insert_timeseries_query` (lines 7440-7853)

### Step 8: Populate operations/select.rs
**File**: `remdb/src/sql/operations/select.rs`

Move SELECT operations (~1,800 lines):
- `execute_select_query` (lines 1653-2081)
- `execute_select_join_query` (lines 2321-3151)
- `process_group_by_query` (lines 5157-5587)
- `process_aggregate_query` (lines 1079-1542)
- `add_joined_row` (lines 2081-2150)
- `validate_cross_table_columns` (lines 2254-2321)

### Step 9: Populate operations/timeseries.rs
**File**: `remdb/src/sql/operations/timeseries.rs`

Move time series operations (~1,500 lines):
- `execute_select_timeseries_query` (lines 596-741)
- `execute_timeseries_expression` (lines 1027-1079)
- `downsample_records` (lines 741-835)
- `interpolate_missing_window` (lines 835-922)
- `parse_sample_interval` (lines 922-953)
- `extract_time_range_from_condition` (lines 511-596)
- Time processing: `process_at_time_zone`, `process_timezone_function`, `process_to_char`, `process_to_iso8601`, `process_to_epoch` (lines 9074-9148)

### Step 10: Refactor query_executor.rs
**File**: `remdb/src/sql/query_executor.rs`

Reduce to thin routing layer (~200 lines):
- Keep `execute_query` as main entry point/router
- Keep `QueryStats` struct (lines 1630-1653)
- Import and delegate to operation modules
- Remove all moved code

### Step 11: Update Module Exports
**File**: `remdb/src/sql/mod.rs`

Update exports to maintain API compatibility:
```rust
mod error;
mod utils;

pub use error::QueryExecutionError;
pub use query_executor::execute_query;
// ... existing exports
```

### Step 12: Update operations/mod.rs
**File**: `remdb/src/sql/operations/mod.rs`

```rust
pub mod ddl;
pub mod dml;
pub mod select;
pub mod timeseries;
pub mod expression;
pub mod comparison;
pub mod vector;

pub use ddl::*;
pub use dml::*;
pub use select::*;
pub use timeseries::*;
pub use expression::*;
pub use comparison::*;
pub use vector::*;
```

---

## Key Considerations

### 1. Dependency Management
Each new module will need appropriate imports. Common patterns:
```rust
use alloc::string::String;
use alloc::vec::Vec;
use crate::sql::{QueryExecutionError, ResultSet, SqlQuery};
use crate::types::{DataType, TypedValue, Value};
use crate::{MemoryTable, RemDb, RemDbError};
```

### 2. no_std Compatibility
Maintain `alloc` crate usage instead of `std` for collections. Use `#[cfg(feature = "log")]` for logging.

### 3. Circular Dependencies
Avoid circular dependencies by:
- Keeping shared types in `error.rs` and `utils.rs`
- Having operation modules depend on `error` and `utils`, not each other
- Using the main `query_executor.rs` for orchestration

### 4. Testing Strategy
After each module extraction:
1. Run `cargo test --lib -p remdb` to verify all tests pass
2. Run `cargo clippy -- -D warnings` to ensure no new warnings
3. Run `cargo build --no-default-features --features=baremetal -p remdb` to verify `no_std` compatibility

---

## Verification

1. **Build**: `cargo build -p remdb`
2. **Test**: `cargo test --lib -p remdb`
3. **Lint**: `cargo clippy --all-targets --all-features -- -D warnings`
4. **no_std**: `cargo build --no-default-features --features=baremetal -p remdb`
5. **Size check**: Verify the modularized code compiles to similar size

---

## Expected Outcome

| File | Before | After |
|------|--------|-------|
| query_executor.rs | ~9,423 lines | ~200 lines |
| error.rs | N/A | ~60 lines |
| utils.rs | N/A | ~400 lines |
| operations/ddl.rs | ~83 lines | ~1,800 lines |
| operations/dml.rs | ~18 lines | ~1,200 lines |
| operations/select.rs | ~10 lines | ~1,800 lines |
| operations/timeseries.rs | ~18 lines | ~1,500 lines |
| operations/expression.rs | ~10 lines | ~1,200 lines |
| operations/comparison.rs | N/A | ~500 lines |
| operations/vector.rs | N/A | ~300 lines |

This modularization will:
- Improve code navigation and comprehension
- Enable easier unit testing of individual components
- Reduce merge conflicts in concurrent development
- Follow established patterns from the `functions/` module
- Maintain full backward compatibility and `no_std` support