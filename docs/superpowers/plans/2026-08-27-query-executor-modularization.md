# Query Executor Modularization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the in-flight migration of `src/sql/query_executor.rs` (7,752 lines) into focused `operations/` modules with zero behavior change.

**Architecture:** Verbatim code moves only — function bodies, doc comments, and `#[cfg]` gates are relocated unchanged. `query_executor.rs` becomes a thin router (dispatch + permission checks). The one code-shape change: the inline ALTER TABLE block is wrapped into `execute_alter_table_query`. Spec: `docs/superpowers/specs/2026-08-27-query-executor-modularization-design.md`.

**Tech Stack:** Rust 2021, `no_std`-compatible (uses `alloc`), optional features `log`/`std` gated with `#[cfg]`.

**Testing approach (TDD adaptation):** This is a pure mechanical refactor of code with an existing comprehensive test suite (integration tests in `tests/` exercise `execute_query` heavily). Instead of writing new failing tests, the discipline is: capture a baseline in Task 1, and keep the full suite green after **every** task. Any test delta = a botched move, not a test problem.

**Conventions for all move steps:**
- "Move function X" means: cut from `query_executor.rs` the `///` doc-comment lines immediately above `fn X(...)` (if any), any `#[derive(...)]`/`#[cfg]` attributes attached to it, and the full body down to its closing `}` — paste into the target file unchanged. Do not reformat, rename, or "improve" anything.
- Line numbers below refer to the **pre-refactor** file and shift after each task. Always locate items by Grep pattern (given per task), never by stale line numbers.
- The crate does NOT allow `unused_imports`, so each task ends by pruning imports the compiler flags.
- Work happens in `/workspace`.

---

## File Structure (final state)

```
src/sql/
├── mod.rs                  (unchanged exports; no edits needed)
├── query_executor.rs       ~400 lines: execute_query router + permission checks
├── error.rs                (unchanged)
├── utils.rs                + 7 helpers (sort, memory estimate, timeout, field lookup)
├── functions/              (unchanged)
└── operations/
    ├── mod.rs              + dml, select, timeseries declarations
    ├── comparison.rs       + IndexOperation, extract_index_operation, extract_indexed_condition
    ├── expression.rs       + evaluate_expression_for_aggregate
    ├── vector.rs           (unchanged)
    ├── ddl.rs              + 8 handlers + execute_alter_table_query
    ├── dml.rs              NEW
    ├── select.rs           NEW
    └── timeseries.rs       NEW
```

Task order is dependency-driven: leaf helpers first (utils → comparison → expression), then leaf modules (timeseries → dml → ddl), then select (depends on all prior), then final verification.

---

### Task 1: Capture baseline

**Files:** none modified.

- [ ] **Step 1.1: Record baseline build + warnings**

Run: `cargo build 2>&1 | tail -5 > /tmp/baseline_build.txt; cat /tmp/baseline_build.txt`
Expected: `Finished` line. Note any warnings count.

- [ ] **Step 2.2: Record baseline test results**

Run: `cargo test 2>&1 | tail -30 > /tmp/baseline_test.txt; cat /tmp/baseline_test.txt`
Expected: test summary lines. Record pass/fail counts — every later task must reproduce exactly this set.

- [ ] **Step 3.3: Record baremetal build status**

Run: `cargo build --no-default-features --features=baremetal 2>&1 | tail -5 > /tmp/baseline_baremetal.txt; cat /tmp/baseline_baremetal.txt`
Expected: `Finished` (if it fails on baseline, baremetal parity is waived — final task only requires std parity).

- [ ] **Step 4.4: No commit** (nothing changed)

---

### Task 2: Move shared helpers to utils.rs

**Files:**
- Modify: `src/sql/utils.rs`
- Modify: `src/sql/query_executor.rs`

Functions to move (locate each with Grep pattern `^fn <name>`):

| Function | Grep pattern | Pre-refactor line |
|---|---|---|
| `estimate_memory_usage` | `^fn estimate_memory_usage\(` | 906 |
| `estimate_memory_usage_for_records` | `^fn estimate_memory_usage_for_records\(` | 913 |
| `execute_with_timeout` | `^fn execute_with_timeout` | 921 |
| `get_field_value_from_condition` | `^fn get_field_value_from_condition` | 2138 |
| `sort_rows_with_alias` | `^fn sort_rows_with_alias\(` | 7080 |
| `sort_rows` | `^fn sort_rows\(` | 7409 |
| `expr_to_order_by_string` | `^fn expr_to_order_by_string\(` | 7725 |

