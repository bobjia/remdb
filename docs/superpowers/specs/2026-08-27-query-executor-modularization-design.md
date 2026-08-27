# Design: Modularize query_executor.rs (complete the existing migration)

Date: 2026-08-27
Status: Pending user review
Supersedes: continues `.trae/documents/Modularize query_executor.md` (partially executed)

## 1. Context

`src/sql/query_executor.rs` is 7,752 lines and mixes routing, permission checks, DDL,
DML, SELECT/JOIN, aggregation, time-series logic, and sorting. The repo already contains
a prior plan (`.trae/documents/Modularize query_executor.md`) whose first steps are done:

- `src/sql/error.rs` — `QueryExecutionError` (done)
- `src/sql/utils.rs` — `parse_data_type_with_precision`, `check_memory_limit`, time helpers (done)
- `src/sql/operations/expression.rs` — expression evaluation (done, 1,137 lines)
- `src/sql/operations/comparison.rs` — comparisons, LIKE (done, 1,183 lines)
- `src/sql/operations/vector.rs` — vector distance ops (done, 96 lines)
- `src/sql/operations/ddl.rs` — 5 handlers: drop table, create/use/close/drop database (done, 100 lines)

This design completes the remaining steps: populate `operations/ddl.rs`, create
`operations/dml.rs`, `operations/select.rs`, `operations/timeseries.rs`, and reduce
`query_executor.rs` to a thin router.

**Goal**: finish the migration. Zero behavior change. Verified by the existing test
suite and builds matching baseline.

## 2. Target structure

```
src/sql/
├── mod.rs                  (unchanged: mod decls + pub use execute_query)
├── query_executor.rs       ~400 lines: execute_query router + permission checks
├── error.rs                (done)
├── utils.rs                ~930 lines: existing + sort/estimate/misc helpers
├── functions/              (unchanged)
└── operations/
    ├── mod.rs              + pub mod dml; pub mod select; pub mod timeseries;
    ├── comparison.rs       ~1,350 lines: + IndexOperation, extract_index_operation,
    │                                  extract_indexed_condition
    ├── expression.rs       ~1,240 lines: + evaluate_expression_for_aggregate
    ├── vector.rs           (done)
    ├── ddl.rs              ~1,350 lines: + 8 handlers + execute_alter_table_query
    ├── dml.rs              NEW ~1,950 lines
    ├── select.rs           NEW ~2,560 lines
    └── timeseries.rs       NEW ~545 lines
```

## 3. Function-to-module mapping

All moves are verbatim (body, doc comments, `#[cfg]` gates, `unsafe` blocks unchanged).
Line numbers refer to the current `query_executor.rs`.

### query_executor.rs (keeps)
| Item | Line | Notes |
|---|---|---|
| `execute_query` | 43 | router: permission checks, dispatch, short inline arms (transactions 288-308, CreateModel 313-333, RBAC 334-413) stay inline |
| ALTER TABLE inline block | 146-286 | extracted verbatim into new `execute_alter_table_query` in ddl.rs — the only code-shape change in this refactor |

### operations/ddl.rs (extends existing)
| Function | Line |
|---|---|
| `execute_create_table_query` | 3112 |
| `execute_show_index_build_status_query` | 3689 |
| `execute_reindex_query` | 3814 |
| `execute_show_tables_query` | 3836 |
| `execute_create_index_query` | 3877 |
| `execute_describe_query` | 4760 |
| `execute_create_checkpoint_query` | 7696 |
| `execute_create_time_series_table_query` | 6491 |
| `execute_alter_table_query` | new (from inline block 146-286) |

### operations/dml.rs (new)
| Function | Line |
|---|---|
| `execute_insert_query` | 4980 |
| `execute_insert_timeseries_query` | 6640 |
| `execute_delete_query` | 5535 |
| `execute_update_query` | 5634 |
| `set_field_value` | 5795 |
| `set_field_value_with_depth` | 5807 |

### operations/select.rs (new)
| Function | Line |
|---|---|
| `execute_select_query` | 1614 |
| `struct QueryStats` | 1602 |
| `execute_expression_query` | 4713 |
| `validate_expression` | 4643 |
| `validate_columns` | 4701 |
| `find_table_by_name` | 4615 |
| `add_joined_row` | 2062 |
| `validate_cross_table_columns` | 2205 |
| `execute_select_join_query` | 2277 |
| `process_group_by_query` | 4128 |
| `process_aggregate_query` | 1021 |

### operations/timeseries.rs (new)
| Function | Line |
|---|---|
| `find_timeseries_table_by_name` | 419 |
| `extract_time_range_from_condition` | 433 |
| `execute_select_timeseries_query` | 528 |
| `downsample_records` | 674 |
| `interpolate_missing_window` | 784 |
| `parse_sample_interval` | 875 |
| `evaluate_timeseries_expression` | 963 |

