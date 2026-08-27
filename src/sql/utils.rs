//! SQL Query Utility Functions
//!
//! This module contains shared utility functions used across SQL operations.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "log")]
use crate::log::{debug, error};
use crate::sql::QueryExecutionError;
use crate::sql::query_parser::{BinaryOperator, Expression};
use crate::sql::{OrderByClause, SqlQuery, Value as SqlValue};
use crate::types::TypedValue;
use crate::types::{DataType, DEFAULT_JSON_SIZE, DEFAULT_TEXT_SIZE};
use crate::MemoryTable;

/// 解析数据类型字符串，提取基本类型、精度/维度和距离类型
/// 例如："TIMESTAMP(6)" -> ("TIMESTAMP", 6, None)
///       "VECTOR(768)" -> ("VECTOR", 768, None)
///       "VECTOR(64) WITH DISTANCE=IP" -> ("VECTOR", 64, Some(InnerProduct))
pub fn parse_data_type_with_precision(
    type_str: &str,
) -> Result<(String, u16, Option<crate::types::DistanceType>), QueryExecutionError> {
    #[cfg(feature = "log")]
    crate::log::debug!("parse_data_type_with_precision called with: {}", type_str);
    let type_str = type_str.to_uppercase();

    // 查找左括号位置
    if let Some(open_paren) = type_str.find('(') {
        // 查找对应的右括号，忽略WITH子句
        let close_paren = type_str
            .find(')')
            .ok_or(QueryExecutionError::TypeMismatch)?;

        // 提取括号内的维度值，确保只包含数字
        let param_str = type_str[open_paren + 1..close_paren].trim();
        let param = param_str
            .parse::<u16>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;

        // 提取纯基本类型，用于匹配DataType
        let base_type = type_str[..open_paren].trim();

        // 对向量类型添加维度限制
        if base_type == "VECTOR" {
            // 向量维度限制为1-4096
            if !(1..=4096).contains(&param) {
                return Err(QueryExecutionError::TypeMismatch);
            }
        }

        // 解析距离类型（仅适用于向量类型）
        let mut distance_type = None;
        if base_type == "VECTOR" {
            // 检查是否包含WITH DISTANCE修饰符
            if type_str.contains("WITH DISTANCE=L2") {
                distance_type = Some(crate::types::DistanceType::L2);
            } else if type_str.contains("WITH DISTANCE=INNER_PRODUCT")
                || type_str.contains("WITH DISTANCE=IP")
            {
                distance_type = Some(crate::types::DistanceType::InnerProduct);
            } else if type_str.contains("WITH DISTANCE=COSINE") {
                distance_type = Some(crate::types::DistanceType::Cosine);
            }
        }

        // 验证基本类型是否有效
        match base_type {
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64"
            | "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32"
            | "UINT64" | "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" | "VARCHAR"
            | "CHAR" | "TEXT" | "BOOL" | "BOOLEAN" | "TIMESTAMP" | "TIMESTAMPTZ" | "JSON"
            | "VECTOR" => Ok((base_type.to_string(), param, distance_type)),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    } else {
        // 没有参数，使用默认值
        let base_type = type_str.trim();

        // 验证基本类型是否有效，并为不同类型设置合适的默认大小
        match base_type {
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64"
            | "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32"
            | "UINT64" | "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" | "BOOL"
            | "BOOLEAN" => Ok((base_type.to_string(), 8, None)),
            "VARCHAR" | "CHAR" => Ok((base_type.to_string(), 64, None)),
            "TEXT" => Ok((base_type.to_string(), DEFAULT_TEXT_SIZE as u16, None)),
            "JSON" => Ok((base_type.to_string(), DEFAULT_JSON_SIZE as u16, None)),
            "TIMESTAMP" | "TIMESTAMPTZ" => Ok((base_type.to_string(), 6, None)),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 检查内存限制
pub fn check_memory_limit(
    estimated_usage: usize,
    max_memory_mb: Option<u32>,
) -> Result<(), QueryExecutionError> {
    if let Some(max_mb) = max_memory_mb {
        let max_bytes = (max_mb as usize) * 1024 * 1024;
        if estimated_usage > max_bytes {
            return Err(QueryExecutionError::ResourceLimitExceeded(format!(
                "Query exceeds memory limit: {}MB estimated, {}MB allowed",
                estimated_usage / (1024 * 1024),
                max_mb
            )));
        }
    }

    Ok(())
}

/// 处理AT TIME ZONE操作符
/// 将timestamp转换为指定时区的timestamp
pub fn process_at_time_zone(
    timestamp: &crate::types::db_timestamp,
    timezone_spec: &str,
) -> Result<crate::types::db_timestamp, QueryExecutionError> {
    // 解析时区规范
    let tz_offset = if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            ((hours * 3600) + (minutes * 60)) as i16
        } else {
            return Err(QueryExecutionError::TypeMismatch);
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec).ok_or(QueryExecutionError::TypeMismatch)?
    };

    // 转换时间戳到指定时区
    Ok(crate::types::convert_timezone(timestamp, tz_offset))
}

/// 处理TIMEZONE()函数
/// 获取指定时区的偏移量
pub fn process_timezone_function(timezone_spec: &str) -> Result<i16, QueryExecutionError> {
    // 解析时区规范
    if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1]
                .parse::<i32>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            Ok(((hours * 3600) + (minutes * 60)) as i16)
        } else {
            Err(QueryExecutionError::TypeMismatch)
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec).ok_or(QueryExecutionError::TypeMismatch)
    }
}