All seven keep their current `fn` (private) visibility — they are called via `crate::sql::utils::<name>` from sibling modules, which works because `mod utils` is private to `sql` but `operations::*` is a descendant module.

- [ ] **Step 2.1: Append imports to utils.rs**

After the existing `use crate::types::{DEFAULT_JSON_SIZE, DEFAULT_TEXT_SIZE};` line add:

```rust
use crate::sql::query_parser::Expression;
use crate::sql::{OrderByClause, SqlQuery};
use crate::types::TypedValue;
use crate::MemoryTable;
```

(`execute_with_timeout`'s `std::` uses are inner `use` statements inside the function body — they move with it. `BTreeMap` and `alloc::string::String` in `sort_rows_with_alias`/`expr_to_order_by_string` are fully-qualified paths — no imports needed.)

- [ ] **Step 2.2: Move the seven functions**

Cut each function (doc comments + attributes + body, verbatim) from `query_executor.rs`; paste at the end of `utils.rs` in the table order.

- [ ] **Step 2.3: Fix callers still inside query_executor.rs**

`execute_select_query` (still in `query_executor.rs` until Task 8) calls `sort_rows_with_alias` and `estimate_memory_usage_for_records`; the timeseries SELECT path calls `estimate_memory_usage`. Add to `query_executor.rs` imports:

```rust
use crate::sql::utils::{
    estimate_memory_usage, estimate_memory_usage_for_records, sort_rows_with_alias,
};
```

(Adjust names to what the compiler actually reports as unresolved — e.g. drop `estimate_memory_usage` if the call moved already.)

- [ ] **Step 2.4: Build and prune**

Run: `cargo build 2>&1 | grep -E "^(error|warning)" | sort | uniq -c`
Expected: no `error`; no new `warning: unused import` vs baseline. Prune any flagged imports, rebuild.

- [ ] **Step 2.5: Test**

Run: `cargo test 2>&1 | tail -5`
Expected: identical pass/fail set to `/tmp/baseline_test.txt`.

- [ ] **Step 2.6: Commit**

```bash
git add src/sql/utils.rs src/sql/query_executor.rs
git commit -m "refactor: move sort/estimate/timeout helpers from query_executor to sql::utils"
```

---

### Task 3: Move index-extraction to operations/comparison.rs

**Files:**
- Modify: `src/sql/operations/comparison.rs`
- Modify: `src/sql/query_executor.rs`

Items to move:

| Item | Grep pattern | Pre-refactor line |
|---|---|---|
| `enum IndexOperation` | `^enum IndexOperation` | 4029 |
| `fn extract_indexed_condition` | `^fn extract_indexed_condition\(` | 3971 |
| `fn extract_index_operation` | `^fn extract_index_operation\(` | 4037 |

- [ ] **Step 3.1: Move the three items verbatim** into `operations/comparison.rs` (end of file). Change visibility: `enum IndexOperation` → `pub enum IndexOperation`; `fn extract_index_operation` → `pub fn extract_index_operation`; `extract_indexed_condition` stays private (only called by `extract_index_operation`). Move `enum IndexOperation`'s doc comment `/// 索引操作类型` with it.

- [ ] **Step 3.2: Fix caller in query_executor.rs**

`execute_select_query` uses `extract_index_operation` and matches on `IndexOperation::Equal`/`IndexOperation::Range`. Add to its imports:

```rust
use crate::sql::operations::comparison::{extract_index_operation, IndexOperation};
```

and merge with the existing comparison import (`compare_field_with_condition, compare_values, evaluate_condition_with_alias`) into one `use` statement.

- [ ] **Step 3.3: Build, prune, test**

Run: `cargo build 2>&1 | grep -cE "^error"` → `0`; `cargo test 2>&1 | tail -5` → baseline parity.

- [ ] **Step 3.4: Commit**

```bash
git add src/sql/operations/comparison.rs src/sql/query_executor.rs
git commit -m "refactor: move IndexOperation and index-extraction helpers to operations::comparison"
```

---

### Task 4: Move aggregate expression evaluator to operations/expression.rs

**Files:**
- Modify: `src/sql/operations/expression.rs`
- Modify: `src/sql/query_executor.rs`

- [ ] **Step 4.1: Move `evaluate_expression_for_aggregate`** (Grep `^fn evaluate_expression_for_aggregate\(`, pre-refactor line 1501) verbatim to end of `operations/expression.rs`, changing `fn` → `pub fn`.

- [ ] **Step 4.2: Fix caller** — `process_aggregate_query` (still in `query_executor.rs` until Task 8). Add to its imports:

```rust
use crate::sql::operations::expression::evaluate_expression_for_aggregate;
```

(merge with the existing `operations::expression` import).

- [ ] **Step 4.3: Build, prune, test** — same expectations as Task 3.

- [ ] **Step 4.4: Commit**

```bash
git add src/sql/operations/expression.rs src/sql/query_executor.rs
git commit -m "refactor: move evaluate_expression_for_aggregate to operations::expression"
```

---

### Task 5: Create operations/timeseries.rs

**Files:**
- Create: `src/sql/operations/timeseries.rs`
- Modify: `src/sql/operations/mod.rs`
- Modify: `src/sql/query_executor.rs`

Functions to move — this is the contiguous block pre-refactor lines 419–1021:

| Function | Grep pattern | Visibility after move |
|---|---|---|
| `find_timeseries_table_by_name` | `^fn find_timeseries_table_by_name` | private |
| `extract_time_range_from_condition` | `^fn extract_time_range_from_condition\(` | private |
| `execute_select_timeseries_query` | `^fn execute_select_timeseries_query\(` | `pub` (router calls it) |
| `downsample_records` | `^fn downsample_records\(` | private |
| `interpolate_missing_window` | `^fn interpolate_missing_window\(` | private |
| `parse_sample_interval` | `^fn parse_sample_interval\(` | private |
| `evaluate_timeseries_expression` | `^fn evaluate_timeseries_expression\(` | private |

- [ ] **Step 5.1: Create `src/sql/operations/timeseries.rs`** with header + imports:

```rust
//! SQL时序表查询操作
//!
//! 该模块包含时序表的SELECT查询执行、降采样和插值逻辑。

use crate::sql::query_parser::Expression;
use crate::sql::utils::estimate_memory_usage;
use crate::sql::{
    check_memory_limit, Condition, QueryExecutionError, ResultSet, SqlQuery, Value as SqlValue,
};
use crate::types::TypedValue;
use crate::{RemDb, TimeSeriesTable};
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
```

- [ ] **Step 5.2: Move the seven functions verbatim** (doc comments + `#[cfg]` gates included — `execute_select_timeseries_query` has one at pre-refactor line 660).

- [ ] **Step 5.3: Register module** — `src/sql/operations/mod.rs` becomes:

```rust
//! SQL Operations Module
//!
//! This module contains SQL operation implementations organized by category.

pub mod comparison;
pub mod ddl;
pub mod expression;
pub mod timeseries;
pub mod vector;
```

- [ ] **Step 5.4: Fix router** — in `query_executor.rs` the Select arm already calls `execute_select_timeseries_query(db, query)`. Add import:

```rust
use crate::sql::operations::timeseries::execute_select_timeseries_query;
```

- [ ] **Step 5.5: Build, prune, test** — `cargo build` zero errors; prune unused imports flagged in either file; `cargo test` baseline parity.

- [ ] **Step 5.6: Commit**

```bash
git add src/sql/operations/timeseries.rs src/sql/operations/mod.rs src/sql/query_executor.rs
git commit -m "refactor: extract timeseries SELECT logic into operations::timeseries"
```

---

### Task 6: Create operations/dml.rs

**Files:**
- Create: `src/sql/operations/dml.rs`
- Modify: `src/sql/operations/mod.rs`
- Modify: `src/sql/query_executor.rs`

Functions to move:

| Function | Grep pattern | Pre-refactor line | Visibility |
|---|---|---|---|
| `execute_insert_query` | `^fn execute_insert_query\(` | 4980 | `pub` |
| `execute_delete_query` | `^fn execute_delete_query\(` | 5535 | `pub` |
| `execute_update_query` | `^fn execute_update_query\(` | 5634 | `pub` |
| `set_field_value` | `^fn set_field_value\(` | 5795 | private |
| `set_field_value_with_depth` | `^fn set_field_value_with_depth\(` | 5807 | private |
| `execute_insert_timeseries_query` | `^fn execute_insert_timeseries_query\(` | 6640 | private (only `execute_insert_query` calls it) |

- [ ] **Step 6.1: Create `src/sql/operations/dml.rs`** with header + starter imports (superset — prune in 6.4):

```rust
//! SQL DML (Data Manipulation Language) 操作
//!
//! 该模块包含INSERT/UPDATE/DELETE查询的执行逻辑，含时序表插入。

use crate::try_lock;

use crate::sql::operations::comparison::compare_field_with_condition;
use crate::sql::operations::expression::{
    evaluate_expression, evaluate_expression_with_depth,
};
use crate::sql::Value as SqlValue;
use crate::sql::{
    check_memory_limit, ComparisonCondition, ComparisonOperator, Condition, QueryExecutionError,
    ResultSet, SqlQuery,
};
use crate::types::{DataType, JsonStorage, TypedValue};
use crate::{MemoryTable, RemDb, Value, MAX_STRING_LEN};
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
```

- [ ] **Step 6.2: Move the six functions verbatim.** `try_lock!` is used inside `execute_insert_timeseries_query` (pre-refactor lines 6766, 6770) — the `use crate::try_lock;` above covers it.

- [ ] **Step 6.3: Fix router** — three arms change from local calls to module calls:

```rust
        crate::sql::QueryType::Insert => dml::execute_insert_query(db, query),
        crate::sql::QueryType::Update => dml::execute_update_query(db, query),
        crate::sql::QueryType::Delete => dml::execute_delete_query(db, query),
```

Add to router imports: `use crate::sql::operations::dml;` (or extend the existing `use crate::sql::operations::ddl;` into a braced import `{ddl, dml}`).

- [ ] **Step 6.4: Register module** — add `pub mod dml;` to `src/sql/operations/mod.rs` (alphabetical: between `comparison` and `ddl`).

- [ ] **Step 6.5: Build, prune, test** — zero errors; prune flagged imports (e.g. drop `evaluate_expression_with_depth`/`Arc`/`DataType` if unused); baseline parity.

- [ ] **Step 6.6: Commit**

```bash
git add src/sql/operations/dml.rs src/sql/operations/mod.rs src/sql/query_executor.rs
git commit -m "refactor: extract INSERT/UPDATE/DELETE logic into operations::dml"
```

---

### Task 7: Expand operations/ddl.rs + extract ALTER TABLE handler

**Files:**
- Modify: `src/sql/operations/ddl.rs`
- Modify: `src/sql/query_executor.rs`

Functions to move (all become `pub fn` — router calls each):

| Function | Grep pattern | Pre-refactor line |
|---|---|---|
| `execute_create_table_query` | `^fn execute_create_table_query\(` | 3112 |
| `execute_show_index_build_status_query` | `^fn execute_show_index_build_status_query\(` | 3689 |
| `execute_reindex_query` | `^fn execute_reindex_query\(` | 3814 |
| `execute_show_tables_query` | `^fn execute_show_tables_query\(` | 3836 |
| `execute_create_index_query` | `^fn execute_create_index_query\(` | 3877 |
| `execute_describe_query` | `^fn execute_describe_query\(` | 4760 |
| `execute_create_time_series_table_query` | `^fn execute_create_time_series_table_query\(` | 6491 |
| `execute_create_checkpoint_query` | `^fn execute_create_checkpoint_query\(` | 7696 |

- [ ] **Step 7.1: Append starter imports to ddl.rs** (prune in 7.5):

```rust
use crate::try_lock;

#[cfg(feature = "log")]
use crate::log::{debug, error, info};
use crate::sql::parse_data_type_with_precision;
use crate::sql::query_parser::Expression;
use crate::sql::Value as SqlValue;
use crate::sql::{Condition, QueryExecutionError, ResultSet, SqlQuery};
use crate::types::{DataType, TypedValue};
use crate::{
    DdlExecutor, IndexType, MemoryTable, RemDb, TableDef, TimeSeriesTable, Value,
};
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
```

- [ ] **Step 7.2: Move the eight handlers verbatim** (their `#[cfg(feature = "log")]` inner gates move with them; `try_lock!` at pre-refactor line 3715 is covered by the import above).

- [ ] **Step 7.3: Extract `execute_alter_table_query`**

In `query_executor.rs`, the `AlterTable` match arm (pre-refactor lines 146–286) contains an inline `for` loop. Replace the whole arm body with a call, and add the new function to `ddl.rs`:

Router arm becomes:

```rust
        crate::sql::QueryType::AlterTable => ddl::execute_alter_table_query(db, query),
```

New function in `ddl.rs` — the body between the braces is the arm's body **copied verbatim** (the `for` loop and everything through `Ok(ResultSet::new(Vec::new()))`):

```rust
/// 执行ALTER TABLE查询
pub fn execute_alter_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 处理ALTER TABLE语句
    // （以下循环体从query_executor.rs的AlterTable分支原样迁移）
    for (field1, field2, pk, not_null, unique, auto_inc, default_val) in &query.table_def {
        // ... verbatim lines from the original arm (DROP COLUMN / ADD / MODIFY / RENAME) ...
    }
    Ok(ResultSet::new(Vec::new()))
}
```

- [ ] **Step 7.4: Fix remaining router arms**:

```rust
        crate::sql::QueryType::Describe => ddl::execute_describe_query(db, query),
        crate::sql::QueryType::CreateTable => ddl::execute_create_table_query(db, query),
        crate::sql::QueryType::CreateTimeSeriesTable => {
            ddl::execute_create_time_series_table_query(db, query)
        }
        crate::sql::QueryType::CreateIndex => ddl::execute_create_index_query(db, query),
        crate::sql::QueryType::ShowIndexBuildStatus => {
            ddl::execute_show_index_build_status_query(db, query)
        }
        crate::sql::QueryType::Reindex => ddl::execute_reindex_query(db, query),
        crate::sql::QueryType::ShowTables => ddl::execute_show_tables_query(db),
        crate::sql::QueryType::CreateCheckpoint => ddl::execute_create_checkpoint_query(db),
```

- [ ] **Step 7.5: Build, prune, test** — zero errors; prune; baseline parity. (Router's `parse_data_type_with_precision` import becomes unused after the ALTER move — remove it.)

- [ ] **Step 7.6: Commit**

```bash
git add src/sql/operations/ddl.rs src/sql/query_executor.rs
git commit -m "refactor: move DDL handlers and ALTER TABLE into operations::ddl"
```

---

### Task 8: Create operations/select.rs (largest — move last)

**Files:**
- Create: `src/sql/operations/select.rs`
- Modify: `src/sql/operations/mod.rs`
- Modify: `src/sql/query_executor.rs`

Items to move:

| Item | Grep pattern | Pre-refactor line | Visibility |
|---|---|---|---|
| `struct QueryStats` (+ `#[derive(Default)]` + doc) | `^struct QueryStats` | 1602 | private |
| `process_aggregate_query` | `^fn process_aggregate_query\(` | 1021 | private |
| `execute_select_query` | `^fn execute_select_query\(` | 1614 | `pub` |
| `add_joined_row` | `^fn add_joined_row\(` | 2062 | private |
| `validate_cross_table_columns` | `^fn validate_cross_table_columns\(` | 2205 | private |
| `execute_select_join_query` | `^fn execute_select_join_query\(` | 2277 | private |
| `process_group_by_query` | `^fn process_group_by_query\(` | 4128 | private |
| `find_table_by_name` | `^fn find_table_by_name` | 4615 | private |
| `validate_expression` | `^fn validate_expression\(` | 4643 | private |
| `validate_columns` | `^fn validate_columns\(` | 4701 | private |
| `execute_expression_query` | `^fn execute_expression_query\(` | 4713 | private |

- [ ] **Step 8.1: Create `src/sql/operations/select.rs`** with header + starter imports (prune in 8.5):

```rust
//! SQL SELECT查询操作
//!
//! 该模块包含SELECT查询执行逻辑：普通查询、JOIN、聚合、GROUP BY与表达式查询。

use crate::sql::operations::comparison::{
    compare_field_with_condition, compare_values, evaluate_condition_with_alias,
    extract_index_operation, IndexOperation,
};
use crate::sql::operations::expression::{
    evaluate_expression, evaluate_expression_without_table, evaluate_expression_for_aggregate,
    execute_function_call,
};
use crate::sql::query_parser::{BinaryOperator, Expression, GroupByClause, JoinType};
use crate::sql::utils::{estimate_memory_usage_for_records, sort_rows_with_alias};
use crate::sql::Value as SqlValue;
use crate::sql::{check_memory_limit, Condition, OrderByClause, QueryExecutionError, ResultSet, SqlQuery};
use crate::types::{JsonStorage, TypedValue};
use crate::{MemoryTable, RemDb, RemDbError, Value, MAX_STRING_LEN};
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::time::Instant;
```

- [ ] **Step 8.2: Move the eleven items verbatim.** Note `std::time::Instant` is used by `execute_select_query` (pre-refactor line 1623).

- [ ] **Step 8.3: Register module + fix router**

`operations/mod.rs` gains `pub mod select;` (alphabetical, after `expression`).

Router Select arm becomes:

```rust
        crate::sql::QueryType::Select => {
            if is_timeseries_table {
                timeseries::execute_select_timeseries_query(db, query)
            } else {
                select::execute_select_query(db, query)
            }
        }
```

Router imports shrink to roughly (prune to exact):

```rust
use crate::model::model_manager::get_global_model_manager;
use crate::sql::operations::{ddl, dml, select, timeseries};
use crate::sql::{QueryExecutionError, ResultSet, SqlQuery};
use crate::RemDb;
use alloc::string::String;
use alloc::vec::Vec;
```

(`use crate::try_lock;`, the `std::time::Instant`, vector/expression/comparison imports, and most `crate::{...}` re-exports are now unused in the router — remove all flagged ones.)

- [ ] **Step 8.4: Sanity-check router contents**

Run: `grep -cE "^(pub )?fn " src/sql/query_executor.rs`
Expected: `1` (only `execute_query`).

- [ ] **Step 8.5: Build, prune, test** — zero errors; prune; baseline parity.

- [ ] **Step 8.6: Commit**

```bash
git add src/sql/operations/select.rs src/sql/operations/mod.rs src/sql/query_executor.rs
git commit -m "refactor: extract SELECT/JOIN/aggregate logic into operations::select; query_executor is now a thin router"
```

---

### Task 9: Final verification vs baseline

**Files:** none modified (verification only).

- [ ] **Step 9.1: Build parity**

Run: `cargo build 2>&1 | grep -cE "^(error|warning)"`
Expected: error count `0`; warning count ≤ baseline (`/tmp/baseline_build.txt`).

- [ ] **Step 9.2: Test parity**

Run: `cargo test 2>&1 | tail -30`
Expected: byte-identical pass/fail set vs `/tmp/baseline_test.txt`.

- [ ] **Step 9.3: Baremetal parity** (only if baseline passed)

Run: `cargo build --no-default-features --features=baremetal 2>&1 | tail -3`
Expected: same outcome as `/tmp/baseline_baremetal.txt`.

- [ ] **Step 9.4: Structure check**

Run: `wc -l src/sql/query_executor.rs src/sql/operations/*.rs src/sql/utils.rs`
Expected: `query_executor.rs` ≈ 350–450 lines; no operations file exceeds ~2,600 lines.

- [ ] **Step 9.5: No-commit-if-clean check**

Run: `git status --short`
Expected: clean working tree (all changes committed in Tasks 2–8).

---

## Self-Review (performed after writing)

1. **Spec coverage**: every item in spec §3's mapping tables appears in exactly one task (utils→T2, comparison→T3, expression→T4, timeseries→T5, dml→T6, ddl+ALTER→T7, select→T8, router→T8, verification→T9). ✔
2. **Placeholder scan**: the only "..." in this plan is inside the Task 7 ALTER function, which explicitly instructs copying verbatim lines from the source arm — the source IS the content. No TBDs. ✔
3. **Type consistency**: `execute_alter_table_query(db, query)` signature in T7 matches the router call in T7.4; `select::execute_select_query` / `timeseries::execute_select_timeseries_query` paths in T8.3 match module declarations in T5.3/T8.3. ✔
