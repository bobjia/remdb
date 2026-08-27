//! SQL SELECT查询操作
//!
//! 该模块包含SELECT查询执行逻辑：普通查询、JOIN、聚合、GROUP BY与表达式查询。

use std::time::Instant;

#[cfg(feature = "log")]
use crate::log::{debug, error};
use crate::sql::operations::comparison::{
    compare_values, evaluate_condition_with_alias, get_field_value, extract_index_operation,
    IndexOperation,
};
use crate::sql::operations::expression::{
    evaluate_expression, evaluate_expression_for_aggregate, evaluate_expression_without_table,
    execute_function_call,
};
use crate::sql::query_parser::{Expression, GroupByClause, JoinType};
use crate::sql::utils::{estimate_memory_usage_for_records, sort_rows_with_alias};
use crate::sql::{
    check_memory_limit, ComparisonCondition, ComparisonOperator, Condition, QueryExecutionError,
    ResultSet, SqlQuery,
};
use crate::types::{DataType, JsonStorage, TypedValue};
use crate::{MemoryTable, RemDb, RemDbError, Value, MAX_STRING_LEN};
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
/// 执行没有FROM子句的表达式查询
fn execute_expression_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 确定要返回的列表达式
    let columns = query.columns.clone();

    // 生成结果集的列名
    let result_columns = columns
        .iter()
        .map(|expr| match expr {
            Expression::Field { name, alias } => alias.clone().unwrap_or_else(|| name.clone()),
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            }
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            }
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            }
            Expression::LogicalOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "logical_op".to_string())
            }
            Expression::UnaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "unary_op".to_string())
            }
        })
        .collect();

    // 创建结果集
    let mut result_set = ResultSet::new(result_columns);

    // 评估每个表达式
    let mut row_values = Vec::with_capacity(columns.len());
    for expr in &columns {
        let value = evaluate_expression_without_table(db, expr)?;
        row_values.push(value);
    }

    // 添加行到结果集
    result_set.add_row(row_values);

    Ok(result_set)
}

/// 验证列表达式是否有效
fn validate_columns(
    table: &MemoryTable,
    columns: &[Expression],
) -> Result<(), QueryExecutionError> {
    for column in columns {
        validate_expression(table, column)?;
    }

    Ok(())
}

/// 验证表达式中的字段名是否有效
fn validate_expression(table: &MemoryTable, expr: &Expression) -> Result<(), QueryExecutionError> {
    match expr {
        Expression::Field {
            name: field_name, ..
        } => {
            // 跳过对 * 的验证，它是一个特殊情况
            if field_name != "*" {
                // 处理带表别名的字段名，如 "t.id"
                let actual_field_name = if field_name.contains('.') {
                    // 提取点号后面的部分作为实际字段名
                    field_name
                        .split('.')
                        .next_back()
                        .expect("field name must contain '.'")
                } else {
                    // 没有表别名，直接使用字段名
                    field_name
                };

                if !table
                    .def
                    .fields
                    .iter()
                    .any(|field| field.name == *actual_field_name)
                {
                    return Err(QueryExecutionError::FieldNotFound);
                }
            }
        }
        Expression::FunctionCall { args, .. } => {
            // 验证函数参数中的字段名
            for arg in args {
                validate_expression(table, arg)?;
            }
        }
        Expression::Constant { .. } => {
            // 常量值不需要验证
        }
        Expression::BinaryOp { left, right, .. } => {
            // 验证二元操作的左右操作数
            validate_expression(table, left)?;
            validate_expression(table, right)?;
        }
        Expression::LogicalOp { left, right, .. } => {
            // 验证逻辑操作的左右操作数
            validate_expression(table, left)?;
            validate_expression(table, right)?;
        }
        Expression::UnaryOp { operand, .. } => {
            // 验证一元操作的操作数
            validate_expression(table, operand)?;
        }
    }

    Ok(())
}

/// 查找表
fn find_table_by_name<'a>(
    db: &'a RemDb,
    table_name: &str,
) -> Result<&'a MemoryTable, QueryExecutionError> {
    // 先查找普通表
    for table in db.tables.iter().flatten() {
        if table.def.name == table_name {
            return Ok(table);
        }
    }

    // 再查找时序表
    for ts_table_opt in db.time_series_tables.iter() {
        if let Some(ts_table) = ts_table_opt {
            if ts_table.def.base.name == table_name {
                // 时序表也有MemoryTable接口，但需要转换
                // 这里我们返回时序表的内部MemoryTable
                // 注意：这可能需要调整，因为时序表和普通表的结构可能不同
                // 暂时返回TableNotFound，需要进一步处理
                return Err(QueryExecutionError::TableNotFound);
            }
        }
    }

    Err(QueryExecutionError::TableNotFound)
}

