//! SQL DML (Data Manipulation Language) 操作
//!
//! 该模块包含INSERT/UPDATE/DELETE查询的执行逻辑，含时序表插入。

use crate::try_lock;

#[cfg(feature = "log")]
use crate::log::debug;
use crate::sql::operations::comparison::evaluate_condition;
use crate::sql::operations::expression::evaluate_expression_with_depth;
use crate::sql::query_parser::Expression;
use crate::sql::Value as SqlValue;
use crate::sql::{QueryExecutionError, ResultSet, SqlQuery};
use crate::types::{DataType, TypedValue};
use crate::{MemoryTable, RemDb, RemDbError, Value};
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
/// 执行时序表INSERT查询
fn execute_insert_timeseries_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    use crate::time_series::TimeSeriesRecord;

    // 1. 查找时序表
    let ts_table_id = db
        .time_series_tables
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.base.name == query.table_name
            } else {
                false
            }
        })
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 获取时序表的可变引用
    let ts_table = db.time_series_tables[ts_table_id]
        .as_mut()
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 3. 解析字段索引
    let time_field_idx = ts_table.def.time_field;
    let value_field_idx = ts_table.def.value_field;
    let tag_field_indices = &ts_table.def.tag_fields;

    // 4. 开始事务（如果需要）
    let mut tx_buffer = crate::transaction::Transaction::default();
    let mut log_buffer = alloc::vec![crate::transaction::VariableSizeLogItem::default(); 10];
    let has_active_tx = crate::transaction::has_active_tx();

    if !has_active_tx {
        unsafe {
            crate::transaction::begin(
                crate::transaction::TransactionType::ReadWrite,
                crate::transaction::IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 5. 执行插入操作
    let mut affected_rows = 0;

    for values in &query.values {
        let mut timestamp: u64 = 0;
        let mut value: f64 = 0.0;
        let mut tags = [0u64; 8];
        let mut tag_count = 0;

        // 解析每个字段的值
        for (i, field) in ts_table.def.base.fields.iter().enumerate() {
            let field_value = if !query.insert_columns.is_empty() {
                if let Some(col_index) = query
                    .insert_columns
                    .iter()
                    .position(|col| *col == field.name)
                {
                    if col_index < values.len() {
                        Some(&values[col_index])
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                if i < values.len() {
                    Some(&values[i])
                } else {
                    None
                }
            };

            if let Some(val) = field_value {
                if i == time_field_idx {
                    // 时间字段
                    timestamp = match val {
                        crate::sql::query_parser::Value::Integer(v) => *v as u64,
                        crate::sql::query_parser::Value::Float(v) => *v as u64,
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                } else if i == value_field_idx {
                    // 值字段
                    value = match val {
                        crate::sql::query_parser::Value::Integer(v) => *v as f64,
                        crate::sql::query_parser::Value::Float(v) => *v,
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                } else if tag_field_indices.contains(&i) {
                    // 标签字段
                    if tag_count < 8 {
                        match val {
                            crate::sql::query_parser::Value::String(s) => {
                                let mut hash: u64 = 0;
                                for c in s.chars() {
                                    hash = hash.wrapping_mul(31).wrapping_add(c as u64);
                                }
                                tags[tag_count as usize] = hash;
                            }
                            crate::sql::query_parser::Value::Integer(v) => {
                                tags[tag_count as usize] = *v as u64;
                            }
                            _ => {}
                        }
                        tag_count += 1;
                    }
                }
            }
        }

        // 创建时序记录
        let record = TimeSeriesRecord {
            timestamp,
            value,
            tag_count,
            tags,
        };

        // 获取或创建分区
        let mut partitions_guard = try_lock!(ts_table.partitions);
        let partition = partitions_guard.get_or_create_partition(record.timestamp);

        // 写入记录到分区
        let mut partition_guard = try_lock!(partition);
        partition_guard.records.push(record);
        partition_guard.stats.record_count = partition_guard.records.len();

        // 更新索引
        ts_table
            .index
            .insert(record.timestamp, affected_rows as usize);

        affected_rows += 1;
    }

    // 6. 创建结果集
    let columns = alloc::vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: DataType::Int64,
        value: Value {
            i64: affected_rows as i64
        },
    }]);

    Ok(result_set)
}

/// 设置字段值
fn set_field_value_with_depth(
    table: &MemoryTable,
    record_data: &mut Vec<u8>,
    offset: usize,
    data_type: DataType,
    field_size: usize,
    expr: &Expression,
    depth: usize,
) -> Result<(), QueryExecutionError> {
    // Check recursion depth to prevent stack overflow
    const MAX_RECURSION_DEPTH: usize = 100;
    if depth > MAX_RECURSION_DEPTH {
        return Err(QueryExecutionError::InternalError);
    }

    #[cfg(feature = "log")]
    debug!(
        "set_field_value_with_depth: data_type={:?}, offset={}, field_size={}, expr={:?}",
        data_type, offset, field_size, expr
    );
    unsafe {
        // 1. 从record_data中提取所有字段的当前值
        let record_values = table
            .def
            .fields
            .iter()
            .map(|field| {
                let field_ptr = record_data.as_ptr().add(field.offset);
                let value = match field.data_type {
                    DataType::UInt8 => crate::types::Value {
                        u8: unsafe { *field_ptr },
                    },
                    DataType::UInt16 => crate::types::Value {
                        u16: unsafe { core::ptr::read_unaligned(field_ptr as *const u16) },
                    },
                    DataType::UInt32 => crate::types::Value {
                        u32: unsafe { core::ptr::read_unaligned(field_ptr as *const u32) },
                    },
                    DataType::UInt64 => crate::types::Value {
                        u64: unsafe { core::ptr::read_unaligned(field_ptr as *const u64) },
                    },
                    DataType::Int8 => crate::types::Value {
                        i8: unsafe { core::ptr::read_unaligned(field_ptr as *const i8) },
                    },
                    DataType::Int16 => crate::types::Value {
                        i16: unsafe { core::ptr::read_unaligned(field_ptr as *const i16) },
                    },
                    DataType::Int32 => crate::types::Value {
                        i32: unsafe { core::ptr::read_unaligned(field_ptr as *const i32) },
                    },
                    DataType::Int64 => crate::types::Value {
                        i64: unsafe { core::ptr::read_unaligned(field_ptr as *const i64) },
                    },
                    DataType::Float32 => crate::types::Value {
                        float32: unsafe { core::ptr::read_unaligned(field_ptr as *const f32) },
                    },
                    DataType::Float64 => crate::types::Value {
                        float64: unsafe { core::ptr::read_unaligned(field_ptr as *const f64) },
                    },
                    DataType::Bool => crate::types::Value {
                        bool: unsafe { *field_ptr != 0 },
                    },
                    DataType::VarChar | DataType::Char => {
                        let mut str_value = [0u8; crate::types::MAX_STRING_LEN];
                        // Only copy up to MAX_STRING_LEN bytes to avoid buffer overflow
                        let copy_len = core::cmp::min(field.size, crate::types::MAX_STRING_LEN);
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                field_ptr,
                                str_value.as_mut_ptr(),
                                copy_len,
                            );
                        }
                        crate::types::Value { string: str_value }
                    }
                    DataType::Text => {
                        // Read TextStorage from the record
                        let text_storage = unsafe {
                            core::ptr::read_unaligned(field_ptr as *const crate::types::TextStorage)
                        };
                        crate::types::Value { text_storage }
                    }
                    DataType::Json => crate::types::Value {
                        json_storage: unsafe {
                            core::ptr::read_unaligned(field_ptr as *const crate::types::JsonStorage)
                        },
                    },
                    _ => crate::types::Value { i64: 0 },
                };
                crate::types::TypedValue {
                    value_type: field.data_type,
                    value,
                }
            })
            .collect::<Vec<_>>();

        // 2. 评估表达式
        let evaluated_value =
            evaluate_expression_with_depth(table, &record_values, expr, depth + 1)?;
        #[cfg(feature = "log")]
        debug!(
            "evaluated_value: value_type={:?}, field_type={:?}",
            evaluated_value.value_type, data_type
        );

        // 3. 根据字段类型设置值
        match data_type {
            // 无符号整数类型
            DataType::UInt8 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as u8,
                    DataType::Float64 => evaluated_value.value.float64 as u8,
                    DataType::Bool => evaluated_value.value.bool as u8,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // u8不需要对齐，直接复制
                record_data[offset] = value;
            }
            DataType::UInt16 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as u16,
                    DataType::Float64 => evaluated_value.value.float64 as u16,
                    DataType::Bool => evaluated_value.value.bool as u16,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u16, value);
            }
            DataType::UInt32 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as u32,
                    DataType::Float64 => evaluated_value.value.float64 as u32,
                    DataType::Bool => evaluated_value.value.bool as u32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u32, value);
            }
            DataType::UInt64 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as u64,
                    DataType::Float64 => evaluated_value.value.float64 as u64,
                    DataType::Bool => evaluated_value.value.bool as u64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u64, value);
            }

            // 有符号整数类型
            DataType::Int8 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as i8,
                    DataType::Float64 => evaluated_value.value.float64 as i8,
                    DataType::Bool => evaluated_value.value.bool as i8,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // i8不需要对齐，直接复制
                record_data[offset] = value as u8;
            }
            DataType::Int16 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as i16,
                    DataType::Float64 => evaluated_value.value.float64 as i16,
                    DataType::Bool => evaluated_value.value.bool as i16,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i16, value);
            }
            DataType::Int32 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as i32,
                    DataType::Float64 => evaluated_value.value.float64 as i32,
                    DataType::Bool => evaluated_value.value.bool as i32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i32, value);
            }
            DataType::Int64 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64,
                    DataType::Float64 => evaluated_value.value.float64 as i64,
                    DataType::Bool => evaluated_value.value.bool as i64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i64, value);
            }

            // 浮点数类型
            DataType::Float32 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as f32,
                    DataType::Float64 => evaluated_value.value.float64 as f32,
                    DataType::Bool => (evaluated_value.value.bool as u8) as f32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut f32, value);
            }
            DataType::Float64 => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 as f64,
                    DataType::Float64 => evaluated_value.value.float64,
                    DataType::Bool => (evaluated_value.value.bool as u8) as f64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut f64, value);
            }

            // 布尔类型
            DataType::Bool => {
                let value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64 != 0,
                    DataType::Float64 => evaluated_value.value.float64 != 0.0,
                    DataType::Bool => evaluated_value.value.bool,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // bool不需要对齐，直接复制
                record_data[offset] = value as u8;
            }

            // 时间戳类型
            DataType::Timestamp => {
                // 支持从数值类型或字符串转换为时间戳
                let timestamp_value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64,
                    DataType::UInt64 => evaluated_value.value.u64 as i64,
                    DataType::Int32 => evaluated_value.value.i32 as i64,
                    DataType::UInt32 => evaluated_value.value.u32 as i64,
                    DataType::Int16 => evaluated_value.value.i16 as i64,
                    DataType::UInt16 => evaluated_value.value.u16 as i64,
                    DataType::Int8 => evaluated_value.value.i8 as i64,
                    DataType::UInt8 => evaluated_value.value.u8 as i64,
                    DataType::Float64 => evaluated_value.value.float64 as i64,
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        let s = core::str::from_utf8(&evaluated_value.value.string)
                            .unwrap_or_default()
                            .trim_end_matches(char::from(0));
                        crate::sql::query_parser::parse_time_string(s)
                            .map_err(|_| QueryExecutionError::TypeMismatch)?
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 创建时间戳值
                let timestamp = crate::types::db_timestamp::new(timestamp_value, 0, 0, 0);

                // 写入时间戳到记录数据
                let ptr = record_data.as_mut_ptr().add(offset) as *mut crate::types::db_timestamp;
                core::ptr::write_unaligned(ptr, timestamp);
            }
            // 时间戳TZ类型
            DataType::TimestampTZ => {
                // 支持从数值类型或字符串转换为时间戳TZ
                let timestamp_value = match evaluated_value.value_type {
                    DataType::Int64 => evaluated_value.value.i64,
                    DataType::UInt64 => evaluated_value.value.u64 as i64,
                    DataType::Int32 => evaluated_value.value.i32 as i64,
                    DataType::UInt32 => evaluated_value.value.u32 as i64,
                    DataType::Int16 => evaluated_value.value.i16 as i64,
                    DataType::UInt16 => evaluated_value.value.u16 as i64,
                    DataType::Int8 => evaluated_value.value.i8 as i64,
                    DataType::UInt8 => evaluated_value.value.u8 as i64,
                    DataType::Float64 => evaluated_value.value.float64 as i64,
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        let s = core::str::from_utf8(&evaluated_value.value.string)
                            .unwrap_or_default()
                            .trim_end_matches(char::from(0));
                        crate::sql::query_parser::parse_time_string(s)
                            .map_err(|_| QueryExecutionError::TypeMismatch)?
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 创建时间戳值（默认UTC时区）
                let timestamp = crate::types::db_timestamp::new(timestamp_value, 0, 0, 0);

                // 写入时间戳到记录数据
                let ptr = record_data.as_mut_ptr().add(offset) as *mut crate::types::db_timestamp;
                core::ptr::write_unaligned(ptr, timestamp);
            }

            // 字符串类型（VarChar/Char）
            DataType::VarChar | DataType::Char => {
                let str_value = match evaluated_value.value_type {
                    DataType::VarChar | DataType::Char => {
                        let s =
                            core::str::from_utf8(&evaluated_value.value.string).unwrap_or_default();
                        s
                    }
                    DataType::Text => {
                        // 从TextStorage提取字符串
                        let text_content = if evaluated_value.value.text_storage.is_inline() {
                            if let Some(data) = evaluated_value.value.text_storage.as_inline() {
                                let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
                                core::str::from_utf8(&data[..end])
                                    .unwrap_or_default()
                                    .to_string()
                            } else {
                                String::new()
                            }
                        } else if evaluated_value.value.text_storage.is_external() {
                            if let Some(ext) = evaluated_value.value.text_storage.as_external() {
                                if !ext.data_ptr.is_null() {
                                    let bytes = unsafe {
                                        core::slice::from_raw_parts(
                                            ext.data_ptr,
                                            ext.length as usize,
                                        )
                                    };
                                    core::str::from_utf8(bytes).unwrap_or_default().to_string()
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        // Return as owned string
                        return match alloc::string::String::from_utf8(text_content.into_bytes()) {
                            Ok(s) => {
                                let ptr = record_data.as_mut_ptr().add(offset);
                                let max_len = field_size;
                                let bytes = s.as_bytes();
                                let copy_len = core::cmp::min(bytes.len(), max_len);
                                let mut i = 0;
                                while i < copy_len {
                                    *ptr.add(i) = bytes[i];
                                    i += 1;
                                }
                                while i < max_len {
                                    *ptr.add(i) = 0;
                                    i += 1;
                                }
                                Ok(())
                            }
                            Err(_) => Err(QueryExecutionError::TypeMismatch),
                        };
                    }
                    DataType::Json => {
                        // 从JSON存储中提取字符串（用于向VarChar列写入JSON字符串）
                        let json_str = match &evaluated_value.value.json_storage {
                            crate::types::JsonStorage::Inline(json_bytes) => {
                                core::str::from_utf8(json_bytes.as_slice())
                                    .unwrap_or_default()
                                    .trim_end_matches(char::from(0))
                                    .to_string()
                            }
                            crate::types::JsonStorage::Null => {
                                // For Null storage, the JSON string was too large for the inline buffer.
                                // Try to extract the original string from the expression.
                                if let Expression::Constant {
                                    value: crate::sql::Value::Json(s),
                                    ..
                                } = expr
                                {
                                    s.clone()
                                } else {
                                    return Err(QueryExecutionError::TypeMismatch);
                                }
                            }
                            _ => return Err(QueryExecutionError::TypeMismatch),
                        };
                        // 直接写入记录缓冲区
                        let ptr = record_data.as_mut_ptr().add(offset);
                        let max_len = field_size;
                        let bytes = json_str.as_bytes();
                        let copy_len = core::cmp::min(bytes.len(), max_len);
                        let mut i = 0;
                        while i < copy_len {
                            *ptr.add(i) = bytes[i];
                            i += 1;
                        }
                        // 填充剩余空间为0
                        while i < max_len {
                            *ptr.add(i) = 0;
                            i += 1;
                        }
                        return Ok(());
                    }
                    DataType::Int64 => &evaluated_value.value.i64.to_string(),
                    DataType::Float64 => &evaluated_value.value.float64.to_string(),
                    DataType::Bool => &evaluated_value.value.bool.to_string(),
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let ptr = record_data.as_mut_ptr().add(offset);
                // 复制字符串到缓冲区，确保不超过字段大小
                let max_len = field_size;
                for (i, c) in str_value.as_bytes().iter().enumerate() {
                    if i < max_len {
                        *ptr.add(i) = *c;
                    } else {
                        break;
                    }
                }
                // 填充剩余空间为0
                for i in str_value.len()..max_len {
                    *ptr.add(i) = 0;
                }
            }

            // TEXT类型（支持动态分配）
            DataType::Text => {
                // 获取字符串内容
                let (text_bytes, text_len) = match evaluated_value.value_type {
                    DataType::VarChar | DataType::Char => {
                        // Try to get the original string from the expression constant
                        // (evaluate_expression truncates to 64 bytes for string constants).
                        // This is critical for TEXT columns that can hold >64 bytes.
                        let s = if let Expression::Constant {
                            value: SqlValue::String(s),
                            ..
                        } = expr
                        {
                            s.clone()
                        } else {
                            core::str::from_utf8(&evaluated_value.value.string)
                                .unwrap_or_default()
                                .trim_end_matches(char::from(0))
                                .to_string()
                        };
                        let bytes = s.as_bytes().to_vec();
                        let len = bytes.len();
                        (bytes, len)
                    }
                    DataType::Text => {
                        // TextStorage already set, just write it directly
                        let ptr =
                            record_data.as_mut_ptr().add(offset) as *mut crate::types::TextStorage;
                        // Free old external allocation if present
                        crate::table::free_text_storage(ptr);
                        // Write the new TextStorage value
                        core::ptr::write_unaligned(ptr, evaluated_value.value.text_storage);
                        return Ok(());
                    }
                    DataType::Int64 => {
                        let s = evaluated_value.value.i64.to_string();
                        let len = s.len();
                        (s.into_bytes(), len)
                    }
                    DataType::Float64 => {
                        let s = evaluated_value.value.float64.to_string();
                        let len = s.len();
                        (s.into_bytes(), len)
                    }
                    DataType::Bool => {
                        let s = evaluated_value.value.bool.to_string();
                        let len = s.len();
                        (s.into_bytes(), len)
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 创建TextStorage并写入
                let text_storage = if text_len <= 256 {
                    // 内联存储
                    let mut inline_data = [0u8; 256];
                    inline_data[..text_len].copy_from_slice(&text_bytes[..text_len]);
                    crate::types::TextStorage::new_inline(inline_data)
                } else {
                    // 外部存储：通过全局分配器分配内存
                    let capacity = text_len;
                    match crate::memory::allocator::alloc(capacity) {
                        Ok(ptr) => {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    text_bytes.as_ptr(),
                                    ptr.as_ptr(),
                                    text_len,
                                );
                            }
                            crate::types::TextStorage::new_external(
                                ptr.as_ptr(),
                                text_len as u32,
                                capacity as u32,
                            )
                        }
                        Err(_) => {
                            // 分配失败，回退到Null
                            crate::types::TextStorage::new_null()
                        }
                    }
                };

                let ptr = record_data.as_mut_ptr().add(offset) as *mut crate::types::TextStorage;
                // Free old external allocation if present
                crate::table::free_text_storage(ptr);
                // Write the new TextStorage value
                core::ptr::write_unaligned(ptr, text_storage);
            }
            // 时间间隔类型
            DataType::Interval => {
                return Err(QueryExecutionError::TypeMismatch);
            }
            // 向量类型
            DataType::Vector => {
                // 处理字符串类型的向量字面量（来自evaluate_expression的结果）
                if matches!(
                    evaluated_value.value_type,
                    DataType::VarChar | DataType::Char | DataType::Text
                ) {
                    // 从固定大小的字符串数组中提取有效字符串（去除后面的零字节）
                    let string_slice = evaluated_value
                        .value
                        .string
                        .iter()
                        .take_while(|&&c| c != 0)
                        .copied()
                        .collect::<Vec<_>>();
                    let s = core::str::from_utf8(&string_slice).unwrap_or_default();

                    // 检查是否是向量字面量格式 [x1, x2, ..., xn]
                    if s.starts_with('[') && s.ends_with(']') {
                        let vec_str = &s[1..s.len() - 1];
                        let vec_values: Vec<&str> = vec_str.split(',').map(|v| v.trim()).collect();

                        // 计算向量维度
                        let expected_dim = field_size / 4;
                        if vec_values.len() != expected_dim {
                            return Err(QueryExecutionError::TypeMismatch);
                        }

                        // 解析向量值并写入记录
                        let vec_ptr = record_data.as_mut_ptr().add(offset) as *mut f32;
                        for (i, val_str) in vec_values.iter().enumerate() {
                            if let Ok(val) = val_str.parse::<f32>() {
                                core::ptr::write_unaligned(vec_ptr.add(i), val);
                            } else {
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        }
                        return Ok(());
                    }
                } else if matches!(evaluated_value.value_type, DataType::Json) {
                    // 处理JSON类型的向量字面量
                    let json_str = match &evaluated_value.value.json_storage {
                        crate::types::JsonStorage::Inline(json_bytes) => {
                            core::str::from_utf8(json_bytes.as_slice())
                                .unwrap_or_default()
                                .trim_end_matches(char::from(0))
                                .to_string()
                        }
                        crate::types::JsonStorage::Null => {
                            // For Null storage, the JSON string was too large for the inline buffer.
                            // Try to extract the original string from the Constant expression.
                            let extracted = match expr {
                                Expression::Constant {
                                    value: crate::sql::Value::Json(s),
                                    ..
                                } => Some(s.clone()),
                                Expression::Constant {
                                    value: crate::sql::Value::String(s),
                                    ..
                                } => Some(s.clone()),
                                _ => None,
                            };
                            match extracted {
                                Some(s) => s,
                                None => {
                                    #[cfg(feature = "log")]
                                    debug!(
                                        "Vector field: JsonStorage::Null but expression is not Expression::Constant with Json/String value, expr={:?}",
                                        expr
                                    );
                                    return Err(QueryExecutionError::TypeMismatch);
                                }
                            }
                        }
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };

                    // Check if it's a vector literal [x1, x2, ..., xn]
                    let s = json_str.trim();
                    if s.starts_with('[') && s.ends_with(']') {
                        let vec_str = &s[1..s.len() - 1];
                        let vec_values: Vec<&str> = vec_str.split(',').map(|v| v.trim()).collect();

                        // Calculate expected dimension
                        let expected_dim = field_size / 4;
                        if vec_values.len() != expected_dim {
                            return Err(QueryExecutionError::TypeMismatch);
                        }

                        // Parse vector values and write to record
                        let vec_ptr = record_data.as_mut_ptr().add(offset) as *mut f32;
                        for (i, val_str) in vec_values.iter().enumerate() {
                            if let Ok(val) = val_str.parse::<f32>() {
                                core::ptr::write_unaligned(vec_ptr.add(i), val);
                            } else {
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        }
                        return Ok(());
                    }
                }

                // 处理其他表达式类型或非向量字面量情况
                match evaluated_value.value_type {
                    DataType::Vector => {
                        // 直接复制向量数据
                        core::ptr::copy_nonoverlapping(
                            evaluated_value.value.vector,
                            record_data.as_mut_ptr().add(offset) as *mut f32,
                            field_size / 4,
                        );
                    }
                    _ => {
                        // 调试信息
                        if matches!(
                            evaluated_value.value_type,
                            DataType::VarChar | DataType::Char | DataType::Text
                        ) {
                            let s = core::str::from_utf8(&evaluated_value.value.string)
                                .unwrap_or_default();
                            #[cfg(feature = "log")]
                            debug!("Vector field got string: '{}', starts_with('['): {}, ends_with(']'): {}", s, s.starts_with('['), s.ends_with(']'));
                        } else {
                            #[cfg(feature = "log")]
                            debug!(
                                "Vector field got unexpected type: {:?}",
                                evaluated_value.value_type
                            );
                        }
                        return Err(QueryExecutionError::TypeMismatch);
                    }
                }
            }
            // JSON类型
            DataType::Json => {
                #[cfg(feature = "log")]
                debug!(
                    "JSON field - evaluated_value.value_type: {:?}",
                    evaluated_value.value_type
                );
                // 处理JSON类型
                match evaluated_value.value_type {
                    DataType::Json => {
                        // 直接复制JSON存储
                        core::ptr::write_unaligned(
                            record_data.as_mut_ptr().add(offset) as *mut crate::types::JsonStorage,
                            evaluated_value.value.json_storage,
                        );
                    }
                    DataType::Int64 => {
                        // 处理NULL值（Int64(0)表示NULL）
                        if evaluated_value.value.i64 == 0 {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(offset)
                                    as *mut crate::types::JsonStorage,
                                crate::types::JsonStorage::Null,
                            );
                        } else {
                            #[cfg(feature = "log")]
                            debug!(
                                "JSON field got unexpected Int64 value: {}",
                                evaluated_value.value.i64
                            );
                            return Err(QueryExecutionError::TypeMismatch);
                        }
                    }
                    _ => {
                        #[cfg(feature = "log")]
                        debug!(
                            "JSON field got unexpected type: {:?}",
                            evaluated_value.value_type
                        );
                        return Err(QueryExecutionError::TypeMismatch);
                    }
                }
            }
        }
    }

    Ok(())
}

/// 设置字段值（包装函数，使用深度0）
fn set_field_value(
    table: &MemoryTable,
    record_data: &mut Vec<u8>,
    offset: usize,
    data_type: DataType,
    field_size: usize,
    expr: &Expression,
) -> Result<(), QueryExecutionError> {
    set_field_value_with_depth(table, record_data, offset, data_type, field_size, expr, 0)
}

/// 执行UPDATE查询
pub fn execute_update_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要更新的表的ID
    let table_id = db
        .tables
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == query.table_name
            } else {
                false
            }
        })
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 获取可变表引用（用于遍历和更新）
    let table_mut = db
        .get_table_mut(table_id)
        .map_err(|_| QueryExecutionError::InternalError)?;
    let record_size = table_mut.record_size;

    // 3. 检查是否有活跃事务，如果没有则创建一个
    let has_active_tx = crate::transaction::has_active_tx();
    let mut tx_buffer = crate::transaction::Transaction::default();
    let mut log_buffer = alloc::vec![crate::transaction::VariableSizeLogItem::default(); 10];

    if !has_active_tx {
        // 没有活跃事务，开始一个新事务
        unsafe {
            crate::transaction::begin(
                crate::transaction::TransactionType::ReadWrite,
                crate::transaction::IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 4. 遍历表中的所有记录，收集要更新的记录ID和它们的当前数据
    let mut to_update = Vec::new();

    unsafe {
        // 遍历表中的所有记录
        let iterate_result = table_mut.iterate(|id, record_ptr| {
            // 检查记录是否符合WHERE条件
            let mut matches = true;
            if let Some(where_clause) = &query.where_clause {
                matches = evaluate_condition(table_mut, record_ptr, &where_clause.condition);
            }

            if matches {
                // 复制记录数据到临时缓冲区
                let mut record_data = alloc::vec![0; record_size];
                core::ptr::copy_nonoverlapping(record_ptr, record_data.as_mut_ptr(), record_size);
                to_update.push((id, record_data));
            }

            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }

    // 5. 执行更新操作
    let mut affected_rows = 0;
    for (id, mut record_data) in to_update {
        // 遍历所有要更新的字段值对
        for (field_name, new_value) in &query.update_pairs {
            // 查找字段索引
            let field_index = table_mut
                .def
                .fields
                .iter()
                .position(|field| field.name == *field_name)
                .ok_or(QueryExecutionError::FieldNotFound)?;

            let field = &table_mut.def.fields[field_index];

            // 设置新的字段值
            set_field_value(
                table_mut,
                &mut record_data,
                field.offset,
                field.data_type,
                field.size,
                new_value,
            )?;
        }

        // 记录日志（如果有活跃事务）
        unsafe {
            if false {
                // crate::transaction::has_active_tx()
                // 保存旧数据
                let mut old_data = alloc::vec![0; record_size];
                let old_record_ptr = table_mut.get_record_ptr_mut(id);
                core::ptr::copy_nonoverlapping(old_record_ptr, old_data.as_mut_ptr(), record_size);

                // 保存新数据
                let mut new_data = alloc::vec![0; record_size];
                core::ptr::copy_nonoverlapping(
                    record_data.as_ptr(),
                    new_data.as_mut_ptr(),
                    record_size,
                );

                // 检查当前事务是否有效，避免访问悬空指针
                if let Some(mut tx) = crate::transaction::get_current_tx() {
                    // 直接使用事务添加日志项，不检查is_active()和is_read_only()
                    let tx_id = tx.as_mut().id;
                    tx.as_mut().begin_log_item(
                        tx_id,
                        crate::transaction::LogOperation::Update,
                        table_mut.def.id,
                        id as u16,
                        record_size as u16,
                        Some(&old_data),
                        Some(&new_data),
                    );
                }
            }

            // 获取记录指针并写入更新后的数据
            let record_ptr = table_mut.get_record_ptr_mut(id);
            core::ptr::copy_nonoverlapping(record_data.as_ptr(), record_ptr, record_size);

            // 更新记录版本号
            let status_ptr = table_mut.get_status_ptr(id);
            let status = &mut *status_ptr;
            status.version += 1;
        }

        affected_rows += 1;
    }

    // 6. 创建结果集，返回受影响的行数
    let columns = alloc::vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);

    let row_data = alloc::vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value {
            u64: affected_rows as u64
        },
    }];
    result_set.add_row(row_data);

    // 如果是自动创建的事务，提交它
    if !has_active_tx {
        unsafe {
            crate::transaction::commit().map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    Ok(result_set)
}

/// 执行DELETE查询
pub fn execute_delete_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要删除的表的ID
    let table_id = db
        .tables
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == query.table_name
            } else {
                false
            }
        })
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 获取表引用（用于遍历）
    let table_ref = db.tables[table_id]
        .as_ref()
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 3. 检查是否有活跃事务，如果没有则创建一个
    let has_active_tx = crate::transaction::has_active_tx();
    let mut tx_buffer = crate::transaction::Transaction::default();
    let mut log_buffer = alloc::vec![crate::transaction::VariableSizeLogItem::default(); 10];

    if !has_active_tx {
        // 没有活跃事务，开始一个新事务
        unsafe {
            crate::transaction::begin(
                crate::transaction::TransactionType::ReadWrite,
                crate::transaction::IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 4. 遍历表中的所有记录，收集要删除的记录ID
    let mut to_delete = Vec::new();

    unsafe {
        // 遍历表中的所有记录
        let iterate_result = table_ref.iterate(|id, record_ptr| {
            // 检查记录是否符合WHERE条件
            let mut matches = true;
            if let Some(where_clause) = &query.where_clause {
                matches = evaluate_condition(table_ref, record_ptr, &where_clause.condition);
            }

            if matches {
                to_delete.push(id);
            }

            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }

    // 4. 获取可变表引用（用于删除）
    let table_mut = db
        .get_table_mut(table_id)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 5. 执行删除操作
    let mut affected_rows = 0;
    for id in to_delete {
        match unsafe { table_mut.delete(id) } {
            Ok(_) => affected_rows += 1,
            Err(_) => continue, // 跳过删除失败的记录
        }
    }

    // 6. 创建结果集，返回受影响的行数
    let columns = alloc::vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);

    let row_data = alloc::vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value {
            u64: affected_rows as u64
        },
    }];
    result_set.add_row(row_data);

    // 如果是自动创建的事务，提交它
    if !has_active_tx {
        unsafe {
            crate::transaction::commit().map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    Ok(result_set)
}

/// 执行INSERT查询
pub fn execute_insert_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 检查是否是时序表
    let is_timeseries = db.time_series_tables.iter().any(|table_opt| {
        if let Some(table) = table_opt {
            table.def.base.name == query.table_name
        } else {
            false
        }
    });

    if is_timeseries {
        return execute_insert_timeseries_query(db, query);
    }

    // 2. 查找要插入的表的ID
    #[cfg(feature = "log")]
    debug!("查找表: {}", query.table_name);
    let table_id = db
        .tables
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == query.table_name
            } else {
                false
            }
        })
        .ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 获取可变表引用
    #[cfg(feature = "log")]
    debug!(
        "table_id = {}, db.tables.len() = {}",
        table_id,
        db.tables.len()
    );
    let table = db.get_table_mut(table_id).map_err(|e| {
        #[cfg(feature = "log")]
        debug!("get_table_mut failed with error: {:?}", e);
        QueryExecutionError::InternalError
    })?;

    // 3. 验证插入的字段名
    if !query.insert_columns.is_empty() {
        // 插入指定列，验证列名是否存在
        for col_name in &query.insert_columns {
            table
                .def
                .fields
                .iter()
                .position(|field| field.name == *col_name)
                .ok_or(QueryExecutionError::FieldNotFound)?;
        }
    }

    // 4. 暂时跳过事务处理，直接执行插入操作
    let mut tx_buffer = crate::transaction::Transaction::default();
    let mut log_buffer = alloc::vec![crate::transaction::VariableSizeLogItem::default(); 10];
    let has_active_tx = crate::transaction::has_active_tx();

    if !has_active_tx {
        // 没有活跃事务，开始一个新事务
        unsafe {
            crate::transaction::begin(
                crate::transaction::TransactionType::ReadWrite,
                crate::transaction::IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 5. 执行插入操作
    let mut affected_rows = 0;
    let mut last_insert_id = 0u64;

    for values in &query.values {
        // 5. 创建记录数据缓冲区并初始化为0
        let mut record_data = alloc::vec![0; table.record_size];

        // 6. 将字段值写入缓冲区
        for (i, field) in table.def.fields.iter().enumerate() {
            #[cfg(feature = "log")]
            debug!(
                "Processing field {} (index {}), insert_columns={:?}",
                field.name, i, query.insert_columns
            );
            let field_value = if !query.insert_columns.is_empty() {
                // 插入指定列
                if let Some(col_index) = query
                    .insert_columns
                    .iter()
                    .position(|col| *col == field.name)
                {
                    #[cfg(feature = "log")]
                    debug!(
                        "Field '{}' found in insert_columns at index {}",
                        field.name, col_index
                    );
                    if col_index < values.len() {
                        #[cfg(feature = "log")]
                        debug!(
                            "Using value at index {} for field '{}'",
                            col_index, field.name
                        );
                        Some(&values[col_index])
                    } else {
                        #[cfg(feature = "log")]
                        debug!(
                            "No value available for field '{}' (col_index {} >= values.len {})",
                            field.name,
                            col_index,
                            values.len()
                        );
                        None
                    }
                } else {
                    #[cfg(feature = "log")]
                    debug!("Field '{}' not found in insert_columns", field.name);
                    None
                }
            } else {
                // 插入所有列
                if i < values.len() {
                    Some(&values[i])
                } else {
                    None
                }
            };

            // 检查是否为主键且自动递增
            let is_pk_auto_incr = field.primary_key && field.auto_increment;

            #[cfg(feature = "log")]
            debug!(
                "Field: {}, PK={}, AutoIncr={}, HasValue={}",
                field.name,
                field.primary_key,
                field.auto_increment,
                field_value.is_some()
            );

            // 如果是自动递增主键且未提供值，则生成唯一值
            if is_pk_auto_incr && field_value.is_none() {
                // 生成自动递增主键值
                // 使用表中已维护的最大主键值
                let max_pk = table.max_pk;

                // 生成新的主键值，考虑目标数据类型的最大值，防止溢出
                let new_pk = match field.data_type {
                    DataType::UInt8 => {
                        if max_pk >= u8::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::UInt16 => {
                        if max_pk >= u16::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::UInt32 => {
                        if max_pk >= u32::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::UInt64 => {
                        if max_pk >= u64::MAX {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::Int8 => {
                        if max_pk >= i8::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::Int16 => {
                        if max_pk >= i16::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::Int32 => {
                        if max_pk >= i32::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    DataType::Int64 => {
                        if max_pk >= i64::MAX as u64 {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                    _ => {
                        if max_pk == u64::MAX {
                            1
                        } else {
                            max_pk + 1
                        }
                    }
                };

                // 更新表的最大主键值
                table.max_pk = new_pk;

                // 将新的主键值写入记录
                unsafe {
                    match field.data_type {
                        DataType::UInt8 => {
                            record_data[field.offset] = new_pk as u8;
                        }
                        DataType::UInt16 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u16,
                                new_pk as u16,
                            );
                        }
                        DataType::UInt32 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u32,
                                new_pk as u32,
                            );
                        }
                        DataType::UInt64 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u64,
                                new_pk,
                            );
                        }
                        DataType::Int8 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i8,
                                new_pk as i8,
                            );
                        }
                        DataType::Int16 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i16,
                                new_pk as i16,
                            );
                        }
                        DataType::Int32 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i32,
                                new_pk as i32,
                            );
                        }
                        DataType::Int64 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i64,
                                new_pk as i64,
                            );
                        }
                        _ => {}
                    }
                }
            } else if let Some(sql_value) = field_value {
                // 验证字符串长度
                if field.data_type == DataType::VarChar || field.data_type == DataType::Char {
                    if let crate::sql::Value::String(s) = sql_value {
                        // 验证字符串长度（VarChar最大65536）
                        if let Some(max_length) = field.string_length {
                            if s.len() > max_length {
                                // 自动创建的事务需要回滚，避免泄漏
                                if !has_active_tx {
                                    unsafe {
                                        crate::transaction::rollback()
                                            .map_err(|_| QueryExecutionError::InternalError)?;
                                    }
                                }
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        }
                    }
                } else if field.data_type == DataType::Text {
                    // TEXT类型没有长度限制
                }

                // 转换并设置字段值
                // 为插入操作创建一个Expression::Constant
                let expr = Expression::Constant {
                    value: sql_value.clone(),
                    alias: None,
                };

                match set_field_value(
                    table,
                    &mut record_data,
                    field.offset,
                    field.data_type,
                    field.size,
                    &expr,
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        #[cfg(feature = "log")]
                        debug!(
                            "set_field_value failed for field '{}' with error: {:?}",
                            field.name, e
                        );
                        #[cfg(feature = "log")]
                        debug!(
                            "field.type={:?}, field.offset={}, field.size={}",
                            field.data_type, field.offset, field.size
                        );
                        // 自动创建的事务需要回滚，避免泄漏
                        if !has_active_tx {
                            unsafe {
                                crate::transaction::rollback()
                                    .map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        }
                        return Err(e);
                    }
                }
            } else if let Some(default_value) = &field.default_value {
                // 使用字段默认值
                // 直接写入默认值，因为default_value是types::Value类型
                unsafe {
                    match field.data_type {
                        DataType::UInt8 => {
                            record_data[field.offset] = default_value.u8;
                        }
                        DataType::UInt16 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u16,
                                default_value.u16,
                            );
                        }
                        DataType::UInt32 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u32,
                                default_value.u32,
                            );
                        }
                        DataType::UInt64 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut u64,
                                default_value.u64,
                            );
                        }
                        DataType::Int8 => {
                            record_data[field.offset] = default_value.i8 as u8;
                        }
                        DataType::Int16 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i16,
                                default_value.i16,
                            );
                        }
                        DataType::Int32 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i32,
                                default_value.i32,
                            );
                        }
                        DataType::Int64 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut i64,
                                default_value.i64,
                            );
                        }
                        DataType::Float32 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut f32,
                                default_value.float32,
                            );
                        }
                        DataType::Float64 => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset) as *mut f64,
                                default_value.float64,
                            );
                        }
                        DataType::Bool => {
                            record_data[field.offset] = default_value.bool as u8;
                        }
                        DataType::Timestamp => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset)
                                    as *mut crate::types::db_timestamp,
                                default_value.time,
                            );
                        }
                        DataType::TimestampTZ => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset)
                                    as *mut crate::types::db_timestamp,
                                default_value.time,
                            );
                        }
                        DataType::VarChar | DataType::Char => {
                            // Only copy up to MAX_STRING_LEN bytes to avoid buffer overflow
                            let copy_len = core::cmp::min(field.size, crate::types::MAX_STRING_LEN);
                            core::ptr::copy_nonoverlapping(
                                default_value.string.as_ptr(),
                                record_data.as_mut_ptr().add(field.offset),
                                copy_len,
                            );
                        }
                        DataType::Text => {
                            // Write TextStorage as default value (usually Null)
                            let ptr = record_data.as_mut_ptr().add(field.offset)
                                as *mut crate::types::TextStorage;
                            core::ptr::write_unaligned(ptr, default_value.text_storage);
                        }
                        DataType::Interval => {
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset)
                                    as *mut crate::types::db_interval,
                                default_value.interval,
                            );
                        }
                        DataType::Vector => {
                            // 写入向量数据（考虑压缩）
                            let vector_metadata = field
                                .vector_metadata
                                .as_ref()
                                .expect("vector_metadata must be set for vector fields");
                            let dimension = vector_metadata.dimension as usize;

                            // 压缩向量数据后写入
                            crate::compression::compress_vector(
                                default_value.vector,
                                dimension,
                                record_data.as_mut_ptr().add(field.offset),
                            );
                        }
                        DataType::Json => {
                            // 写入JSON数据
                            core::ptr::write_unaligned(
                                record_data.as_mut_ptr().add(field.offset)
                                    as *mut crate::types::JsonStorage,
                                default_value.json_storage,
                            );
                        }
                    }
                }
            }
        }

        // 7. 调用表的插入方法
        match table.insert(record_data.as_ptr()) {
            Ok(slot_id) => {
                affected_rows += 1;
                last_insert_id = slot_id as u64;
            }
            Err(e) => {
                match e {
                    RemDbError::DuplicateKey => {
                        if query.ignore_duplicates {
                            // 忽略重复键，继续处理下一条记录
                            continue;
                        } else {
                            // 自动创建的事务需要回滚，避免泄漏
                            if !has_active_tx {
                                unsafe {
                                    crate::transaction::rollback()
                                        .map_err(|_| QueryExecutionError::InternalError)?;
                                }
                            }
                            return Err(QueryExecutionError::ConstraintsConflicts);
                        }
                    }
                    RemDbError::InvalidRecordSize | RemDbError::TypeMismatch => {
                        // 如果是自动创建的事务，需要回滚
                        if !has_active_tx {
                            unsafe {
                                crate::transaction::rollback()
                                    .map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        }
                        return Err(QueryExecutionError::ConstraintsConflicts);
                    }
                    RemDbError::OutOfMemory => {
                        // 如果是自动创建的事务，需要回滚
                        if !has_active_tx {
                            unsafe {
                                crate::transaction::rollback()
                                    .map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        }
                        return Err(QueryExecutionError::OutOfMemory);
                    }
                    _ => {
                        // 如果是自动创建的事务，需要回滚
                        if !has_active_tx {
                            unsafe {
                                crate::transaction::rollback()
                                    .map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        }
                        return Err(QueryExecutionError::InternalError);
                    }
                }
            }
        }
    }

    // 8. 创建结果集，返回受影响的行数
    let columns = alloc::vec!["affected_rows".to_string(), "last_insert_id".to_string()];
    let mut result_set = ResultSet::new(columns);

    let row_data = alloc::vec![
        TypedValue {
            value_type: DataType::UInt64,
            value: crate::Value {
                u64: affected_rows as u64
            },
        },
        TypedValue {
            value_type: DataType::UInt64,
            value: crate::Value {
                u64: last_insert_id
            },
        },
    ];
    result_set.add_row(row_data);

    // 提交事务
    if !has_active_tx {
        unsafe {
            crate::transaction::commit().map_err(|_| QueryExecutionError::InternalError)?;
        }
    }

    // 如果更新的是系统配置表，刷新配置缓存
    if query.table_name == crate::system_tables::SYSTEM_CONFIG_TABLE {
        unsafe {
            crate::system_tables::refresh_config_cache().unwrap_or(());
        }
    }

    Ok(result_set)
}