/// 处理TO_CHAR()函数
/// 将时间戳转换为指定格式的字符串
pub fn process_to_char(
    timestamp: &crate::types::db_timestamp,
    format: &str,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_char(timestamp, format))
}

/// 处理TO_ISO8601()函数
/// 将时间戳转换为ISO 8601格式的字符串
pub fn process_to_iso8601(
    timestamp: &crate::types::db_timestamp,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_iso8601(timestamp))
}

/// 处理TO_EPOCH()函数
/// 将时间戳转换为epoch秒数
pub fn process_to_epoch(
    timestamp: &crate::types::db_timestamp,
) -> Result<f64, QueryExecutionError> {
    Ok(crate::types::time_format::to_epoch(timestamp))
}
/// 将表达式转换为ORDER BY子句用的字符串表示（用于向量表达式匹配）
pub fn expr_to_order_by_string(expr: &Expression) -> alloc::string::String {
    match expr {
        Expression::BinaryOp {
            left, op, right, ..
        } => {
            let left_name = match left.as_ref() {
                Expression::Field { name, .. } => name.clone(),
                _ => return alloc::string::String::new(),
            };
            let op_str = match op {
                BinaryOperator::VectorL2 => "<->",
                BinaryOperator::VectorIP => "<#>",
                BinaryOperator::VectorCosine => "<=>",
                _ => return alloc::string::String::new(),
            };
            let right_str = match right.as_ref() {
                Expression::Constant { value, .. } => match value {
                    SqlValue::Json(json_str) => json_str.clone(),
                    SqlValue::String(s) => s.clone(),
                    _ => return alloc::string::String::new(),
                },
                _ => return alloc::string::String::new(),
            };
            alloc::format!("{} {} {}", left_name, op_str, right_str)
        }
        _ => alloc::string::String::new(),
    }
}