/// 处理GROUP BY查询
fn process_group_by_query(
    table: &MemoryTable,
    columns: &[Expression],
    rows_to_process: &[Vec<TypedValue>],
    group_by: &GroupByClause,
    result_set: &mut ResultSet,
) -> Result<(), QueryExecutionError> {
    use alloc::collections::BTreeMap;

    // 定义安全的分组键类型
    struct GroupKey {
        // 使用u64数组作为分组键，每个u64代表一个分组字段的哈希值
        // 这样可以避免直接比较TypedValue
        values: Vec<u64>,
    }

    // 实现必要的trait for GroupKey
    impl PartialEq for GroupKey {
        fn eq(&self, other: &Self) -> bool {
            self.values == other.values
        }
    }

    impl Eq for GroupKey {}

    impl PartialOrd for GroupKey {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.values.partial_cmp(&other.values)
        }
    }

    impl Ord for GroupKey {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.values.cmp(&other.values)
        }
    }

    impl Clone for GroupKey {
        fn clone(&self) -> Self {
            GroupKey {
                values: self.values.clone(),
            }
        }
    }

    // 构建别名映射，将别名解析为原始表达式
    // 例如: TIME_BUCKET('15m', timestamp) AS time_window  =>  "time_window" -> FunctionCall(TIME_BUCKET, ...)
    let mut alias_to_expr: BTreeMap<String, &Expression> = BTreeMap::new();
    for expr in columns {
        let alias = match expr {
            Expression::Field { alias, name } => alias.clone().unwrap_or_else(|| name.clone()),
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            }
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            }
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            }
            Expression::LogicalOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "logical_op".to_string())
            }
            Expression::UnaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "unary_op".to_string())
            }
        };
        alias_to_expr.insert(alias.to_uppercase(), expr);
    }

    // 创建分组映射：GroupKey -> Vec<record_values>
    let mut groups = BTreeMap::new();

    // 简单的哈希函数，用于生成分组键
    fn hash_typed_value(value: &TypedValue) -> u64 {
        unsafe {
            match value.value_type {
                DataType::UInt8 => value.value.u8 as u64,
                DataType::UInt16 => value.value.u16 as u64,
                DataType::UInt32 => value.value.u32 as u64,
                DataType::UInt64 => value.value.u64,
                DataType::Int8 => value.value.i8 as u64,
                DataType::Int16 => value.value.i16 as u64,
                DataType::Int32 => value.value.i32 as u64,
                DataType::Int64 => value.value.i64 as u64,
                DataType::Float32 => value.value.float32.to_bits() as u64,
                DataType::Float64 => value.value.float64.to_bits(),
                DataType::Bool => value.value.bool as u64,
                DataType::Timestamp => value.value.time.value as u64,
                DataType::TimestampTZ => {
                    (value.value.time.value as u64) ^ (value.value.time.tz_offset as u64)
                }
                DataType::Interval => value.value.interval.value as u64,
                DataType::VarChar | DataType::Char | DataType::Text => {
                    // 简单的字符串哈希
                    let s = core::str::from_utf8(&value.value.string).unwrap_or("");
                    let trimmed = s.trim_end_matches(char::from(0));
                    let mut hash = 0u64;
                    for c in trimmed.chars() {
                        hash = hash.wrapping_mul(31).wrapping_add(c as u64);
                    }
                    hash
                }
                DataType::Vector => value.value.vector as u64,
                DataType::Json => {
                    // JSON类型的简单哈希
                    0 // 暂时返回0，实际应用中可能需要更复杂的哈希逻辑
                }
            }
        }
    }

    // 将行数据分组
    for record_values in rows_to_process {
        // 评估每个分组表达式，生成分组键
        let mut key_values = Vec::new();
        for expr in &group_by.expressions {
            // 解析别名：如果GROUP BY表达式是Field且匹配SELECT列的别名，则使用原始表达式
            let resolved_expr = match expr {
                Expression::Field { name, .. } => {
                    let upper = name.to_uppercase();
                    if let Some(original) = alias_to_expr.get(&upper) {
                        *original
                    } else {
                        expr
                    }
                }
                _ => expr,
            };
            let value = evaluate_expression(table, record_values, resolved_expr)?;
            if cfg!(feature = "log") {
                crate::log::debug!(
                    "process_group_by_query: group expr value_type={:?}",
                    value.value_type
                );
            }
            let hash = hash_typed_value(&value);
            key_values.push(hash);
        }

        // 创建安全的分组键
        let group_key = GroupKey { values: key_values };

        // 将记录添加到对应的分组中
        groups
            .entry(group_key)
            .or_insert_with(Vec::new)
            .push(record_values.clone());
    }

    // 处理每个分组
    for (_, group_rows) in groups {
        // 为当前分组创建结果行
        let mut row_data = Vec::with_capacity(columns.len());

        // 评估每个表达式
        for expr in columns {
            match expr {
                Expression::Field {
                    name: field_name, ..
                } => {
                    // 对于简单字段，使用第一个记录的值
                    // 因为GROUP BY查询中，分组字段的值在同一个分组中都是相同的
                    let field_index = table
                        .def
                        .fields
                        .iter()
                        .position(|f| f.name == *field_name)
                        .ok_or(QueryExecutionError::FieldNotFound)?;
                    row_data.push(group_rows[0][field_index].clone());
                }
                Expression::FunctionCall { name, args, .. } => {
                    let upper_name = name.to_uppercase();

                    // 处理非聚合函数（如JSON_EXTRACT、TIME_BUCKET等）：使用分组中第一个记录的值
                    let is_aggregate = matches!(
                        upper_name.as_str(),
                        "COUNT"
                            | "SUM"
                            | "AVG"
                            | "MIN"
                            | "MAX"
                            | "STDDEV"
                            | "VAR"
                            | "STDDEV_SAMP"
                            | "VAR_SAMP"
                            | "MOVING_AVERAGE"
                            | "MOVING_SUM"
                    );
                    if !is_aggregate {
                        let mut arg_values = Vec::with_capacity(args.len());
                        for arg in args {
                            arg_values.push(evaluate_expression(table, &group_rows[0], arg)?);
                        }
                        let result = execute_function_call(name, &arg_values)?;
                        row_data.push(result);
                        continue;
                    }

                    // 为每个聚合函数准备初始值
                    let mut agg_result = match upper_name.as_str() {
                        "COUNT" => TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        },
                        "SUM" => TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        },
                        "AVG" => TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        },
                        "MIN" => TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        },
                        "MAX" => TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        },
                        "STDDEV" | "VAR" | "STDDEV_SAMP" | "VAR_SAMP" => TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        },
                        "MOVING_AVERAGE" | "MOVING_SUM" => TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        },
                        "TIME_BUCKET" => {
                            // TIME_BUCKET函数的处理被移到了前面
                            unreachable!()
                        }
                        _ => {
                            return Err(QueryExecutionError::UnsupportedFunction(name.to_string()))
                        }
                    };

                    // 遍历分组中的所有行，更新聚合结果
                    for record_values in &group_rows {
                        // 评估函数参数
                        let mut arg_values = Vec::with_capacity(args.len());
                        for arg in args {
                            arg_values.push(evaluate_expression(table, record_values, arg)?);
                        }

                        // 更新聚合结果
                        match name.to_uppercase().as_str() {
                            "COUNT" => unsafe {
                                agg_result.value.u64 += 1;
                            },
                            "SUM" => {
                                // 如果是第一个元素，初始化agg_result为正确的类型
                                let is_first = agg_result.value_type == DataType::UInt64
                                    && unsafe { agg_result.value.u64 == 0 };
                                if is_first {
                                    // 直接使用第一个值的类型作为初始类型
                                    agg_result = arg_values[0].clone();
                                } else {
                                    // 确保类型匹配
                                    if agg_result.value_type != arg_values[0].value_type {
                                        return Err(QueryExecutionError::TypeMismatch);
                                    }

                                    // 累加值
                                    unsafe {
                                        match agg_result.value_type {
                                            DataType::UInt8 => {
                                                agg_result.value.u8 += arg_values[0].value.u8
                                            }
                                            DataType::UInt16 => {
                                                agg_result.value.u16 += arg_values[0].value.u16
                                            }
                                            DataType::UInt32 => {
                                                agg_result.value.u32 += arg_values[0].value.u32
                                            }
                                            DataType::UInt64 => {
                                                agg_result.value.u64 += arg_values[0].value.u64
                                            }
                                            DataType::Int8 => {
                                                agg_result.value.i8 += arg_values[0].value.i8
                                            }
                                            DataType::Int16 => {
                                                agg_result.value.i16 += arg_values[0].value.i16
                                            }
                                            DataType::Int32 => {
                                                agg_result.value.i32 += arg_values[0].value.i32
                                            }
                                            DataType::Int64 => {
                                                agg_result.value.i64 += arg_values[0].value.i64
                                            }
                                            DataType::Float32 => {
                                                agg_result.value.float32 +=
                                                    arg_values[0].value.float32
                                            }
                                            DataType::Float64 => {
                                                agg_result.value.float64 +=
                                                    arg_values[0].value.float64
                                            }
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        }
                                    }
                                }
                            }
                            "MIN" => {
                                // 如果是第一个元素，直接使用它作为初始值
                                let is_first = agg_result.value_type == DataType::UInt64
                                    && unsafe { agg_result.value.u64 == 0 };
                                if is_first {
                                    agg_result = arg_values[0].clone();
                                } else {
                                    // 比较并更新最小值
                                    let is_less = unsafe {
                                        match (agg_result.value_type, arg_values[0].value_type) {
                                            (DataType::UInt8, DataType::UInt8) => {
                                                arg_values[0].value.u8 < agg_result.value.u8
                                            }
                                            (DataType::UInt16, DataType::UInt16) => {
                                                arg_values[0].value.u16 < agg_result.value.u16
                                            }
                                            (DataType::UInt32, DataType::UInt32) => {
                                                arg_values[0].value.u32 < agg_result.value.u32
                                            }
                                            (DataType::UInt64, DataType::UInt64) => {
                                                arg_values[0].value.u64 < agg_result.value.u64
                                            }
                                            (DataType::Int8, DataType::Int8) => {
                                                arg_values[0].value.i8 < agg_result.value.i8
                                            }
                                            (DataType::Int16, DataType::Int16) => {
                                                arg_values[0].value.i16 < agg_result.value.i16
                                            }
                                            (DataType::Int32, DataType::Int32) => {
                                                arg_values[0].value.i32 < agg_result.value.i32
                                            }
                                            (DataType::Int64, DataType::Int64) => {
                                                arg_values[0].value.i64 < agg_result.value.i64
                                            }
                                            (DataType::Float32, DataType::Float32) => {
                                                arg_values[0].value.float32
                                                    < agg_result.value.float32
                                            }
                                            (DataType::Float64, DataType::Float64) => {
                                                arg_values[0].value.float64
                                                    < agg_result.value.float64
                                            }
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        }
                                    };
                                    if is_less {
                                        agg_result = arg_values[0].clone();
                                    }
                                }
                            }
                            "MAX" => {
                                // 如果是第一个元素，直接使用它作为初始值
                                let is_first = agg_result.value_type == DataType::UInt64
                                    && unsafe { agg_result.value.u64 == 0 };
                                if is_first {
                                    agg_result = arg_values[0].clone();
                                } else {
                                    // 比较并更新最大值
                                    let is_greater = unsafe {
                                        match (agg_result.value_type, arg_values[0].value_type) {
                                            (DataType::UInt8, DataType::UInt8) => {
                                                arg_values[0].value.u8 > agg_result.value.u8
                                            }
                                            (DataType::UInt16, DataType::UInt16) => {
                                                arg_values[0].value.u16 > agg_result.value.u16
                                            }
                                            (DataType::UInt32, DataType::UInt32) => {
                                                arg_values[0].value.u32 > agg_result.value.u32
                                            }
                                            (DataType::UInt64, DataType::UInt64) => {
                                                arg_values[0].value.u64 > agg_result.value.u64
                                            }
                                            (DataType::Int8, DataType::Int8) => {
                                                arg_values[0].value.i8 > agg_result.value.i8
                                            }
                                            (DataType::Int16, DataType::Int16) => {
                                                arg_values[0].value.i16 > agg_result.value.i16
                                            }
                                            (DataType::Int32, DataType::Int32) => {
                                                arg_values[0].value.i32 > agg_result.value.i32
                                            }
                                            (DataType::Int64, DataType::Int64) => {
                                                arg_values[0].value.i64 > agg_result.value.i64
                                            }
                                            (DataType::Float32, DataType::Float32) => {
                                                arg_values[0].value.float32
                                                    > agg_result.value.float32
                                            }
                                            (DataType::Float64, DataType::Float64) => {
                                                arg_values[0].value.float64
                                                    > agg_result.value.float64
                                            }
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        }
                                    };
                                    if is_greater {
                                        agg_result = arg_values[0].clone();
                                    }
                                }
                            }
                            "AVG" => {
                                // 如果是第一个元素，初始化agg_result为正确的类型
                                let is_first = agg_result.value_type == DataType::UInt64
                                    && unsafe { agg_result.value.u64 == 0 };
                                if is_first {
                                    // 将第一个值转换为float64
                                    let float_val = unsafe {
                                        match arg_values[0].value_type {
                                            DataType::UInt8 => arg_values[0].value.u8 as f64,
                                            DataType::UInt16 => arg_values[0].value.u16 as f64,
                                            DataType::UInt32 => arg_values[0].value.u32 as f64,
                                            DataType::UInt64 => arg_values[0].value.u64 as f64,
                                            DataType::Int8 => arg_values[0].value.i8 as f64,
                                            DataType::Int16 => arg_values[0].value.i16 as f64,
                                            DataType::Int32 => arg_values[0].value.i32 as f64,
                                            DataType::Int64 => arg_values[0].value.i64 as f64,
                                            DataType::Float32 => arg_values[0].value.float32 as f64,
                                            DataType::Float64 => arg_values[0].value.float64,
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        }
                                    };
                                    // 为AVG创建一个float64类型的初始值
                                    agg_result = TypedValue {
                                        value_type: DataType::Float64,
                                        value: Value { float64: float_val },
                                    };
                                } else {
                                    // 累加值
                                    let float_val = unsafe {
                                        match arg_values[0].value_type {
                                            DataType::UInt8 => arg_values[0].value.u8 as f64,
                                            DataType::UInt16 => arg_values[0].value.u16 as f64,
                                            DataType::UInt32 => arg_values[0].value.u32 as f64,
                                            DataType::UInt64 => arg_values[0].value.u64 as f64,
                                            DataType::Int8 => arg_values[0].value.i8 as f64,
                                            DataType::Int16 => arg_values[0].value.i16 as f64,
                                            DataType::Int32 => arg_values[0].value.i32 as f64,
                                            DataType::Int64 => arg_values[0].value.i64 as f64,
                                            DataType::Float32 => arg_values[0].value.float32 as f64,
                                            DataType::Float64 => arg_values[0].value.float64,
                                            _ => return Err(QueryExecutionError::TypeMismatch),
                                        }
                                    };
                                    unsafe {
                                        agg_result.value.float64 += float_val;
                                    }
                                }
                            }
                            _ => {
                                // 其他聚合函数暂时不支持
                                return Err(QueryExecutionError::UnsupportedFunction(
                                    name.to_string(),
                                ));
                            }
                        }
                    }

                    // 对于AVG函数，需要除以计数
                    if name.to_uppercase() == "AVG" {
                        let count = group_rows.len() as f64;
                        if count > 0.0 {
                            unsafe {
                                agg_result.value.float64 /= count;
                            }
                        }
                    }

                    row_data.push(agg_result);
                }
                _ => {
                    return Err(QueryExecutionError::InvalidCondition);
                }
            }
        }

        // 将结果行添加到结果集中
        result_set.add_row(row_data);
    }

    Ok(())
}