### operations/comparison.rs (extends existing)
| Item | Line |
|---|---|
| `enum IndexOperation` | 4029 |
| `extract_index_operation` | 4037 |
| `extract_indexed_condition` | 3971 |

### operations/expression.rs (extends existing)
| Function | Line |
|---|---|
| `evaluate_expression_for_aggregate` | 1501 |

### utils.rs (extends existing)
| Item | Line | Notes |
|---|---|---|
| `sort_rows` | 7409 | dead code — see §6 |
| `sort_rows_with_alias` | 7080 | used by select.rs |
| `expr_to_order_by_string` | 7725 | used by sort_rows_with_alias |
| `estimate_memory_usage` | 906 | used by timeseries.rs |
| `estimate_memory_usage_for_records` | 913 | used by select.rs |
| `execute_with_timeout` | 921 | dead code — see §6 |
| `get_field_value_from_condition` | 2138 | dead code — see §6 |

## 4. Deviations from the original plan doc

The old doc's line numbers reference a 9,423-line version and predate later additions
(ALTER TABLE, RBAC, checkpoint, reindex, model registration). Deviations, with rationale:

1. **`QueryStats` moves to select.rs** (doc said keep in query_executor.rs). It is used
   only by `execute_select_query`; keeping it in the router would create an inverted
   dependency.
2. **`find_table_by_name`, `find_timeseries_table_by_name`, `validate_expression`,
   `validate_columns` colocate with their consumers** (doc said utils.rs). Each has
   exactly one consumer module. Sort/estimate helpers still go to utils.rs because
   utils.rs is their shared home per the doc and it keeps select.rs smaller.
3. **`execute_alter_table_query` extraction** (not in doc): the doc predates ALTER TABLE
   support. Wrapping the 140-line inline block keeps the router thin, matching the doc's
   intent (~200-line router).
4. **New QueryTypes handled since the doc** (Reindex, CreateCheckpoint, ShowIndexBuildStatus,
   RBAC ops, transactions, CreateModel): DDL-ish handlers go to ddl.rs; short arms stay
   inline in the router.

## 5. Mechanics

- **Verbatim moves**: function bodies, doc comments, and all 47 `#[cfg(feature = ...)]`
  gates move unchanged. No signature changes, no renames (beyond the single ALTER
  extraction).
- **Imports**: each new file starts with the import block it needs (`alloc::*`,
  `crate::sql::{...}` types, `crate::{MemoryTable, RemDb, ...}`). Verify with
  `cargo build` and prune unused imports.
- **Visibility**: operations modules stay `pub mod` (existing pattern; git history shows
  they were made public for benchmark access). Handlers are `pub fn`, matching the
  existing `ddl::execute_drop_table_query` style. Module-internal helpers stay private.
- **Cross-module calls**: `select.rs` uses `crate::sql::operations::comparison::extract_index_operation`;
  `select.rs`/`dml.rs` use `crate::sql::utils::sort_rows_with_alias` etc.
  `query_executor.rs` dispatches via `crate::sql::operations::{ddl, dml, select, timeseries}`.
- **no_std**: `alloc::` imports preserved per module. `std::time::Instant` import moves
  with its consumer unchanged (status quo for baremetal builds is preserved, not fixed).
- **Public API unchanged**: `crate::sql::execute_query` remains the sole public entry
  point; `src/sql/mod.rs` needs no export changes. Nothing outside `src/sql` imports
  from `query_executor` (verified by grep).

## 6. Dead code

`execute_with_timeout`, `get_field_value_from_condition`, and `sort_rows` are private
functions with zero call sites (crate has `#![allow(dead_code)]`, so no warnings today).
They are moved verbatim, not deleted, to keep this refactor purely mechanical. Listed
here as candidates for deletion in a follow-up.

## 7. What does not change

- No behavior, signature, error-type, or public-API changes.
- No logic deduplication (e.g., the 4-way repeated permission-check blocks stay as-is).
- `query_parser.rs`, `functions/`, existing `operations/` code, tests, benches untouched.

## 8. Verification

Baseline (before refactor), then identical after — results must match:

1. `cargo build` — zero new warnings vs baseline
2. `cargo test` — full suite (integration tests in `tests/` heavily exercise
   `execute_query`: database_test, alter_table_test, json_test, like_operator_test, ...)
3. `cargo build --no-default-features --features=baremetal` — only if it passes on
   baseline; parity required, no improvement promised
4. Sanity: `grep -c "fn " src/sql/query_executor.rs` — router holds only `execute_query`

## 9. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Import/visibility errors after split | Mechanical; compiler-driven fix-up; each module gets its own import block |
| Accidental behavior drift during moves | Verbatim-only policy; single exception (ALTER extraction) is a pure wrap of existing code into a function with `(db, query)` params |
| Hidden coupling (e.g., `static` items, macros) | Verified: only 2 non-fn items (`QueryStats`, `IndexOperation`); `try_lock!` is a crate macro, resolves the same from any module |
| Dead code removal temptation | Explicitly out of scope (§6) |