/// 对行进行排序
pub fn sort_rows(
    rows: &mut Vec<Vec<TypedValue>>,
    table: &MemoryTable,
    order_by: &OrderByClause,
) -> Result<(), QueryExecutionError> {
    // 检查ORDER BY子句是否使用位置索引
    if let Ok(col_index) = order_by.field.parse::<usize>() {
        // ORDER BY使用位置索引
        let sort_col_index = col_index - 1; // SQL位置索引从1开始

        // 确保索引有效
        if rows.is_empty() {
            return Ok(());
        }
        if sort_col_index >= rows[0].len() {
            return Err(QueryExecutionError::FieldNotFound);
        }

        // 对行进行排序
        rows.sort_by(|a, b| {
            let val_a = &a[sort_col_index];
            let val_b = &b[sort_col_index];

            // 根据值的实际类型比较
            unsafe {
                let comparison = match (val_a.value_type, val_b.value_type) {
                    // 无符号整数类型
                    (DataType::UInt8, DataType::UInt8) => val_a.value.u8.cmp(&val_b.value.u8),
                    (DataType::UInt16, DataType::UInt16) => val_a.value.u16.cmp(&val_b.value.u16),
                    (DataType::UInt32, DataType::UInt32) => val_a.value.u32.cmp(&val_b.value.u32),
                    (DataType::UInt64, DataType::UInt64) => val_a.value.u64.cmp(&val_b.value.u64),

                    // 有符号整数类型
                    (DataType::Int8, DataType::Int8) => val_a.value.i8.cmp(&val_b.value.i8),
                    (DataType::Int16, DataType::Int16) => val_a.value.i16.cmp(&val_b.value.i16),
                    (DataType::Int32, DataType::Int32) => val_a.value.i32.cmp(&val_b.value.i32),
                    (DataType::Int64, DataType::Int64) => val_a.value.i64.cmp(&val_b.value.i64),

                    // 浮点数类型
                    (DataType::Float32, DataType::Float32) => val_a
                        .value
                        .float32
                        .partial_cmp(&val_b.value.float32)
                        .unwrap_or(core::cmp::Ordering::Equal),
                    (DataType::Float64, DataType::Float64) => val_a
                        .value
                        .float64
                        .partial_cmp(&val_b.value.float64)
                        .unwrap_or(core::cmp::Ordering::Equal),

                    // 时间戳类型
                    (DataType::Timestamp, DataType::Timestamp) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }
                    (DataType::TimestampTZ, DataType::TimestampTZ) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }

                    // 其他类型，默认按升序排列
                    _ => core::cmp::Ordering::Equal,
                };

                // 根据排序方向调整结果
                match order_by.direction {
                    crate::sql::OrderDirection::Ascending => comparison,
                    crate::sql::OrderDirection::Descending => comparison.reverse(),
                }
            }
        });

        return Ok(());
    }

    // 处理带表别名的字段名，如 "t.id"
    let actual_field_name = if order_by.field.contains('.') {
        // 提取点号后面的部分作为实际字段名
        order_by
            .field
            .split('.')
            .next_back()
            .expect("field name must contain '.'")
    } else {
        // 没有表别名，直接使用字段名
        &order_by.field
    };

    // 查找排序字段在表中的索引
    #[cfg(feature = "log")]
    debug!(
        "DEBUG get_field_value (unsafe): looking for field '{}' in table '{}'",
        actual_field_name, table.def.name
    );
    let field_index = table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
        .ok_or_else(|| {
            #[cfg(feature = "log")]
            error!("DEBUG get_field_value (unsafe): field '{}' not found in table '{}'. Available fields: {:?}", actual_field_name, table.def.name, table.def.fields.iter().map(|f| &f.name).collect::<Vec<_>>());
            QueryExecutionError::FieldNotFound
        })?;

    let field_type = table.def.fields[field_index].data_type;

    // 对行进行排序
    rows.sort_by(|a, b| {
        // 查找排序字段在返回列中的索引
        // 遍历表的所有字段，找到在返回列中对应的索引
        let mut sort_col_index = 0;
        for (i, field) in table.def.fields.iter().enumerate() {
            if field.name == *actual_field_name {
                sort_col_index = i;
                break;
            }
        }

        // 确保索引不超出范围
        if sort_col_index >= a.len() || sort_col_index >= b.len() {
            return core::cmp::Ordering::Equal;
        }

        let val_a = &a[sort_col_index];
        let val_b = &b[sort_col_index];

        // 根据字段类型比较值
        let comparison = match field_type {
            // 无符号整数类型
            DataType::UInt8 => {
                let a_val = unsafe { val_a.value.u8 };
                let b_val = unsafe { val_b.value.u8 };
                a_val.cmp(&b_val)
            }
            DataType::UInt16 => {
                let a_val = unsafe { val_a.value.u16 };
                let b_val = unsafe { val_b.value.u16 };
                a_val.cmp(&b_val)
            }
            DataType::UInt32 => {
                let a_val = unsafe { val_a.value.u32 };
                let b_val = unsafe { val_b.value.u32 };
                a_val.cmp(&b_val)
            }
            DataType::UInt64 => {
                let a_val = unsafe { val_a.value.u64 };
                let b_val = unsafe { val_b.value.u64 };
                a_val.cmp(&b_val)
            }

            // 有符号整数类型
            DataType::Int8 => {
                let a_val = unsafe { val_a.value.i8 };
                let b_val = unsafe { val_b.value.i8 };
                a_val.cmp(&b_val)
            }
            DataType::Int16 => {
                let a_val = unsafe { val_a.value.i16 };
                let b_val = unsafe { val_b.value.i16 };
                a_val.cmp(&b_val)
            }
            DataType::Int32 => {
                let a_val = unsafe { val_a.value.i32 };
                let b_val = unsafe { val_b.value.i32 };
                a_val.cmp(&b_val)
            }
            DataType::Int64 => {
                let a_val = unsafe { val_a.value.i64 };
                let b_val = unsafe { val_b.value.i64 };
                a_val.cmp(&b_val)
            }

            // 浮点数类型
            DataType::Float32 => {
                let a_val = unsafe { val_a.value.float32 };
                let b_val = unsafe { val_b.value.float32 };
                a_val
                    .partial_cmp(&b_val)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }
            DataType::Float64 => {
                let a_val = unsafe { val_a.value.float64 };
                let b_val = unsafe { val_b.value.float64 };
                a_val
                    .partial_cmp(&b_val)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }

            // 布尔类型
            DataType::Bool => {
                let a_val = unsafe { val_a.value.bool };
                let b_val = unsafe { val_b.value.bool };
                a_val.cmp(&b_val)
            }

            // 时间戳类型
            DataType::Timestamp => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            }
            DataType::TimestampTZ => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            }

            // 字符串类型
            DataType::VarChar | DataType::Char | DataType::Text => {
                let a_str = unsafe { &val_a.value.string };
                let b_str = unsafe { &val_b.value.string };

                let a_str = String::from_utf8_lossy(a_str)
                    .trim_end_matches(char::from(0))
                    .to_string();
                let b_str = String::from_utf8_lossy(b_str)
                    .trim_end_matches(char::from(0))
                    .to_string();

                a_str.cmp(&b_str)
            }
            // 时间间隔类型
            DataType::Interval => {
                let a_val = unsafe { val_a.value.interval.value };
                let b_val = unsafe { val_b.value.interval.value };
                a_val.cmp(&b_val)
            }
            // 向量类型 - 目前不支持排序
            DataType::Vector => core::cmp::Ordering::Equal,
            // JSON类型 - 目前不支持排序
            DataType::Json => core::cmp::Ordering::Equal,
        };

        // 根据排序方向调整结果
        match order_by.direction {
            crate::sql::OrderDirection::Ascending => comparison,
            crate::sql::OrderDirection::Descending => comparison.reverse(),
        }
    });

    Ok(())
}