/// 执行连接查询（带JOIN子句）
fn execute_select_join_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 从系统表获取查询资源配置
    let (_max_memory_mb, _query_timeout_ms) = crate::get_query_resource_config();

    // 1. 查找主表
    let main_table = find_table_by_name(db, &query.table_name)?;

    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回主表所有列（作为Field表达式）
        let fields = main_table
            .def
            .fields
            .iter()
            .map(|field| Expression::Field {
                name: field.name.to_string(),
                alias: None,
            })
            .collect::<Vec<_>>();

        // TODO: 添加所有连接表的列
        fields
    } else {
        // 返回指定列表达式
        // 验证跨表列引用
        validate_cross_table_columns(
            main_table,
            &query.table_name,
            query.table_alias.as_deref(),
            &query.joins,
            &query.columns,
            db,
        )?;
        query.columns.clone()
    };

    // 3. 生成结果集的列名
    let result_columns = columns
        .iter()
        .map(|expr| match expr {
            Expression::Field { name, alias } => alias.clone().unwrap_or_else(|| name.clone()),
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            }
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            }
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            }
            Expression::LogicalOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "logical_op".to_string())
            }
            Expression::UnaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "unary_op".to_string())
            }
        })
        .collect();

    // 4. 创建结果集
    let mut result_set = ResultSet::new(result_columns);

    // 5. 执行表连接操作
    // 目前仅支持内连接
    // TODO: 支持左连接、右连接和全连接

    // 6. 遍历主表中的所有记录
    unsafe {
        let iterate_result = main_table.iterate(|_main_id, main_record_ptr| {
            // 从主表记录中提取所有字段值
            let mut main_record_values = Vec::with_capacity(main_table.def.fields.len());
            for field in main_table.def.fields.iter() {
                if let Ok(typed_value) = get_field_value(main_table, main_record_ptr, &field.name) {
                    main_record_values.push(typed_value);
                } else {
                    continue; // 跳过错误记录
                }
            }

            // 7. 对于每个主表记录，遍历所有连接表
            for join_clause in &query.joins {
                // 查找连接表
                let join_table = find_table_by_name(db, &join_clause.table_name).unwrap();

                // 标记是否有匹配的连接记录
                let mut has_matching_join = false;

                // 遍历连接表中的所有记录
                join_table
                    .iterate(|_join_id, join_record_ptr| {
                        // 从连接表记录中提取所有字段值
                        let mut join_record_values =
                            Vec::with_capacity(join_table.def.fields.len());
                        for field in join_table.def.fields.iter() {
                            if let Ok(typed_value) =
                                get_field_value(join_table, join_record_ptr, &field.name)
                            {
                                join_record_values.push(typed_value);
                            } else {
                                return true; // 跳过错误记录，继续遍历
                            }
                        }

                        // 8. 评估连接条件
                        let join_condition = &join_clause.on_condition;
                        let mut condition_matches = true;

                        // 处理连接条件
                        if let Condition::Comparison(ComparisonCondition {
                            field: left_field,
                            operator,
                            value: right_value,
                        }) = join_condition
                        {
                            // 处理带表名/别名的字段
                            let (table_name_part, field_name_part) = if left_field.contains('.') {
                                let parts: Vec<&str> = left_field.split('.').collect();
                                (Some(parts[0]), parts[1])
                            } else {
                                (None, left_field.as_str())
                            };

                            // 根据表名确定从哪个记录中获取字段值
                            let (table, record_values) = if let Some(table_name) = table_name_part {
                                if table_name == query.table_name
                                    || Some(table_name) == query.table_alias.as_deref()
                                {
                                    (&main_table, &main_record_values)
                                } else {
                                    (&join_table, &join_record_values)
                                }
                            } else {
                                // 没有指定表名，尝试从主表查找，找不到再从连接表查找
                                if main_table
                                    .def
                                    .fields
                                    .iter()
                                    .any(|f| f.name == field_name_part)
                                {
                                    (&main_table, &main_record_values)
                                } else if join_table
                                    .def
                                    .fields
                                    .iter()
                                    .any(|f| f.name == field_name_part)
                                {
                                    (&join_table, &join_record_values)
                                } else {
                                    return true; // 字段不存在，跳过
                                }
                            };

                            // 查找字段索引
                            let field_index = table
                                .def
                                .fields
                                .iter()
                                .position(|f| f.name == field_name_part)
                                .unwrap();

                            // 获取字段值
                            let field_value = &record_values[field_index];

                            // 比较条件，目前仅支持相等比较
                            if *operator == ComparisonOperator::Equal {
                                // 简单的相等比较，支持更多基本类型
                                condition_matches =
                                    unsafe {
                                        // 使用完整的命名空间来区分SQL解析的Value和数据库存储的Value
                                        match (&field_value.value_type, &right_value) {
                                            (DataType::Int8, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i8 == *v as i8
                                            }
                                            (DataType::Int16, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i16 == *v as i16
                                            }
                                            (DataType::Int32, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i32 == *v as i32
                                            }
                                            (DataType::Int64, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i64 == *v
                                            }
                                            (DataType::UInt8, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u8 == *v as u8
                                            }
                                            (DataType::UInt16, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u16 == *v as u16
                                            }
                                            (DataType::UInt32, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u32 == *v as u32
                                            }
                                            (DataType::UInt64, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u64 == *v as u64
                                            }
                                            (
                                                DataType::VarChar | DataType::Char | DataType::Text,
                                                crate::sql::Value::String(v),
                                            ) => {
                                                let field_str =
                                                    core::str::from_utf8(&field_value.value.string)
                                                        .expect("invalid UTF-8 in field value")
                                                        .trim_end_matches(char::from(0));
                                                field_str == v
                                            }
                                            (DataType::Bool, crate::sql::Value::Boolean(v)) => {
                                                field_value.value.bool == *v
                                            }
                                            (DataType::Float32, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float32 - *v as f32).abs() < 1e-6
                                            }
                                            (DataType::Float64, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float64 - *v).abs() < 1e-12
                                            }
                                            // 支持字段引用比较
                                            (_, crate::sql::Value::Identifier(right_field)) => {
                                                // 右值是字段引用，处理字段到字段的比较
                                                // 处理带表名/别名的右字段
                                                let (right_table_name_part, right_field_name_part) =
                                                    if right_field.contains('.') {
                                                        let parts: Vec<&str> =
                                                            right_field.split('.').collect();
                                                        (Some(parts[0]), parts[1])
                                                    } else {
                                                        (None, right_field.as_str())
                                                    };

                                                // 根据表名确定从哪个记录中获取右字段值
                                                let (right_table, right_record_values) =
                                                    if let Some(table_name) = right_table_name_part
                                                    {
                                                        if table_name == query.table_name
                                                            || Some(table_name)
                                                                == query.table_alias.as_deref()
                                                        {
                                                            (&main_table, &main_record_values)
                                                        } else {
                                                            (&join_table, &join_record_values)
                                                        }
                                                    } else {
                                                        // 没有指定表名，尝试从主表查找，找不到再从连接表查找
                                                        if main_table.def.fields.iter().any(|f| {
                                                            f.name == right_field_name_part
                                                        }) {
                                                            (&main_table, &main_record_values)
                                                        } else if join_table.def.fields.iter().any(
                                                            |f| f.name == right_field_name_part,
                                                        ) {
                                                            (&join_table, &join_record_values)
                                                        } else {
                                                            return true; // 字段不存在，跳过
                                                        }
                                                    };

                                                // 查找右字段索引
                                                let right_field_index = right_table
                                                    .def
                                                    .fields
                                                    .iter()
                                                    .position(|f| f.name == right_field_name_part)
                                                    .unwrap();

                                                // 获取右字段值
                                                let right_field_value =
                                                    &right_record_values[right_field_index];

                                                // 使用compare_values函数比较两个字段值
                                                compare_values(field_value, right_field_value)
                                            }
                                            _ => false, // 不支持的类型比较
                                        }
                                    };
                            } else {
                                condition_matches = false; // 只支持相等条件
                            }
                        }

                        // 9. 根据连接类型和条件匹配情况，合并记录并添加到结果集
                        if condition_matches {
                            // 合并主表和连接表的记录值
                            let mut combined_values = main_record_values.clone();
                            combined_values.extend(join_record_values.clone());

                            // 计算所有列表达式的值
                            let mut row_data = Vec::with_capacity(columns.len());
                            for expr in &columns {
                                // 支持跨表字段引用
                                match expr {
                                    Expression::Field { name, .. } => {
                                        // 处理带表名/别名的字段
                                        let (_table_name_part, field_name_part) =
                                            if name.contains('.') {
                                                let parts: Vec<&str> = name.split('.').collect();
                                                (Some(parts[0]), parts[1])
                                            } else {
                                                (None, name.as_str())
                                            };

                                        // 尝试从主表获取字段
                                        if let Some(field_index) = main_table
                                            .def
                                            .fields
                                            .iter()
                                            .position(|f| f.name == field_name_part)
                                        {
                                            row_data.push(main_record_values[field_index].clone());
                                        }
                                        // 尝试从连接表获取字段
                                        else if let Some(field_index) = join_table
                                            .def
                                            .fields
                                            .iter()
                                            .position(|f| f.name == field_name_part)
                                        {
                                            row_data.push(join_record_values[field_index].clone());
                                        } else {
                                            // 字段不存在，添加默认值
                                            let default_value = TypedValue {
                                                value_type: DataType::Int64,
                                                value: Value { i64: 0 },
                                            };
                                            row_data.push(default_value);
                                        }
                                    }
                                    _ => {
                                        // 其他表达式类型，添加默认值
                                        let default_value = TypedValue {
                                            value_type: DataType::Int64,
                                            value: Value { i64: 0 },
                                        };
                                        row_data.push(default_value);
                                    }
                                }
                            }

                            // 添加到结果集
                            result_set.add_row(row_data);
                            has_matching_join = true;
                        }

                        true // 继续遍历连接表
                    })
                    .expect("iterator should not fail");

                // 左连接和全连接处理：如果没有匹配的连接记录，仍然需要添加主表记录
                if (join_clause.join_type == JoinType::Left
                    || join_clause.join_type == JoinType::Full)
                    && !has_matching_join
                {
                    // 创建连接表的默认值记录
                    let mut join_default_values = Vec::with_capacity(join_table.def.fields.len());
                    for field in join_table.def.fields.iter() {
                        // 根据字段类型创建默认值
                        let default_value = match field.data_type {
                            DataType::Int8 => TypedValue {
                                value_type: DataType::Int8,
                                value: Value { i8: 0 },
                            },
                            DataType::Int16 => TypedValue {
                                value_type: DataType::Int16,
                                value: Value { i16: 0 },
                            },
                            DataType::Int32 => TypedValue {
                                value_type: DataType::Int32,
                                value: Value { i32: 0 },
                            },
                            DataType::Int64 => TypedValue {
                                value_type: DataType::Int64,
                                value: Value { i64: 0 },
                            },
                            DataType::UInt8 => TypedValue {
                                value_type: DataType::UInt8,
                                value: Value { u8: 0 },
                            },
                            DataType::UInt16 => TypedValue {
                                value_type: DataType::UInt16,
                                value: Value { u16: 0 },
                            },
                            DataType::UInt32 => TypedValue {
                                value_type: DataType::UInt32,
                                value: Value { u32: 0 },
                            },
                            DataType::UInt64 => TypedValue {
                                value_type: DataType::UInt64,
                                value: Value { u64: 0 },
                            },
                            DataType::Float32 => TypedValue {
                                value_type: DataType::Float32,
                                value: Value { float32: 0.0 },
                            },
                            DataType::Float64 => TypedValue {
                                value_type: DataType::Float64,
                                value: Value { float64: 0.0 },
                            },
                            DataType::Bool => TypedValue {
                                value_type: DataType::Bool,
                                value: Value { bool: false },
                            },
                            DataType::VarChar | DataType::Char | DataType::Text => {
                                let buf = [0; MAX_STRING_LEN];
                                TypedValue {
                                    value_type: DataType::VarChar,
                                    value: Value { string: buf },
                                }
                            }
                            DataType::Timestamp => TypedValue {
                                value_type: DataType::Timestamp,
                                value: Value { u64: 0 },
                            },
                            DataType::TimestampTZ => TypedValue {
                                value_type: DataType::TimestampTZ,
                                value: Value { u64: 0 },
                            },
                            DataType::Interval => TypedValue {
                                value_type: DataType::Interval,
                                value: Value { i64: 0 },
                            },
                            DataType::Vector => TypedValue {
                                value_type: DataType::Vector,
                                value: Value { u64: 0 },
                            },
                            DataType::Json => TypedValue {
                                value_type: DataType::Json,
                                value: Value {
                                    json_storage: JsonStorage::Null,
                                },
                            },
                        };
                        join_default_values.push(default_value);
                    }

                    // 添加左连接的默认记录
                    add_joined_row(
                        &mut result_set,
                        &columns,
                        main_table,
                        &main_record_values,
                        join_table,
                        &join_default_values,
                    )
                    .expect("get_field_value should not fail")
                }
            }

            true // 继续遍历主表
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }

    // 处理 RIGHT JOIN 和 FULL JOIN：遍历连接表，添加没有匹配主表的记录
    for join_clause in &query.joins {
        if join_clause.join_type == JoinType::Right || join_clause.join_type == JoinType::Full {
            // 查找连接表
            let join_table = find_table_by_name(db, &join_clause.table_name)?;

            unsafe {
                // 遍历连接表中的所有记录
                let iterate_result = join_table.iterate(|_join_id, join_record_ptr| {
                    // 从连接表记录中提取所有字段值
                    let mut join_record_values = Vec::with_capacity(join_table.def.fields.len());
                    for field in join_table.def.fields.iter() {
                        if let Ok(typed_value) =
                            get_field_value(join_table, join_record_ptr, &field.name)
                        {
                            join_record_values.push(typed_value);
                        } else {
                            return true; // 跳过错误记录，继续遍历
                        }
                    }

                    // 标记是否有匹配的主表记录
                    let mut has_matching_main = false;

                    // 遍历主表中的所有记录
                    main_table
                        .iterate(|_main_id, main_record_ptr| {
                            // 从主表记录中提取所有字段值
                            let mut main_record_values =
                                Vec::with_capacity(main_table.def.fields.len());
                            for field in main_table.def.fields.iter() {
                                if let Ok(typed_value) =
                                    get_field_value(main_table, main_record_ptr, &field.name)
                                {
                                    main_record_values.push(typed_value);
                                } else {
                                    return true; // 跳过错误记录，继续遍历
                                }
                            }

                            // 评估连接条件
                            let join_condition = &join_clause.on_condition;
                            let mut condition_matches = true;

                            // 处理连接条件
                            if let Condition::Comparison(ComparisonCondition {
                                field: left_field,
                                operator,
                                value: right_value,
                            }) = join_condition
                            {
                                // 处理带表名/别名的字段
                                let (table_name_part, field_name_part) = if left_field.contains('.')
                                {
                                    let parts: Vec<&str> = left_field.split('.').collect();
                                    (Some(parts[0]), parts[1])
                                } else {
                                    (None, left_field.as_str())
                                };

                                // 根据表名确定从哪个记录中获取字段值
                                let (table, record_values) =
                                    if let Some(table_name) = table_name_part {
                                        if table_name == query.table_name
                                            || Some(table_name) == query.table_alias.as_deref()
                                        {
                                            (&main_table, &main_record_values)
                                        } else {
                                            (&join_table, &join_record_values)
                                        }
                                    } else {
                                        // 没有指定表名，尝试从主表查找，找不到再从连接表查找
                                        if main_table
                                            .def
                                            .fields
                                            .iter()
                                            .any(|f| f.name == field_name_part)
                                        {
                                            (&main_table, &main_record_values)
                                        } else if join_table
                                            .def
                                            .fields
                                            .iter()
                                            .any(|f| f.name == field_name_part)
                                        {
                                            (&join_table, &join_record_values)
                                        } else {
                                            return true; // 字段不存在，跳过
                                        }
                                    };

                                // 查找字段索引
                                let field_index = table
                                    .def
                                    .fields
                                    .iter()
                                    .position(|f| f.name == field_name_part)
                                    .unwrap();

                                // 获取字段值
                                let field_value = &record_values[field_index];

                                // 比较条件，目前仅支持相等比较
                                if *operator == ComparisonOperator::Equal {
                                    // 简单的相等比较，支持更多基本类型
                                    condition_matches = unsafe {
                                        // 使用完整的命名空间来区分SQL解析的Value和数据库存储的Value
                                        match (&field_value.value_type, &right_value) {
                                            (DataType::Int8, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i8 == *v as i8
                                            }
                                            (DataType::Int16, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i16 == *v as i16
                                            }
                                            (DataType::Int32, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i32 == *v as i32
                                            }
                                            (DataType::Int64, crate::sql::Value::Integer(v)) => {
                                                field_value.value.i64 == *v
                                            }
                                            (DataType::UInt8, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u8 == *v as u8
                                            }
                                            (DataType::UInt16, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u16 == *v as u16
                                            }
                                            (DataType::UInt32, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u32 == *v as u32
                                            }
                                            (DataType::UInt64, crate::sql::Value::Integer(v)) => {
                                                field_value.value.u64 == *v as u64
                                            }
                                            (
                                                DataType::VarChar | DataType::Char | DataType::Text,
                                                crate::sql::Value::String(v),
                                            ) => {
                                                let field_str =
                                                    core::str::from_utf8(&field_value.value.string)
                                                        .expect("invalid UTF-8 in field value")
                                                        .trim_end_matches(char::from(0));
                                                field_str == v
                                            }
                                            (DataType::Bool, crate::sql::Value::Boolean(v)) => {
                                                field_value.value.bool == *v
                                            }
                                            (DataType::Float32, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float32 - *v as f32).abs() < 1e-6
                                            }
                                            (DataType::Float64, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float64 - *v).abs() < 1e-12
                                            }
                                            // 支持字段引用比较
                                            (_, crate::sql::Value::Identifier(right_field)) => {
                                                // 右值是字段引用，处理字段到字段的比较
                                                // 处理带表名/别名的右字段
                                                let (right_table_name_part, right_field_name_part) =
                                                    if right_field.contains('.') {
                                                        let parts: Vec<&str> =
                                                            right_field.split('.').collect();
                                                        (Some(parts[0]), parts[1])
                                                    } else {
                                                        (None, right_field.as_str())
                                                    };

                                                // 根据表名确定从哪个记录中获取右字段值
                                                let (right_table, right_record_values) =
                                                    if let Some(table_name) = right_table_name_part
                                                    {
                                                        if table_name == query.table_name
                                                            || Some(table_name)
                                                                == query.table_alias.as_deref()
                                                        {
                                                            (&main_table, &main_record_values)
                                                        } else {
                                                            (&join_table, &join_record_values)
                                                        }
                                                    } else {
                                                        // 没有指定表名，尝试从主表查找，找不到再从连接表查找
                                                        if main_table.def.fields.iter().any(|f| {
                                                            f.name == right_field_name_part
                                                        }) {
                                                            (&main_table, &main_record_values)
                                                        } else if join_table.def.fields.iter().any(
                                                            |f| f.name == right_field_name_part,
                                                        ) {
                                                            (&join_table, &join_record_values)
                                                        } else {
                                                            // 字段不存在，使用主表作为默认
                                                            (&main_table, &main_record_values)
                                                        }
                                                    };

                                                // 查找右字段索引
                                                if let Some(right_field_index) =
                                                    right_table.def.fields.iter().position(|f| {
                                                        f.name == right_field_name_part
                                                    })
                                                {
                                                    // 获取右字段值
                                                    let right_field_value =
                                                        &right_record_values[right_field_index];

                                                    // 使用compare_values函数比较两个字段值
                                                    compare_values(field_value, right_field_value)
                                                } else {
                                                    false
                                                }
                                            }
                                            _ => false, // 不支持的类型比较
                                        }
                                    };
                                } else {
                                    condition_matches = false; // 只支持相等条件
                                }
                            }

                            if condition_matches {
                                has_matching_main = true;
                                return false; // 找到匹配，停止遍历主表
                            }

                            true // 继续遍历主表
                        })
                        .expect("iterator should not fail");

                    // 如果没有匹配的主表记录，添加右连接或全连接的默认记录
                    if !has_matching_main {
                        // 创建主表的默认值记录
                        let mut main_default_values =
                            Vec::with_capacity(main_table.def.fields.len());
                        for field in main_table.def.fields.iter() {
                            // 根据字段类型创建默认值
                            let default_value = match field.data_type {
                                DataType::Int8 => TypedValue {
                                    value_type: DataType::Int8,
                                    value: Value { i8: 0 },
                                },
                                DataType::Int16 => TypedValue {
                                    value_type: DataType::Int16,
                                    value: Value { i16: 0 },
                                },
                                DataType::Int32 => TypedValue {
                                    value_type: DataType::Int32,
                                    value: Value { i32: 0 },
                                },
                                DataType::Int64 => TypedValue {
                                    value_type: DataType::Int64,
                                    value: Value { i64: 0 },
                                },
                                DataType::UInt8 => TypedValue {
                                    value_type: DataType::UInt8,
                                    value: Value { u8: 0 },
                                },
                                DataType::UInt16 => TypedValue {
                                    value_type: DataType::UInt16,
                                    value: Value { u16: 0 },
                                },
                                DataType::UInt32 => TypedValue {
                                    value_type: DataType::UInt32,
                                    value: Value { u32: 0 },
                                },
                                DataType::UInt64 => TypedValue {
                                    value_type: DataType::UInt64,
                                    value: Value { u64: 0 },
                                },
                                DataType::Float32 => TypedValue {
                                    value_type: DataType::Float32,
                                    value: Value { float32: 0.0 },
                                },
                                DataType::Float64 => TypedValue {
                                    value_type: DataType::Float64,
                                    value: Value { float64: 0.0 },
                                },
                                DataType::Bool => TypedValue {
                                    value_type: DataType::Bool,
                                    value: Value { bool: false },
                                },
                                DataType::VarChar | DataType::Char | DataType::Text => {
                                    let buf = [0; MAX_STRING_LEN];
                                    TypedValue {
                                        value_type: DataType::VarChar,
                                        value: Value { string: buf },
                                    }
                                }
                                DataType::Timestamp => TypedValue {
                                    value_type: DataType::Timestamp,
                                    value: Value { u64: 0 },
                                },
                                DataType::TimestampTZ => TypedValue {
                                    value_type: DataType::TimestampTZ,
                                    value: Value { u64: 0 },
                                },
                                DataType::Interval => TypedValue {
                                    value_type: DataType::Interval,
                                    value: Value { i64: 0 },
                                },
                                DataType::Vector => TypedValue {
                                    value_type: DataType::Vector,
                                    value: Value { u64: 0 },
                                },
                                DataType::Json => TypedValue {
                                    value_type: DataType::Json,
                                    value: Value {
                                        json_storage: JsonStorage::Null,
                                    },
                                },
                            };
                            main_default_values.push(default_value);
                        }

                        // 计算所有列表达式的值
                        let mut row_data = Vec::with_capacity(columns.len());
                        for expr in &columns {
                            // 支持跨表字段引用
                            match expr {
                                Expression::Field { name, .. } => {
                                    // 处理带表名/别名的字段
                                    let (_table_name_part, field_name_part) = if name.contains('.')
                                    {
                                        let parts: Vec<&str> = name.split('.').collect();
                                        (Some(parts[0]), parts[1])
                                    } else {
                                        (None, name.as_str())
                                    };

                                    // 尝试从主表获取字段
                                    if let Some(field_index) = main_table
                                        .def
                                        .fields
                                        .iter()
                                        .position(|f| f.name == field_name_part)
                                    {
                                        row_data.push(main_default_values[field_index].clone());
                                    }
                                    // 尝试从连接表获取字段
                                    else if let Some(field_index) = join_table
                                        .def
                                        .fields
                                        .iter()
                                        .position(|f| f.name == field_name_part)
                                    {
                                        row_data.push(join_record_values[field_index].clone());
                                    } else {
                                        // 字段不存在，添加默认值
                                        let default_value = TypedValue {
                                            value_type: DataType::Int64,
                                            value: Value { i64: 0 },
                                        };
                                        row_data.push(default_value);
                                    }
                                }
                                _ => {
                                    // 其他表达式类型，添加默认值
                                    let default_value = TypedValue {
                                        value_type: DataType::Int64,
                                        value: Value { i64: 0 },
                                    };
                                    row_data.push(default_value);
                                }
                            }
                        }

                        // 添加到结果集
                        result_set.add_row(row_data);
                    }

                    true // 继续遍历连接表
                });
                iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
            }
        }
    }

    // 10. 应用LIMIT限制
    if let Some(limit) = query.limit {
        if result_set.rows.len() > limit {
            result_set.rows.truncate(limit);
        }
    }

    Ok(result_set)
}

/// 验证跨表列引用
fn validate_cross_table_columns(
    main_table: &MemoryTable,
    main_table_name: &str,
    main_table_alias: Option<&str>,
    joins: &[crate::sql::query_parser::JoinClause],
    columns: &[Expression],
    db: &RemDb,
) -> Result<(), QueryExecutionError> {
    // 构建表名到表定义的映射
    let mut table_map = std::collections::HashMap::new();

    // 添加主表
    table_map.insert(main_table_name.to_string(), main_table);
    if let Some(alias) = main_table_alias {
        table_map.insert(alias.to_string(), main_table);
    }

    // 添加所有连接表
    for join_clause in joins {
        let join_table = find_table_by_name(db, &join_clause.table_name)?;
        table_map.insert(join_clause.table_name.clone(), join_table);
        if let Some(alias) = &join_clause.table_alias {
            table_map.insert(alias.clone(), join_table);
        }
    }

    // 验证每个列表达式
    for expr in columns {
        if let Expression::Field { name, .. } = expr {
            // 处理带表名/别名的字段
            if name.contains('.') {
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() != 2 {
                    return Err(QueryExecutionError::FieldNotFound);
                }
                let table_name = parts[0];
                let field_name = parts[1];

                // 检查表是否存在
                if let Some(table) = table_map.get(table_name) {
                    // 检查字段是否存在于表中
                    if !table.def.fields.iter().any(|f| f.name == field_name) {
                        return Err(QueryExecutionError::FieldNotFound);
                    }
                } else {
                    return Err(QueryExecutionError::TableNotFound);
                }
            } else {
                // 没有指定表名的字段，需要在所有表中查找
                let mut found = false;
                for table in table_map.values() {
                    if table
                        .def
                        .fields
                        .iter()
                        .any(|f| f.name.as_str() == name.as_str())
                    {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(QueryExecutionError::FieldNotFound);
                }
            }
        }
    }

    Ok(())
}

/// 辅助函数：添加连接行到结果集
fn add_joined_row(
    result_set: &mut ResultSet,
    columns: &[Expression],
    main_table: &MemoryTable,
    main_record_values: &[TypedValue],
    join_table: &MemoryTable,
    join_record_values: &[TypedValue],
) -> Result<(), QueryExecutionError> {
    // 计算所有列表达式的值
    let mut row_data = Vec::with_capacity(columns.len());
    for expr in columns {
        // 支持跨表字段引用
        match expr {
            Expression::Field { name, .. } => {
                // 处理带表名/别名的字段
                let (_table_name_part, field_name_part) = if name.contains('.') {
                    let parts: Vec<&str> = name.split('.').collect();
                    (Some(parts[0]), parts[1])
                } else {
                    (None, name.as_str())
                };

                // 尝试从主表获取字段
                if let Some(field_index) = main_table
                    .def
                    .fields
                    .iter()
                    .position(|f| f.name == field_name_part)
                {
                    #[cfg(feature = "log")]
                    debug!(
                        "get_field_value: found field '{}' at index {} in main_table",
                        field_name_part, field_index
                    );
                    row_data.push(main_record_values[field_index].clone());
                }
                // 尝试从连接表获取字段
                else if let Some(field_index) = join_table
                    .def
                    .fields
                    .iter()
                    .position(|f| f.name == field_name_part)
                {
                    row_data.push(join_record_values[field_index].clone());
                } else {
                    // 字段不存在，添加默认值
                    let default_value = TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: 0 },
                    };
                    #[cfg(feature = "log")]
                    debug!(
                        "get_field_value: field '{}' not found, using default value",
                        field_name_part
                    );
                    row_data.push(default_value);
                }
            }
            _ => {
                // 其他表达式类型，添加默认值
                let default_value = TypedValue {
                    value_type: DataType::Int64,
                    value: Value { i64: 0 },
                };
                row_data.push(default_value);
            }
        }
    }

    // 添加到结果集
    result_set.add_row(row_data);
    Ok(())
}