/// 对行进行排序（支持别名）
pub fn sort_rows_with_alias(
    rows: &mut Vec<(Vec<TypedValue>, Vec<TypedValue>)>,
    table: &MemoryTable,
    order_by: &OrderByClause,
    columns: &[Expression],
    alias_map: &alloc::collections::BTreeMap<String, &Expression>,
) -> Result<(), QueryExecutionError> {
    // 检查ORDER BY子句是否使用位置索引
    if let Ok(col_index) = order_by.field.parse::<usize>() {
        // ORDER BY使用位置索引
        let sort_col_index = col_index - 1; // SQL位置索引从1开始

        // 确保索引有效
        if rows.is_empty() {
            return Ok(());
        }
        if sort_col_index >= rows[0].1.len() {
            // rows[i].1是表达式值
            return Err(QueryExecutionError::FieldNotFound);
        }

        // 对行进行排序
        rows.sort_by(|a, b| {
            let val_a = &a.1[sort_col_index];
            let val_b = &b.1[sort_col_index];

            // 根据值的实际类型比较
            unsafe {
                let comparison = match (val_a.value_type, val_b.value_type) {
                    // 无符号整数类型
                    (DataType::UInt8, DataType::UInt8) => val_a.value.u8.cmp(&val_b.value.u8),
                    (DataType::UInt16, DataType::UInt16) => val_a.value.u16.cmp(&val_b.value.u16),
                    (DataType::UInt32, DataType::UInt32) => val_a.value.u32.cmp(&val_b.value.u32),
                    (DataType::UInt64, DataType::UInt64) => val_a.value.u64.cmp(&val_b.value.u64),

                    // 有符号整数类型
                    (DataType::Int8, DataType::Int8) => val_a.value.i8.cmp(&val_b.value.i8),
                    (DataType::Int16, DataType::Int16) => val_a.value.i16.cmp(&val_b.value.i16),
                    (DataType::Int32, DataType::Int32) => val_a.value.i32.cmp(&val_b.value.i32),
                    (DataType::Int64, DataType::Int64) => val_a.value.i64.cmp(&val_b.value.i64),

                    // 浮点数类型
                    (DataType::Float32, DataType::Float32) => val_a
                        .value
                        .float32
                        .partial_cmp(&val_b.value.float32)
                        .unwrap_or(core::cmp::Ordering::Equal),
                    (DataType::Float64, DataType::Float64) => val_a
                        .value
                        .float64
                        .partial_cmp(&val_b.value.float64)
                        .unwrap_or(core::cmp::Ordering::Equal),

                    // 时间戳类型
                    (DataType::Timestamp, DataType::Timestamp) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }
                    (DataType::TimestampTZ, DataType::TimestampTZ) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }

                    // 其他类型，默认按升序排列
                    _ => core::cmp::Ordering::Equal,
                };

                // 根据排序方向调整结果
                match order_by.direction {
                    crate::sql::OrderDirection::Ascending => comparison,
                    crate::sql::OrderDirection::Descending => comparison.reverse(),
                }
            }
        });

        return Ok(());
    }

    // 处理带表别名的字段名，如 "t.id"
    let actual_field_name = if order_by.field.contains('.') {
        // 提取点号后面的部分作为实际字段名
        order_by
            .field
            .split('.')
            .next_back()
            .expect("field name must contain '.'")
    } else {
        // 没有表别名，直接使用字段名
        &order_by.field
    };

    // 检查是否使用别名
    if let Some(alias_expr) = alias_map.get(actual_field_name) {
        // 找到别名对应的表达式索引
        let mut expr_index = 0;
        for (i, expr) in columns.iter().enumerate() {
            if expr == *alias_expr {
                expr_index = i;
                break;
            }
        }

        // 对行进行排序
        rows.sort_by(|a, b| {
            let val_a = &a.1[expr_index];
            let val_b = &b.1[expr_index];

            // 根据值的实际类型比较
            unsafe {
                let comparison = match (val_a.value_type, val_b.value_type) {
                    // 无符号整数类型
                    (DataType::UInt8, DataType::UInt8) => val_a.value.u8.cmp(&val_b.value.u8),
                    (DataType::UInt16, DataType::UInt16) => val_a.value.u16.cmp(&val_b.value.u16),
                    (DataType::UInt32, DataType::UInt32) => val_a.value.u32.cmp(&val_b.value.u32),
                    (DataType::UInt64, DataType::UInt64) => val_a.value.u64.cmp(&val_b.value.u64),

                    // 有符号整数类型
                    (DataType::Int8, DataType::Int8) => val_a.value.i8.cmp(&val_b.value.i8),
                    (DataType::Int16, DataType::Int16) => val_a.value.i16.cmp(&val_b.value.i16),
                    (DataType::Int32, DataType::Int32) => val_a.value.i32.cmp(&val_b.value.i32),
                    (DataType::Int64, DataType::Int64) => val_a.value.i64.cmp(&val_b.value.i64),

                    // 浮点数类型
                    (DataType::Float32, DataType::Float32) => val_a
                        .value
                        .float32
                        .partial_cmp(&val_b.value.float32)
                        .unwrap_or(core::cmp::Ordering::Equal),
                    (DataType::Float64, DataType::Float64) => val_a
                        .value
                        .float64
                        .partial_cmp(&val_b.value.float64)
                        .unwrap_or(core::cmp::Ordering::Equal),

                    // 布尔类型
                    (DataType::Bool, DataType::Bool) => val_a.value.bool.cmp(&val_b.value.bool),

                    // 时间戳类型
                    (DataType::Timestamp, DataType::Timestamp) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }
                    (DataType::TimestampTZ, DataType::TimestampTZ) => {
                        val_a.value.time.value.cmp(&val_b.value.time.value)
                    }

                    // 其他类型，默认按升序排列
                    _ => core::cmp::Ordering::Equal,
                };

                // 根据排序方向调整结果
                match order_by.direction {
                    crate::sql::OrderDirection::Ascending => comparison,
                    crate::sql::OrderDirection::Descending => comparison.reverse(),
                }
            }
        });

        return Ok(());
    }

    // 检查ORDER BY字段是否包含向量距离操作符
    if order_by.field.contains("<->")
        || order_by.field.contains("<#>")
        || order_by.field.contains("<=>")
    {
        // 这是一个向量距离表达式，查找匹配的列表达式
        for (i, expr) in columns.iter().enumerate() {
            let expr_str = expr_to_order_by_string(expr);
            if expr_str == order_by.field {
                // 找到匹配的列，按该列的表达式值排序
                rows.sort_by(|a, b| {
                    let val_a = &a.1[i];
                    let val_b = &b.1[i];

                    unsafe {
                        let comparison = match (val_a.value_type, val_b.value_type) {
                            // 浮点数类型
                            (DataType::Float32, DataType::Float32) => val_a
                                .value
                                .float32
                                .partial_cmp(&val_b.value.float32)
                                .unwrap_or(core::cmp::Ordering::Equal),
                            (DataType::Float64, DataType::Float64) => val_a
                                .value
                                .float64
                                .partial_cmp(&val_b.value.float64)
                                .unwrap_or(core::cmp::Ordering::Equal),
                            // 其他类型，默认按升序排列
                            _ => core::cmp::Ordering::Equal,
                        };

                        match order_by.direction {
                            crate::sql::OrderDirection::Ascending => comparison,
                            crate::sql::OrderDirection::Descending => comparison.reverse(),
                        }
                    }
                });

                return Ok(());
            }
        }
        // 没有找到匹配的列表达式，回退到普通字段查找
    }

    // 没有使用别名，使用原始的排序逻辑
    // 查找排序字段在表中的索引
    #[cfg(feature = "log")]
    debug!(
        "DEBUG get_field_value: looking for field '{}' in table '{}'",
        actual_field_name, table.def.name
    );
    let field_index = table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
        .ok_or_else(|| {
            #[cfg(feature = "log")]
            error!(
                "DEBUG get_field_value: field '{}' not found in table '{}'. Available fields: {:?}",
                actual_field_name,
                table.def.name,
                table.def.fields.iter().map(|f| &f.name).collect::<Vec<_>>()
            );
            QueryExecutionError::FieldNotFound
        })?;

    let field_type = table.def.fields[field_index].data_type;

    // 对行进行排序
    rows.sort_by(|a, b| {
        // 查找排序字段在返回列中的索引
        let val_a = &a.0[field_index]; // a.0是原始记录值
        let val_b = &b.0[field_index];

        // 根据字段类型比较值
        let comparison = match field_type {
            // 无符号整数类型
            DataType::UInt8 => {
                let a_val = unsafe { val_a.value.u8 };
                let b_val = unsafe { val_b.value.u8 };
                a_val.cmp(&b_val)
            }
            DataType::UInt16 => {
                let a_val = unsafe { val_a.value.u16 };
                let b_val = unsafe { val_b.value.u16 };
                a_val.cmp(&b_val)
            }
            DataType::UInt32 => {
                let a_val = unsafe { val_a.value.u32 };
                let b_val = unsafe { val_b.value.u32 };
                a_val.cmp(&b_val)
            }
            DataType::UInt64 => {
                let a_val = unsafe { val_a.value.u64 };
                let b_val = unsafe { val_b.value.u64 };
                a_val.cmp(&b_val)
            }

            // 有符号整数类型
            DataType::Int8 => {
                let a_val = unsafe { val_a.value.i8 };
                let b_val = unsafe { val_b.value.i8 };
                a_val.cmp(&b_val)
            }
            DataType::Int16 => {
                let a_val = unsafe { val_a.value.i16 };
                let b_val = unsafe { val_b.value.i16 };
                a_val.cmp(&b_val)
            }
            DataType::Int32 => {
                let a_val = unsafe { val_a.value.i32 };
                let b_val = unsafe { val_b.value.i32 };
                a_val.cmp(&b_val)
            }
            DataType::Int64 => {
                let a_val = unsafe { val_a.value.i64 };
                let b_val = unsafe { val_b.value.i64 };
                a_val.cmp(&b_val)
            }

            // 浮点数类型
            DataType::Float32 => {
                let a_val = unsafe { val_a.value.float32 };
                let b_val = unsafe { val_b.value.float32 };
                a_val
                    .partial_cmp(&b_val)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }
            DataType::Float64 => {
                let a_val = unsafe { val_a.value.float64 };
                let b_val = unsafe { val_b.value.float64 };
                a_val
                    .partial_cmp(&b_val)
                    .unwrap_or(core::cmp::Ordering::Equal)
            }

            // 布尔类型
            DataType::Bool => {
                let a_val = unsafe { val_a.value.bool };
                let b_val = unsafe { val_b.value.bool };
                a_val.cmp(&b_val)
            }

            // 时间戳类型
            DataType::Timestamp => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            }
            DataType::TimestampTZ => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            }

            // 其他类型，默认按升序排列
            _ => core::cmp::Ordering::Equal,
        };

        // 根据排序方向调整结果
        match order_by.direction {
            crate::sql::OrderDirection::Ascending => comparison,
            crate::sql::OrderDirection::Descending => comparison.reverse(),
        }
    });

    Ok(())
}

/// 辅助函数：从条件中获取字段值
/// 注意：此函数当前未被使用，但保留以供将来使用
pub fn get_field_value_from_condition<'a>(
    field: &'a str,
    query: &'a SqlQuery,
    main_table: &'a MemoryTable,
    main_record_values: &'a [TypedValue],
    join_table: &'a MemoryTable,
    join_record_values: &'a [TypedValue],
) -> (&'a MemoryTable, &'a TypedValue) {
    // 处理带表名/别名的字段
    let (table_name_part, field_name_part) = if field.contains('.') {
        let parts: Vec<&str> = field.split('.').collect();
        (Some(parts[0]), parts[1])
    } else {
        (None, field)
    };

    // 根据表名确定从哪个记录中获取字段值
    if let Some(table_name) = table_name_part {
        if table_name == query.table_name || Some(table_name) == query.table_alias.as_deref() {
            // 从主表获取
            let field_index = main_table
                .def
                .fields
                .iter()
                .position(|f| f.name == field_name_part)
                .unwrap_or(0); // 默认为第一个字段
            (main_table, &main_record_values[field_index])
        } else {
            // 从连接表获取
            let field_index = join_table
                .def
                .fields
                .iter()
                .position(|f| f.name == field_name_part)
                .unwrap_or(0); // 默认为第一个字段
            (join_table, &join_record_values[field_index])
        }
    } else {
        // 没有指定表名，尝试从主表查找，找不到再从连接表查找
        if let Some(field_index) = main_table
            .def
            .fields
            .iter()
            .position(|f| f.name == field_name_part)
        {
            (main_table, &main_record_values[field_index])
        } else if let Some(field_index) = join_table
            .def
            .fields
            .iter()
            .position(|f| f.name == field_name_part)
        {
            (join_table, &join_record_values[field_index])
        } else {
            // 字段未找到，返回主表第一个字段的默认值
            // 注意：这里理论上不会执行到，因为字段在之前的解析中已经验证过
            let default_index = if !main_table.def.fields.is_empty() {
                0
            } else {
                0
            };
            (main_table, &main_record_values[default_index])
        }
    }
}