/// 执行SELECT查询
fn execute_select_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 从系统表获取查询资源配置
    let (max_memory_mb, query_timeout_ms) = crate::get_query_resource_config();
    let _query_timeout_ms = Some(query_timeout_ms as u64);

    // 开始计时
    let start_time = Instant::now();
    let mut _stats = QueryStats::default();

    // 检查是否有FROM子句（如果没有FROM子句，则执行表达式查询）
    if query.table_name.is_empty() {
        // 没有FROM子句，执行表达式查询
        return execute_expression_query(db, query);
    }

    // 检查是否有JOIN子句
    if !query.joins.is_empty() {
        // 有JOIN子句，执行连接查询
        // 计算执行时间
        let end_time = Instant::now();
        let execution_time = end_time.duration_since(start_time).as_micros() as u64;

        // 输出查询执行统计信息
        #[cfg(feature = "log")]
        {
            info!("Query execution stats:");
            info!("  Used index: false");
            info!("  Scanned records: 0");
            info!("  Matched records: 0");
            info!("  Execution time: {}μs", execution_time);
        }

        return execute_select_join_query(db, query);
    }

    // 没有JOIN子句，执行简单查询
    // 1. 查找要查询的表（同时尝试获取索引）
    let (table, maybe_index) = if let Some(where_clause) = &query.where_clause {
        // 检查是否有WHERE条件可以使用索引
        if let Some((indexed_field, index_operation)) =
            extract_index_operation(&where_clause.condition)
        {
            // 尝试获取表和索引
            match db.get_table_and_secondary_index_mut_by_name(&query.table_name) {
                Ok((table_ref, index_ref)) => {
                    // 只有当WHERE条件中的字段确实是索引字段时才使用索引，
                    // 否则（例如对非索引列的等值条件）会错误地走向量/二级索引查找
                    let field_is_indexed = table_ref
                        .def
                        .secondary_index
                        .as_ref()
                        .map(|indices| {
                            indices.iter().any(|&idx| {
                                table_ref
                                    .def
                                    .fields
                                    .get(idx)
                                    .map(|f| f.name == indexed_field)
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false);

                    if field_is_indexed {
                        // 成功获取表和索引，且WHERE字段与索引字段匹配
                        (table_ref, Some((index_ref, indexed_field, index_operation)))
                    } else {
                        // WHERE字段不是索引字段，退回全表扫描
                        (table_ref, None)
                    }
                }
                Err(_) => {
                    // 索引不存在，只获取表
                    let table = find_table_by_name(db, &query.table_name)?;
                    (table, None)
                }
            }
        } else {
            // 没有可索引的条件，只获取表
            let table = find_table_by_name(db, &query.table_name)?;
            (table, None)
        }
    } else {
        // 没有WHERE条件，只获取表
        let table = find_table_by_name(db, &query.table_name)?;
        (table, None)
    };

    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回所有列（作为Field表达式）
        #[cfg(feature = "log")]
        info!(
            "DEBUG: SELECT * query, table fields: {:?}",
            table.def.fields.iter().map(|f| &f.name).collect::<Vec<_>>()
        );
        table
            .def
            .fields
            .iter()
            .map(|field| Expression::Field {
                name: field.name.to_string(),
                alias: None,
            })
            .collect()
    } else {
        // 返回指定列表达式
        validate_columns(table, &query.columns)?;
        query.columns.clone()
    };

    // 3. 生成结果集的列名
    let result_columns = columns
        .iter()
        .map(|expr| match expr {
            Expression::Field { name, alias } => alias.clone().unwrap_or_else(|| name.clone()),
            Expression::FunctionCall { alias, name, .. } => {
                alias.clone().unwrap_or_else(|| name.clone())
            }
            Expression::Constant { alias, .. } => {
                alias.clone().unwrap_or_else(|| "constant".to_string())
            }
            Expression::BinaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "binary_op".to_string())
            }
            Expression::LogicalOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "logical_op".to_string())
            }
            Expression::UnaryOp { alias, .. } => {
                alias.clone().unwrap_or_else(|| "unary_op".to_string())
            }
        })
        .collect();

    // 4. 创建别名映射
    let mut alias_map = alloc::collections::BTreeMap::new();
    for expr in &columns {
        match expr {
            Expression::Field { name: _, alias } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::FunctionCall { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::Constant { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::BinaryOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::LogicalOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
            Expression::UnaryOp { alias, .. } => {
                if let Some(alias) = alias {
                    alias_map.insert(alias.clone(), expr);
                }
            }
        }
    }

    // 5. 创建结果集
    let mut result_set = ResultSet::new(result_columns);

    // 6. 尝试使用索引获取记录，而不是全表扫描
    let mut all_records = Vec::with_capacity(table.def.max_records);
    let mut use_index = false;

    // 检查是否有WHERE条件可以使用索引
    // 使用之前获取的索引（如果存在）
    if let Some((secondary_index, _indexed_field, index_operation)) = maybe_index {
        match index_operation {
            IndexOperation::Equal(index_value) => {
                // 相等查询 - 使用索引
                unsafe {
                    match secondary_index.find(index_value.as_ptr(), index_value.len()) {
                        Ok(record_id) => {
                            // 找到记录，只处理这一条
                            let record_ptr = table.get_record_ptr(record_id as usize);
                            if !record_ptr.is_null() {
                                let mut record_values = Vec::with_capacity(table.def.fields.len());
                                for field in table.def.fields.iter() {
                                    match get_field_value(table, record_ptr, &field.name) {
                                        Ok(typed_value) => record_values.push(typed_value),
                                        Err(_) => break, // 跳过错误记录
                                    }
                                }
                                if record_values.len() == table.def.fields.len() {
                                    all_records.push(record_values);
                                }
                            }
                            use_index = true;
                            _stats.used_index = true;
                            _stats.scanned_records = 1;
                        }
                        Err(RemDbError::RecordNotFound) => {
                            // 没有找到记录，继续使用全表扫描
                        }
                        _ => {
                            // 索引查找失败，继续使用全表扫描
                        }
                    }
                }
            }
            IndexOperation::Range(start_value, end_value) => {
                // 范围查询 - 使用索引
                unsafe {
                    match secondary_index.find_range(
                        start_value.as_ptr(),
                        start_value.len(),
                        end_value.as_ptr(),
                        end_value.len(),
                    ) {
                        Ok(record_id) => {
                            // 找到记录，只处理这一条
                            let record_ptr = table.get_record_ptr(record_id as usize);
                            if !record_ptr.is_null() {
                                let mut record_values = Vec::with_capacity(table.def.fields.len());
                                for field in table.def.fields.iter() {
                                    match get_field_value(table, record_ptr, &field.name) {
                                        Ok(typed_value) => record_values.push(typed_value),
                                        Err(_) => break, // 跳过错误记录
                                    }
                                }
                                if record_values.len() == table.def.fields.len() {
                                    all_records.push(record_values);
                                }
                            }
                            use_index = true;
                            _stats.used_index = true;
                            _stats.scanned_records = 1;
                        }
                        Err(RemDbError::RecordNotFound) => {
                            // 没有找到记录，继续使用全表扫描
                        }
                        _ => {
                            // 索引查找失败，继续使用全表扫描
                        }
                    }
                }
            }
        }
    }

    // 如果没有使用索引，执行全表扫描
    if !use_index {
        unsafe {
            // 遍历表中的所有记录，收集所有记录
            let iterate_result = table.iterate(|_id, record_ptr| {
                // 检查记录指针是否为null
                if record_ptr.is_null() {
                    return true; // 跳过空记录，继续遍历
                }

                // 直接从记录中提取字段值，创建行数据
                let mut record_values = Vec::with_capacity(table.def.fields.len());
                for field in table.def.fields.iter() {
                    match get_field_value(table, record_ptr, &field.name) {
                        Ok(typed_value) => record_values.push(typed_value),
                        Err(_) => return true, // 跳过错误记录，继续遍历
                    }
                }

                // 将记录值添加到向量中
                all_records.push(record_values);

                _stats.scanned_records += 1;
                true // 继续遍历
            });
            iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 更新匹配的记录数
    _stats.matched_records = all_records.len();

    // 内存使用检查
    let estimated_memory = estimate_memory_usage_for_records(&all_records);
    check_memory_limit(estimated_memory, Some(max_memory_mb))?;

    // 7. 计算每个记录的表达式值
    let mut records_with_expr_values = Vec::with_capacity(all_records.len());
    for record_values in &all_records {
        // 计算表达式值
        let mut expr_values = Vec::with_capacity(columns.len());
        for expr in &columns {
            // Evaluate the expression
            let value = evaluate_expression(table, record_values, expr)?;
            expr_values.push(value);
        }

        // 将记录值和表达式值组合起来
        records_with_expr_values.push((record_values.clone(), expr_values));
    }

    // 8. 应用WHERE条件过滤记录
    let mut filtered_records = Vec::with_capacity(records_with_expr_values.len());
    for (record_values, expr_values) in records_with_expr_values {
        // 检查记录是否符合WHERE条件
        let mut matches = true;
        if let Some(where_clause) = &query.where_clause {
            matches = unsafe {
                evaluate_condition_with_alias(
                    table,
                    &record_values,
                    &columns,
                    &expr_values,
                    &where_clause.condition,
                    &alias_map,
                )
            };
        }

        if matches {
            filtered_records.push((record_values, expr_values));
        }
    }

    // 9. 如果有ORDER BY子句，对记录进行排序
    if let Some(order_by) = &query.order_by {
        sort_rows_with_alias(&mut filtered_records, table, order_by, &columns, &alias_map)?;
    }

    // 10. 应用LIMIT限制
    let limit = query.limit.unwrap_or(filtered_records.len());
    let rows_to_process = &filtered_records[..core::cmp::min(filtered_records.len(), limit)];

    // 8. 检查是否包含聚合函数
    let has_aggregate = columns.iter().any(|expr| {
        match expr {
            Expression::FunctionCall { name, .. } => {
                let name = name.to_uppercase();
                // 基础聚合函数
                let basic_agg = name == "COUNT"
                    || name == "SUM"
                    || name == "AVG"
                    || name == "MIN"
                    || name == "MAX";
                // 新增统计学函数
                let stat_agg = name == "STDDEV"
                    || name == "VAR"
                    || name == "STDDEV_SAMP"
                    || name == "VAR_SAMP";
                // 新增滑动窗口函数（目前按聚合函数处理）
                let window_agg = name == "MOVING_AVERAGE" || name == "MOVING_SUM";

                basic_agg || stat_agg || window_agg
            }
            _ => false,
        }
    });

    // 额外检查：如果是COUNT查询，确保作为聚合查询处理
    let is_count_query = columns.iter().any(|expr| match expr {
        Expression::FunctionCall { name, .. } => {
            let name = name.to_uppercase();
            name == "COUNT"
        }
        _ => false,
    });

    // 检查是否有GROUP BY子句
    let has_group_by = query.group_by.is_some();

    if has_aggregate || is_count_query || has_group_by {
        // 处理聚合查询或GROUP BY查询
        // 提取记录值用于聚合计算
        let mut records_for_aggregation = Vec::with_capacity(rows_to_process.len());
        for (record_values, _) in rows_to_process {
            records_for_aggregation.push(record_values.clone());
        }

        // 构建字段名到索引的映射
        let field_index_map: std::collections::HashMap<String, usize> = table
            .def
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.clone(), i))
            .collect();

        if has_group_by {
            // 处理GROUP BY查询
            process_group_by_query(
                table,
                &columns,
                &records_for_aggregation,
                query.group_by.as_ref().expect("group_by must be set"),
                &mut result_set,
            )?;
        } else {
            // 处理普通聚合查询
            process_aggregate_query(
                &columns,
                &records_for_aggregation,
                &mut result_set,
                &field_index_map,
            )?;
        }
    } else {
        // 处理普通查询
        if query.distinct {
            // 使用集合存储唯一行，仅在std环境下支持
            #[cfg(feature = "std")]
            {
                let mut unique_rows = std::collections::HashSet::new();

                // 使用预计算的表达式值进行去重和结果生成
                for (_, expr_values) in rows_to_process {
                    // 只有当行不在集合中时才添加
                    if unique_rows.insert(expr_values.clone()) {
                        result_set.add_row(expr_values.clone());
                    }
                }
            }

            // 在no_std环境下，不支持distinct查询，直接返回所有行
            #[cfg(not(feature = "std"))]
            {
                for (_, expr_values) in rows_to_process {
                    result_set.add_row(expr_values.clone());
                }
            }
        } else {
            // 普通查询，不需要去重
            for (_, expr_values) in rows_to_process {
                result_set.add_row(expr_values.clone());
            }
        }
    }

    Ok(result_set)
}

/// 查询执行统计信息
#[derive(Default)]
struct QueryStats {
    /// 是否使用了索引
    used_index: bool,
    /// 扫描的记录数
    scanned_records: usize,
    /// 匹配的记录数
    matched_records: usize,
    /// 执行时间（微秒）
    execution_time: u64,
}

/// 处理聚合查询
fn process_aggregate_query(
    columns: &[Expression],
    rows_to_process: &[Vec<TypedValue>],
    result_set: &mut ResultSet,
    field_index_map: &std::collections::HashMap<String, usize>,
) -> Result<(), QueryExecutionError> {
    // 为每个聚合函数准备初始值
    let mut aggregate_values = Vec::with_capacity(columns.len());

    // 为方差和标准差准备额外的状态：(sum, sum_of_squares, count)
    let mut var_stddev_states: Vec<(f64, f64, usize)> = Vec::with_capacity(columns.len());

    for expr in columns {
        match expr {
            Expression::FunctionCall { name, .. } => {
                let name = name.to_uppercase();
                match name.as_str() {
                    "COUNT" => {
                        // 初始化COUNT为0
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    "SUM" => {
                        // 初始化SUM为0，类型为UInt64
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    "AVG" => {
                        // 初始化AVG的sum为0，类型为Float64
                        aggregate_values.push(TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        });
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    "MIN" | "MAX" => {
                        // 初始化MIN/MAX为None（使用0作为占位符，后续会更新）
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    // 新增统计学函数初始化
                    "STDDEV" | "VAR" | "STDDEV_SAMP" | "VAR_SAMP" => {
                        // 初始化方差和标准差的中间状态
                        aggregate_values.push(TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        });
                        // 初始化(sum, sum_of_squares, count)
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    // 新增滑动窗口函数初始化
                    "MOVING_AVERAGE" | "MOVING_SUM" => {
                        // 初始化滑动窗口函数为0
                        aggregate_values.push(TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: 0.0 },
                        });
                        // 初始化(sum, sum_of_squares, count)
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    // 时间窗口函数初始化
                    "TIME_BUCKET" => {
                        // TIME_BUCKET函数需要为每个分组保存结果，这里先使用默认值
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                        var_stddev_states.push((0.0, 0.0, 0));
                    }
                    _ => {
                        return Err(QueryExecutionError::UnsupportedFunction(name.to_string()));
                    }
                }
            }
            _ => {
                // 非聚合列在聚合查询中应该有别名或分组
                // 这里暂时不处理，直接返回错误
                return Err(QueryExecutionError::InternalError);
            }
        }
    }

    // 遍历所有行，更新聚合值
    for record_values in rows_to_process {
        for (i, expr) in columns.iter().enumerate() {
            if let Expression::FunctionCall { name, args, .. } = expr {
                let name = name.to_uppercase();

                // 计算当前行的函数值
                let current_value =
                    evaluate_expression_for_aggregate(args, record_values, field_index_map)?;

                // 更新聚合值
                match name.as_str() {
                    "COUNT" => {
                        unsafe {
                            // COUNT函数简单累加
                            aggregate_values[i].value.u64 += 1;
                        }
                    }
                    "SUM" => {
                        unsafe {
                            // SUM函数累加值
                            match current_value.value_type {
                                DataType::UInt8 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u8 as f64
                                }
                                DataType::UInt16 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u16 as f64
                                }
                                DataType::UInt32 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u32 as f64
                                }
                                DataType::UInt64 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u64 as f64
                                }
                                DataType::Int8 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i8 as f64
                                }
                                DataType::Int16 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i16 as f64
                                }
                                DataType::Int32 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i32 as f64
                                }
                                DataType::Int64 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i64 as f64
                                }
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }

                            // 更新计数
                            let (_, _, count) = &mut var_stddev_states[i];
                            *count += 1;
                        }
                    }
                    "MIN" => {
                        // MIN函数取最小值
                        unsafe {
                            if aggregate_values[i].value.u64 == 0 {
                                // 第一次迭代，直接赋值
                                aggregate_values[i] = current_value;
                            } else {
                                // 比较并取最小值
                                let is_less = unsafe {
                                    match (aggregate_values[i].value_type, current_value.value_type)
                                    {
                                        (DataType::UInt8, DataType::UInt8) => {
                                            current_value.value.u8 < aggregate_values[i].value.u8
                                        }
                                        (DataType::UInt16, DataType::UInt16) => {
                                            current_value.value.u16 < aggregate_values[i].value.u16
                                        }
                                        (DataType::UInt32, DataType::UInt32) => {
                                            current_value.value.u32 < aggregate_values[i].value.u32
                                        }
                                        (DataType::UInt64, DataType::UInt64) => {
                                            current_value.value.u64 < aggregate_values[i].value.u64
                                        }
                                        (DataType::Int8, DataType::Int8) => {
                                            current_value.value.i8 < aggregate_values[i].value.i8
                                        }
                                        (DataType::Int16, DataType::Int16) => {
                                            current_value.value.i16 < aggregate_values[i].value.i16
                                        }
                                        (DataType::Int32, DataType::Int32) => {
                                            current_value.value.i32 < aggregate_values[i].value.i32
                                        }
                                        (DataType::Int64, DataType::Int64) => {
                                            current_value.value.i64 < aggregate_values[i].value.i64
                                        }
                                        (DataType::Float32, DataType::Float32) => {
                                            current_value.value.float32
                                                < aggregate_values[i].value.float32
                                        }
                                        (DataType::Float64, DataType::Float64) => {
                                            current_value.value.float64
                                                < aggregate_values[i].value.float64
                                        }
                                        _ => return Err(QueryExecutionError::TypeMismatch),
                                    }
                                };
                                if is_less {
                                    aggregate_values[i] = current_value;
                                }
                            }
                        }
                    }
                    "MAX" => {
                        // MAX函数取最大值
                        unsafe {
                            if aggregate_values[i].value.u64 == 0 {
                                // 第一次迭代，直接赋值
                                aggregate_values[i] = current_value;
                            } else {
                                // 比较并取最大值
                                let is_greater = unsafe {
                                    match (aggregate_values[i].value_type, current_value.value_type)
                                    {
                                        (DataType::UInt8, DataType::UInt8) => {
                                            current_value.value.u8 > aggregate_values[i].value.u8
                                        }
                                        (DataType::UInt16, DataType::UInt16) => {
                                            current_value.value.u16 > aggregate_values[i].value.u16
                                        }
                                        (DataType::UInt32, DataType::UInt32) => {
                                            current_value.value.u32 > aggregate_values[i].value.u32
                                        }
                                        (DataType::UInt64, DataType::UInt64) => {
                                            current_value.value.u64 > aggregate_values[i].value.u64
                                        }
                                        (DataType::Int8, DataType::Int8) => {
                                            current_value.value.i8 > aggregate_values[i].value.i8
                                        }
                                        (DataType::Int16, DataType::Int16) => {
                                            current_value.value.i16 > aggregate_values[i].value.i16
                                        }
                                        (DataType::Int32, DataType::Int32) => {
                                            current_value.value.i32 > aggregate_values[i].value.i32
                                        }
                                        (DataType::Int64, DataType::Int64) => {
                                            current_value.value.i64 > aggregate_values[i].value.i64
                                        }
                                        (DataType::Float32, DataType::Float32) => {
                                            current_value.value.float32
                                                > aggregate_values[i].value.float32
                                        }
                                        (DataType::Float64, DataType::Float64) => {
                                            current_value.value.float64
                                                > aggregate_values[i].value.float64
                                        }
                                        _ => return Err(QueryExecutionError::TypeMismatch),
                                    }
                                };
                                if is_greater {
                                    aggregate_values[i] = current_value;
                                }
                            }
                        }
                    }
                    "AVG" => {
                        // AVG函数需要同时计算总和和计数，最后再求平均
                        // 这里简化处理，只返回总和
                        unsafe {
                            match current_value.value_type {
                                DataType::UInt8 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u8 as f64
                                }
                                DataType::UInt16 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u16 as f64
                                }
                                DataType::UInt32 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u32 as f64
                                }
                                DataType::UInt64 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.u64 as f64
                                }
                                DataType::Int8 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i8 as f64
                                }
                                DataType::Int16 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i16 as f64
                                }
                                DataType::Int32 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i32 as f64
                                }
                                DataType::Int64 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.i64 as f64
                                }
                                DataType::Float32 => {
                                    aggregate_values[i].value.float64 +=
                                        current_value.value.float32 as f64
                                }
                                DataType::Float64 => {
                                    aggregate_values[i].value.float64 += current_value.value.float64
                                }
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }
                        }
                    }
                    // 新增统计学函数处理
                    "STDDEV" | "VAR" | "STDDEV_SAMP" | "VAR_SAMP" => {
                        // 将当前值转换为浮点数
                        let current_float = unsafe {
                            match current_value.value_type {
                                DataType::UInt8 => current_value.value.u8 as f64,
                                DataType::UInt16 => current_value.value.u16 as f64,
                                DataType::UInt32 => current_value.value.u32 as f64,
                                DataType::UInt64 => current_value.value.u64 as f64,
                                DataType::Int8 => current_value.value.i8 as f64,
                                DataType::Int16 => current_value.value.i16 as f64,
                                DataType::Int32 => current_value.value.i32 as f64,
                                DataType::Int64 => current_value.value.i64 as f64,
                                DataType::Float32 => current_value.value.float32 as f64,
                                DataType::Float64 => current_value.value.float64,
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }
                        };

                        // 更新方差和标准差的状态：(sum, sum_of_squares, count)
                        let (sum, sum_of_squares, count) = &mut var_stddev_states[i];
                        *sum += current_float;
                        *sum_of_squares += current_float * current_float;
                        *count += 1;
                    }
                    // 新增滑动窗口函数处理
                    "MOVING_AVERAGE" | "MOVING_SUM" => {
                        // 将当前值转换为浮点数
                        let current_float = unsafe {
                            match current_value.value_type {
                                DataType::UInt8 => current_value.value.u8 as f64,
                                DataType::UInt16 => current_value.value.u16 as f64,
                                DataType::UInt32 => current_value.value.u32 as f64,
                                DataType::UInt64 => current_value.value.u64 as f64,
                                DataType::Int8 => current_value.value.i8 as f64,
                                DataType::Int16 => current_value.value.i16 as f64,
                                DataType::Int32 => current_value.value.i32 as f64,
                                DataType::Int64 => current_value.value.i64 as f64,
                                DataType::Float32 => current_value.value.float32 as f64,
                                DataType::Float64 => current_value.value.float64,
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }
                        };

                        // 更新滑动窗口函数的状态：(sum, sum_of_squares, count)
                        let (sum, _, count) = &mut var_stddev_states[i];
                        *sum += current_float;
                        *count += 1;
                    }
                    // 时间窗口函数处理
                    "TIME_BUCKET" => {
                        // TIME_BUCKET函数的处理逻辑在execute_time_bucket中已经实现
                        // 这里我们只需要保存当前值
                        aggregate_values[i] = current_value;
                    }
                    _ => return Err(QueryExecutionError::UnsupportedFunction(name.to_string())),
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
    }

    // 计算最终的聚合结果，特别是方差和标准差
    for (i, expr) in columns.iter().enumerate() {
        if let Expression::FunctionCall { name, .. } = expr {
            let name = name.to_uppercase();
            match name.as_str() {
                // 计算方差和标准差
                "STDDEV" => {
                    let (sum, sum_of_squares, count) = var_stddev_states[i];
                    if count > 0 {
                        // 总体方差：sum_of_squares/count - (sum/count)^2
                        let mean = sum / count as f64;
                        let variance = sum_of_squares / count as f64 - mean * mean;
                        // 总体标准差：sqrt(variance)
                        #[cfg(feature = "std")]
                        let stddev = variance.sqrt();
                        #[cfg(not(feature = "std"))]
                        let stddev = 0.0;
                        aggregate_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: stddev },
                        };
                    }
                }
                "VAR" => {
                    let (sum, sum_of_squares, count) = var_stddev_states[i];
                    if count > 0 {
                        // 总体方差：sum_of_squares/count - (sum/count)^2
                        let mean = sum / count as f64;
                        let variance = sum_of_squares / count as f64 - mean * mean;
                        aggregate_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: variance },
                        };
                    }
                }
                "AVG" => {
                    let (_, _, count) = var_stddev_states[i];
                    if count > 0 {
                        // 计算平均值：总和 / 计数
                        unsafe {
                            let sum = aggregate_values[i].value.float64;
                            let avg = sum / count as f64;
                            aggregate_values[i] = TypedValue {
                                value_type: DataType::Float64,
                                value: Value { float64: avg },
                            };
                        }
                    }
                }
                "STDDEV_SAMP" => {
                    let (sum, sum_of_squares, count) = var_stddev_states[i];
                    if count > 1 {
                        // 样本方差：(sum_of_squares - sum^2/count) / (count - 1)
                        let _mean = sum / count as f64;
                        let variance =
                            (sum_of_squares - sum * sum / count as f64) / (count - 1) as f64;
                        // 样本标准差：sqrt(variance)
                        #[cfg(feature = "std")]
                        let stddev = variance.sqrt();
                        #[cfg(not(feature = "std"))]
                        let stddev = 0.0;
                        aggregate_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: stddev },
                        };
                    }
                }
                "VAR_SAMP" => {
                    let (sum, sum_of_squares, count) = var_stddev_states[i];
                    if count > 1 {
                        // 样本方差：(sum_of_squares - sum^2/count) / (count - 1)
                        let _mean = sum / count as f64;
                        let variance =
                            (sum_of_squares - sum * sum / count as f64) / (count - 1) as f64;
                        aggregate_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: variance },
                        };
                    }
                }
                // 滑动窗口函数处理
                "MOVING_AVERAGE" => {
                    let (sum, _, count) = var_stddev_states[i];
                    if count > 0 {
                        // 简单实现：返回平均值，不实现完整的滑动窗口逻辑
                        let avg = sum / count as f64;
                        aggregate_values[i] = TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: avg },
                        };
                    }
                }
                "MOVING_SUM" => {
                    let (sum, _, _) = var_stddev_states[i];
                    // 简单实现：返回总和，不实现完整的滑动窗口逻辑
                    aggregate_values[i] = TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: sum },
                    };
                }
                _ => {} // 其他函数不需要额外计算
            }
        }
    }

    // 添加聚合结果到结果集
    result_set.add_row(aggregate_values);

    Ok(())
}