/// 带超时执行的查询包装器
pub fn execute_with_timeout<F, T>(
    timeout_ms: Option<u64>,
    operation: F,
    operation_name: &str,
) -> Result<T, QueryExecutionError>
where
    F: FnOnce() -> Result<T, QueryExecutionError> + Send + 'static,
    T: Send + 'static,
{
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    // 如果没有设置超时，直接执行操作
    if timeout_ms.is_none() {
        return operation();
    }

    let timeout = Duration::from_millis(timeout_ms.expect("timeout_ms must be set"));

    // 创建通道用于接收操作结果
    let (tx, rx) = mpsc::channel();

    // 在新线程中执行操作
    thread::spawn(move || {
        let result = operation();
        // 发送结果到通道，忽略发送错误（如果接收方已关闭）
        let _ = tx.send(result);
    });

    // 等待结果或超时
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => Err(QueryExecutionError::ResourceLimitExceeded(format!(
            "Query timeout after {}ms for {}",
            timeout_ms.expect("timeout_ms must be set"),
            operation_name
        ))),
    }
}

/// 估算普通查询记录的内存使用量（字节）
pub fn estimate_memory_usage_for_records(records: &[Vec<TypedValue>]) -> usize {
    // 简化估算：每条记录的基本大小
    const BASE_VALUE_SIZE: usize = std::mem::size_of::<TypedValue>();
    let total_values = records.iter().map(|record| record.len()).sum::<usize>();
    total_values * BASE_VALUE_SIZE
}

/// 估算时序记录的内存使用量（字节）
pub fn estimate_memory_usage(records: &[crate::time_series::TimeSeriesRecord]) -> usize {
    // 简化估算：每条记录的基本大小 + 标签存储
    const BASE_RECORD_SIZE: usize = std::mem::size_of::<crate::time_series::TimeSeriesRecord>();
    records.len() * BASE_RECORD_SIZE
}

