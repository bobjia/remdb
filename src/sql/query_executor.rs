//! SQL查询执行器
//! 
//! 该模块负责执行SQL查询并返回结果集。

use alloc::string::String;
use alloc::vec::Vec;

use crate::{TableDef,RemDb, MemoryTable, Value, RemDbError, types::{DataType, TypedValue}, IndexType, MAX_STRING_LEN, DdlExecutor, TimeSeriesTable};
use crate::sql::{SqlQuery, ResultSet, Condition, ComparisonCondition, ComparisonOperator, OrderByClause};
use crate::sql::query_parser::{BetweenCondition, Expression, BinaryOperator};

/// 解析数据类型字符串，提取基本类型和精度
/// 例如："TIMESTAMP(6)" -> ("TIMESTAMP", 6)
fn parse_data_type_with_precision(type_str: &str) -> Result<(String, u8), QueryExecutionError> {
    let type_str = type_str.to_uppercase();
    
    // 查找左括号位置
    if let Some(open_paren) = type_str.find('(') {
        // 查找对应的右括号
        if let Some(close_paren) = type_str.find(')') {
            // 提取基本类型
            let base_type = type_str[..open_paren].trim();
            // 提取精度值
            let precision_str = type_str[open_paren + 1..close_paren].trim();
            let precision = precision_str.parse::<u8>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            
            Ok((base_type.to_string(), precision))
        } else {
            Err(QueryExecutionError::TypeMismatch)
        }
    } else {
        // 没有精度，使用默认值
        Ok((type_str.trim().to_string(), 6)) // 默认精度6（微秒）
    }
}

/// 查询执行错误
#[derive(Debug, Clone, PartialEq)]
pub enum QueryExecutionError {
    /// 表未找到
    TableNotFound,
    /// 字段未找到
    FieldNotFound,
    /// 类型不匹配
    TypeMismatch,
    /// 无效的条件
    InvalidCondition,
    /// 内存不足
    OutOfMemory,
    /// 约束冲突
    ConstraintsConflicts,
    /// 内部错误
    InternalError,
    /// 不支持的函数
    UnsupportedFunction(String),
}

impl core::fmt::Display for QueryExecutionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QueryExecutionError::TableNotFound => write!(f, "Table not found"),
            QueryExecutionError::FieldNotFound => write!(f, "Field not found"),
            QueryExecutionError::TypeMismatch => write!(f, "Type mismatch"),
            QueryExecutionError::InvalidCondition => write!(f, "Invalid condition"),
            QueryExecutionError::OutOfMemory => write!(f, "Out of memory"),
            QueryExecutionError::ConstraintsConflicts => write!(f, "Constraints conflicts"),
            QueryExecutionError::InternalError => write!(f, "Internal error"),
            QueryExecutionError::UnsupportedFunction(func) => write!(f, "Unsupported function: {}", func),
        }
    }
}

impl core::error::Error for QueryExecutionError {} 

/// 执行SQL查询
pub fn execute_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 检查是否是时序表查询
    let is_timeseries_table = db.time_series_tables
        .iter()
        .any(|table_opt| {
            if let Some(table) = table_opt {
                table.def.base.name == query.table_name
            } else {
                false
            }
        });
    
    match query.query_type {
        crate::sql::QueryType::Select => {
            if is_timeseries_table {
                execute_select_timeseries_query(db, query)
            } else {
                execute_select_query(db, query)
            }
        },
        crate::sql::QueryType::Insert => execute_insert_query(db, query),
        crate::sql::QueryType::Update => execute_update_query(db, query),
        crate::sql::QueryType::Delete => execute_delete_query(db, query),
        crate::sql::QueryType::Describe => execute_describe_query(db, query),
        crate::sql::QueryType::CreateTable => execute_create_table_query(db, query),
        crate::sql::QueryType::CreateTimeSeriesTable => execute_create_time_series_table_query(db, query),
        crate::sql::QueryType::CreateIndex => execute_create_index_query(db, query),
        _ => Err(QueryExecutionError::InternalError),
    }
}

/// 查找时序表
fn find_timeseries_table_by_name<'a>(db: &'a RemDb, table_name: &str) -> Result<&'a TimeSeriesTable, QueryExecutionError> {
    for table in db.time_series_tables.iter() {
        if let Some(table) = table {
            if table.def.base.name == table_name {
                return Ok(table);
            }
        }
    }
    
    Err(QueryExecutionError::TableNotFound)
}

/// 执行时序表SELECT查询
fn execute_select_timeseries_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的时序表
    let ts_table = find_timeseries_table_by_name(db, &query.table_name)?;
    
    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回所有列（作为Field表达式）
            ts_table.def.base.fields
                .iter()
                .map(|field| Expression::Field {
                    name: field.name.to_string(),
                    alias: None,
                })
                .collect()
    } else {
        // 对于时序表，我们暂时只支持简单字段选择
        // TODO: 实现完整的表达式支持
        query.columns.clone()
    };
    
    // 3. 生成结果集的列名
    let result_columns = columns.iter()
        .map(|expr| {
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
        })
        .collect();
    
    // 4. 创建结果集
    let mut result_set = ResultSet::new(result_columns);
    
    // 5. 遍历时序表中的所有记录，收集匹配的记录
    let mut matched_rows: Vec<Vec<TypedValue>> = Vec::new();
    
    // TODO: 实现时序表的查询逻辑
    // 这里需要实现时序表的查询逻辑，包括：
    // 1. 解析WHERE条件
    // 2. 遍历时序表中的分区
    // 3. 遍历分区中的记录
    // 4. 应用WHERE条件过滤
    // 5. 收集匹配的记录
    
    // 6. 计算表达式值并添加到结果集
    for _ in &matched_rows {
        let mut row_data = Vec::with_capacity(columns.len());
        for expr in &columns {
            // TODO: 实现时序表表达式求值
            // 这里需要实现时序表的表达式求值逻辑
            // 暂时返回默认值
            let default_value = TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: 0 },
            };
            row_data.push(default_value);
        }
        result_set.add_row(row_data);
    }
    
    Ok(result_set)
}

/// 处理聚合查询
fn process_aggregate_query(
    columns: &[Expression],
    rows_to_process: &[Vec<TypedValue>],
    result_set: &mut ResultSet,
) -> Result<(), QueryExecutionError> {
    // 为每个聚合函数准备初始值
    let mut aggregate_values = Vec::with_capacity(columns.len());
    
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
                    },
                    "SUM" | "AVG" => {
                        // 初始化SUM/AVG为0
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                    },
                    "MIN" | "MAX" => {
                        // 初始化MIN/MAX为None（使用0作为占位符，后续会更新）
                        aggregate_values.push(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                    },
                    _ => {
                        return Err(QueryExecutionError::UnsupportedFunction(name.to_string()));
                    },
                }
            },
            _ => {
                // 非聚合列在聚合查询中应该有别名或分组
                // 这里暂时不处理，直接返回错误
                return Err(QueryExecutionError::InternalError);
            },
        }
    }
    
    // 遍历所有行，更新聚合值
    for record_values in rows_to_process {
        for (i, expr) in columns.iter().enumerate() {
            if let Expression::FunctionCall { name, args, .. } = expr {
                let name = name.to_uppercase();
                
                // 计算当前行的函数值
                let current_value = evaluate_expression_for_aggregate(args, record_values)?;
                
                // 更新聚合值
                match name.as_str() {
                    "COUNT" => {
                        unsafe {
                            // COUNT函数简单累加
                            aggregate_values[i].value.u64 += 1;
                        }
                    },
                    "SUM" => {
                        unsafe {
                            // SUM函数累加值
                            match current_value.value_type {
                                DataType::UInt8 => aggregate_values[i].value.u64 += current_value.value.u8 as u64,
                                DataType::UInt16 => aggregate_values[i].value.u64 += current_value.value.u16 as u64,
                                DataType::UInt32 => aggregate_values[i].value.u64 += current_value.value.u32 as u64,
                                DataType::UInt64 => aggregate_values[i].value.u64 += current_value.value.u64,
                                DataType::Int8 => aggregate_values[i].value.u64 += (current_value.value.i8 as i64).abs() as u64,
                                DataType::Int16 => aggregate_values[i].value.u64 += (current_value.value.i16 as i64).abs() as u64,
                                DataType::Int32 => aggregate_values[i].value.u64 += (current_value.value.i32 as i64).abs() as u64,
                                DataType::Int64 => aggregate_values[i].value.u64 += (current_value.value.i64).abs() as u64,
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }
                        }
                    },
                    "MIN" => {
                        // MIN函数取最小值
                        unsafe {
                            if aggregate_values[i].value.u64 == 0 {
                                // 第一次迭代，直接赋值
                                aggregate_values[i] = current_value;
                            } else {
                                // 比较并取最小值
                                let is_less = unsafe {
                                    match (aggregate_values[i].value_type, current_value.value_type) {
                                        (DataType::UInt8, DataType::UInt8) => current_value.value.u8 < aggregate_values[i].value.u8,
                                        (DataType::UInt16, DataType::UInt16) => current_value.value.u16 < aggregate_values[i].value.u16,
                                        (DataType::UInt32, DataType::UInt32) => current_value.value.u32 < aggregate_values[i].value.u32,
                                        (DataType::UInt64, DataType::UInt64) => current_value.value.u64 < aggregate_values[i].value.u64,
                                        (DataType::Int8, DataType::Int8) => current_value.value.i8 < aggregate_values[i].value.i8,
                                        (DataType::Int16, DataType::Int16) => current_value.value.i16 < aggregate_values[i].value.i16,
                                        (DataType::Int32, DataType::Int32) => current_value.value.i32 < aggregate_values[i].value.i32,
                                        (DataType::Int64, DataType::Int64) => current_value.value.i64 < aggregate_values[i].value.i64,
                                        _ => return Err(QueryExecutionError::TypeMismatch),
                                    }
                                };
                                if is_less {
                                    aggregate_values[i] = current_value;
                                }
                            }
                        }
                    },
                    "MAX" => {
                        // MAX函数取最大值
                        unsafe {
                            if aggregate_values[i].value.u64 == 0 {
                                // 第一次迭代，直接赋值
                                aggregate_values[i] = current_value;
                            } else {
                                // 比较并取最大值
                                let is_greater = unsafe {
                                    match (aggregate_values[i].value_type, current_value.value_type) {
                                        (DataType::UInt8, DataType::UInt8) => current_value.value.u8 > aggregate_values[i].value.u8,
                                        (DataType::UInt16, DataType::UInt16) => current_value.value.u16 > aggregate_values[i].value.u16,
                                        (DataType::UInt32, DataType::UInt32) => current_value.value.u32 > aggregate_values[i].value.u32,
                                        (DataType::UInt64, DataType::UInt64) => current_value.value.u64 > aggregate_values[i].value.u64,
                                        (DataType::Int8, DataType::Int8) => current_value.value.i8 > aggregate_values[i].value.i8,
                                        (DataType::Int16, DataType::Int16) => current_value.value.i16 > aggregate_values[i].value.i16,
                                        (DataType::Int32, DataType::Int32) => current_value.value.i32 > aggregate_values[i].value.i32,
                                        (DataType::Int64, DataType::Int64) => current_value.value.i64 > aggregate_values[i].value.i64,
                                        _ => return Err(QueryExecutionError::TypeMismatch),
                                    }
                                };
                                if is_greater {
                                    aggregate_values[i] = current_value;
                                }
                            }
                        }
                    },
                    "AVG" => {
                        // AVG函数需要同时计算总和和计数，最后再求平均
                        // 这里简化处理，只返回总和
                        unsafe {
                            match current_value.value_type {
                                DataType::UInt8 => aggregate_values[i].value.u64 += current_value.value.u8 as u64,
                                DataType::UInt16 => aggregate_values[i].value.u64 += current_value.value.u16 as u64,
                                DataType::UInt32 => aggregate_values[i].value.u64 += current_value.value.u32 as u64,
                                DataType::UInt64 => aggregate_values[i].value.u64 += current_value.value.u64,
                                DataType::Int8 => aggregate_values[i].value.u64 += (current_value.value.i8 as i64).abs() as u64,
                                DataType::Int16 => aggregate_values[i].value.u64 += (current_value.value.i16 as i64).abs() as u64,
                                DataType::Int32 => aggregate_values[i].value.u64 += (current_value.value.i32 as i64).abs() as u64,
                                DataType::Int64 => aggregate_values[i].value.u64 += (current_value.value.i64).abs() as u64,
                                DataType::Float32 => aggregate_values[i].value.u64 += (current_value.value.float32 as f64).abs() as u64,
                                DataType::Float64 => aggregate_values[i].value.u64 += current_value.value.float64.abs() as u64,
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            }
                        }
                    },
                    _ => return Err(QueryExecutionError::UnsupportedFunction(name.to_string())),
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
    }
    
    // 添加聚合结果到结果集
    result_set.add_row(aggregate_values);
    
    Ok(())
}

/// 为聚合函数计算表达式值
fn evaluate_expression_for_aggregate(
    args: &[Expression],
    record_values: &[TypedValue],
) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        // 对于COUNT(*), COUNT(1)等无参数情况
        return Ok(TypedValue {
            value_type: DataType::UInt64,
            value: Value { u64: 1 },
        });
    }
    
    // 简化处理，只处理第一个参数
    // TODO: 支持更复杂的表达式
    let arg = &args[0];
    match arg {
        Expression::Constant { value, .. } => {
            // 常量值，直接转换
            use crate::sql::Value as SqlValue;
            
            let (value_type, value) = match value {
                SqlValue::Integer(i) => (DataType::Int64, Value { i64: *i }),
                SqlValue::Float(f) => (DataType::Float64, Value { float64: *f }),
                SqlValue::String(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(s.as_bytes());
                    (DataType::String, Value { string: buf })
                },
                SqlValue::Boolean(b) => (DataType::Bool, Value { bool: *b }),
                SqlValue::Null => (DataType::Int64, Value { i64: 0 }),
            };
            
            Ok(TypedValue {
                value_type,
                value,
            })
        },
        _ => {
            // 对于其他表达式，这里简化处理，返回默认值
            // TODO: 支持更复杂的表达式
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 1 },
            })
        },
    }
}

/// 执行SELECT查询
fn execute_select_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表
    let table = find_table_by_name(db, &query.table_name)?;
    
    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回所有列（作为Field表达式）
            table.def.fields
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
    let result_columns = columns.iter()
        .map(|expr| {
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
        })
        .collect();
    
    // 4. 创建结果集
    let mut result_set = ResultSet::new(result_columns);
    
    // 5. 遍历表中的所有记录，收集匹配的记录
    let mut matched_rows = Vec::with_capacity(table.def.max_records);
    
    unsafe {
        // 遍历表中的所有记录，收集匹配的记录
        let iterate_result = table.iterate(|id, record_ptr| {
            // 检查记录是否符合WHERE条件
            let mut matches = true;
            if let Some(where_clause) = &query.where_clause {
                matches = evaluate_condition(table, record_ptr, &where_clause.condition);
            }
            
            if matches {
                // 直接从记录中提取字段值，创建行数据
                let mut record_values = Vec::with_capacity(table.def.fields.len());
                for field in table.def.fields.iter() {
                    match get_field_value(table, record_ptr, &field.name) {
                        Ok(typed_value) => record_values.push(typed_value),
                        Err(_) => return true, // 跳过错误记录，继续遍历
                    }
                }
                
                // 将匹配的记录值添加到向量中
                matched_rows.push(record_values);
            }
            
            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }
    
    // 6. 如果有ORDER BY子句，对记录进行排序
    if let Some(order_by) = &query.order_by {
        sort_rows(&mut matched_rows, table, order_by)?;
    }
    
    // 7. 应用LIMIT限制
    let limit = query.limit.unwrap_or(matched_rows.len());
    let rows_to_process = &matched_rows[..core::cmp::min(matched_rows.len(), limit)];
    
    // 8. 检查是否包含聚合函数
    let has_aggregate = columns.iter().any(|expr| {
        match expr {
            Expression::FunctionCall { name, .. } => {
                let name = name.to_uppercase();
                name == "COUNT" || name == "SUM" || name == "AVG" || name == "MIN" || name == "MAX"
            },
            _ => false,
        }
    });
    
    // 额外检查：如果是COUNT查询，确保作为聚合查询处理
    let is_count_query = columns.iter().any(|expr| {
        match expr {
            Expression::FunctionCall { name, .. } => {
                let name = name.to_uppercase();
                name == "COUNT"
            },
            _ => false,
        }
    });
    
    if has_aggregate || is_count_query {
        // 处理聚合查询
        process_aggregate_query(&columns, rows_to_process, &mut result_set)?;
    } else {
        // 处理普通查询
        for record_values in rows_to_process {
            let mut row_data = Vec::with_capacity(columns.len());
            for expr in &columns {
                let value = evaluate_expression(table, record_values, expr)?;
                row_data.push(value);
            }
            result_set.add_row(row_data);
        }
    }
    
    Ok(result_set)
}

/// 评估表达式值
fn evaluate_expression(
    table: &MemoryTable,
    record_values: &[TypedValue],
    expr: &Expression,
) -> Result<TypedValue, QueryExecutionError> {
    match expr {
        Expression::Field { name: field_name, .. } => {
            // 查找字段索引
            let field_index = table.def.fields
                .iter()
                .position(|field| field.name == field_name)
                .ok_or(QueryExecutionError::FieldNotFound)?;
            
            // 返回记录中的字段值
            Ok(record_values[field_index].clone())
        }
        Expression::FunctionCall { name, args, .. } => {
            // 评估函数参数
            let mut arg_values = Vec::with_capacity(args.len());
            for arg in args {
                arg_values.push(evaluate_expression(table, record_values, arg)?);
            }
            
            // 执行函数调用
            execute_function_call(name, &arg_values)
        }
        Expression::Constant { value: constant, .. } => {
            // 将sql::Value转换为types::TypedValue
            use crate::sql::Value as SqlValue;
            
            let (value_type, value) = match constant {
                SqlValue::Integer(i) => (DataType::Int64, Value { i64: *i }),
                SqlValue::Float(f) => (DataType::Float64, Value { float64: *f }),
                SqlValue::String(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(s.as_bytes());
                    (DataType::String, Value { string: buf })
                },
                SqlValue::Boolean(b) => (DataType::Bool, Value { bool: *b }),
                SqlValue::Null => (DataType::Int64, Value { i64: 0 }),
            };
            
            Ok(TypedValue {
                value_type,
                value,
            })
        }
        Expression::BinaryOp { left, op, right, .. } => {
            // 评估左右操作数
            let left_val = evaluate_expression(table, record_values, left)?;
            let right_val = evaluate_expression(table, record_values, right)?;
            
            // 执行二元操作
            evaluate_binary_op(left_val, *op, right_val)
        }
    }
}

/// 评估二元操作
fn evaluate_binary_op(
    left: TypedValue,
    op: BinaryOperator,
    right: TypedValue,
) -> Result<TypedValue, QueryExecutionError> {
    // 首先处理比较操作符
    match op {
        BinaryOperator::Equal | 
        BinaryOperator::NotEqual | 
        BinaryOperator::LessThan | 
        BinaryOperator::LessThanOrEqual | 
        BinaryOperator::GreaterThan | 
        BinaryOperator::GreaterThanOrEqual => {
            // 比较操作符需要返回布尔值
            unsafe {
                // 比较两个时间类型的值
                let t1 = match left.value_type {
                    DataType::Timestamp => left.value.time.value,
                    DataType::TimestampTZ => left.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                let t2 = match right.value_type {
                    DataType::Timestamp => right.value.time.value,
                    DataType::TimestampTZ => right.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                // 执行比较操作
                let result = match op {
                    BinaryOperator::Equal => t1 == t2,
                    BinaryOperator::NotEqual => t1 != t2,
                    BinaryOperator::LessThan => t1 < t2,
                    BinaryOperator::LessThanOrEqual => t1 <= t2,
                    BinaryOperator::GreaterThan => t1 > t2,
                    BinaryOperator::GreaterThanOrEqual => t1 >= t2,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                // 返回布尔结果
                return Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                });
            }
        },
        _ => {}, // 其他操作符继续处理
    }
    
    // 处理减法操作中两个时间类型相减的情况（Timestamp - Timestamp = Interval）
    if op == BinaryOperator::Subtract {
        unsafe {
            // 检查是否是时间类型之间的减法
            match (left.value_type, right.value_type) {
                (DataType::Timestamp, DataType::Timestamp) | 
                (DataType::TimestampTZ, DataType::TimestampTZ) |
                (DataType::Timestamp, DataType::TimestampTZ) |
                (DataType::TimestampTZ, DataType::Timestamp) => {
                    // 任意时间类型之间的减法都返回Interval
                    let t1 = left.value.time.value;
                    let t2 = right.value.time.value;
                    let diff = t1 - t2;
                    
                    return Ok(TypedValue {
                        value_type: DataType::Interval,
                        value: Value {
                            interval: crate::types::db_interval::new(diff, 6, 0)
                        },
                    });
                },
                _ => {}, // 其他情况继续处理
            }
        }
    }
    
    // 解析间隔值，支持字符串格式（如"1 HOUR"）和数值格式（微秒）
    let interval_micros = match right.value_type {
        DataType::Int64 => unsafe { right.value.i64 },
        DataType::String => {
            unsafe {
                let interval_str = core::str::from_utf8(&right.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                parse_interval_string(interval_str)?
            }
        },
        _ => return Err(QueryExecutionError::TypeMismatch),
    };
    
    match op {
        BinaryOperator::Add => {
            // 处理时间类型加法（时间 + 间隔 = 时间）
            unsafe {
                match left.value_type {
                    // Timestamp + Interval = Timestamp
                    DataType::Timestamp => {
                        let timestamp = left.value.time.value;
                        let new_timestamp = timestamp + interval_micros;
                        
                        Ok(TypedValue {
                            value_type: DataType::Timestamp,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0)
                            },
                        })
                    },
                    // TimestampTZ + Interval = TimestampTZ
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp + interval_micros;
                        
                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, tz_offset, 6, 0)
                            },
                        })
                    },
                    // 其他类型的加法操作（暂时不支持）
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        },
        BinaryOperator::Subtract => {
            // 处理时间类型减法（时间 - 间隔 = 时间）
            unsafe {
                match left.value_type {
                    // Timestamp - Interval = Timestamp
                    DataType::Timestamp => {
                        let timestamp = left.value.time.value;
                        let new_timestamp = timestamp - interval_micros;
                        
                        Ok(TypedValue {
                            value_type: DataType::Timestamp,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0)
                            },
                        })
                    },
                    // TimestampTZ - Interval = TimestampTZ
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp - interval_micros;
                        
                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(new_timestamp, tz_offset, 6, 0)
                            },
                        })
                    },
                    // 其他类型的减法操作（暂时不支持）
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        },
        // 处理比较操作符
        BinaryOperator::Equal | 
        BinaryOperator::NotEqual | 
        BinaryOperator::LessThan | 
        BinaryOperator::LessThanOrEqual | 
        BinaryOperator::GreaterThan | 
        BinaryOperator::GreaterThanOrEqual => {
            // 比较操作符需要返回布尔值
            unsafe {
                // 比较两个时间类型的值
                let t1 = match left.value_type {
                    DataType::Timestamp => left.value.time.value,
                    DataType::TimestampTZ => left.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                let t2 = match right.value_type {
                    DataType::Timestamp => right.value.time.value,
                    DataType::TimestampTZ => right.value.time.value,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                // 执行比较操作
                let result = match op {
                    BinaryOperator::Equal => t1 == t2,
                    BinaryOperator::NotEqual => t1 != t2,
                    BinaryOperator::LessThan => t1 < t2,
                    BinaryOperator::LessThanOrEqual => t1 <= t2,
                    BinaryOperator::GreaterThan => t1 > t2,
                    BinaryOperator::GreaterThanOrEqual => t1 >= t2,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                
                // 返回布尔结果
                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                })
            }
        },
    }
}

/// 执行函数调用
fn execute_function_call(
    name: &str,
    args: &[TypedValue],
) -> Result<TypedValue, QueryExecutionError> {
    match name.to_uppercase().as_str() {
        // 基础统计聚合函数
        "COUNT" => execute_count(args),
        "SUM" => execute_sum(args),
        "AVG" => execute_avg(args),
        "MIN" => execute_min(args),
        "MAX" => execute_max(args),
        "TIME_BUCKET" => execute_time_bucket(args),
        // 时间格式化函数
        "TO_ISO8601" => execute_to_iso8601(args),
        "TO_CHAR" => execute_to_char(args),
        "TO_EPOCH" => execute_to_epoch(args),
        _ => {
            // 不支持的函数
            Err(QueryExecutionError::UnsupportedFunction(name.to_string()))
        }
    }
}

/// 执行COUNT函数
fn execute_count(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    // COUNT函数返回记录数，这里简单返回1，实际聚合时会累加
    Ok(TypedValue {
        value_type: DataType::UInt64,
        value: Value { u64: 1 },
    })
}

/// 执行SUM函数
fn execute_sum(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    let arg = &args[0];
    
    // 根据参数类型返回对应的值
    unsafe {
        match arg.value_type {
            DataType::UInt8 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: arg.value.u8 as u64 },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: arg.value.u16 as u64 },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: arg.value.u32 as u64 },
            }),
            DataType::UInt64 => Ok(arg.clone()),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: arg.value.i8 as i64 },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: arg.value.i16 as i64 },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value { i64: arg.value.i32 as i64 },
            }),
            DataType::Int64 => Ok(arg.clone()),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.float32 as f64 },
            }),
            DataType::Float64 => Ok(arg.clone()),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行AVG函数
fn execute_avg(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    let arg = &args[0];
    
    // 转换为浮点数类型
    unsafe {
        match arg.value_type {
            DataType::UInt8 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.u8 as f64 },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.u16 as f64 },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.u32 as f64 },
            }),
            DataType::UInt64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.u64 as f64 },
            }),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.i8 as f64 },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.i16 as f64 },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.i32 as f64 },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.i64 as f64 },
            }),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: arg.value.float32 as f64 },
            }),
            DataType::Float64 => Ok(arg.clone()),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行MIN函数
fn execute_min(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    // MIN函数在聚合时会比较值，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行MAX函数
fn execute_max(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    // MAX函数在聚合时会比较值，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行TIME_BUCKET函数
fn execute_time_bucket(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    // 解析时间间隔参数
    let interval_micros = parse_time_interval(&args[0])?;
    
    // 获取时间戳参数
    let timestamp_arg = &args[1];
    
    unsafe {
        match timestamp_arg.value_type {
            DataType::Timestamp => {
                let timestamp = timestamp_arg.value.time.value;
                // 将时间戳对齐到指定的时间窗口
                let bucketed_timestamp = timestamp - (timestamp % interval_micros);
                
                Ok(TypedValue {
                    value_type: DataType::Timestamp,
                    value: Value { 
                        time: crate::types::db_timestamp::new(bucketed_timestamp, 0, 6, 0) 
                    },
                })
            },
            DataType::TimestampTZ => {
                let timestamp = timestamp_arg.value.time.value;
                let tz_offset = timestamp_arg.value.time.tz_offset;
                // 将时间戳对齐到指定的时间窗口
                let bucketed_timestamp = timestamp - (timestamp % interval_micros);
                
                Ok(TypedValue {
                    value_type: DataType::TimestampTZ,
                    value: Value { 
                        time: crate::types::db_timestamp::new(bucketed_timestamp, tz_offset, 6, 0) 
                    },
                })
            },
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_ISO8601函数
fn execute_to_iso8601(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    let timestamp_arg = &args[0];
    
    unsafe {
        match timestamp_arg.value_type {
            DataType::Timestamp | DataType::TimestampTZ => {
                let timestamp = &timestamp_arg.value.time;
                let result = process_to_iso8601(timestamp)?;
                
                // 将字符串转换为TypedValue
                let mut string_value = [0; MAX_STRING_LEN];
                let len = core::cmp::min(result.len(), MAX_STRING_LEN);
                string_value[..len].copy_from_slice(result.as_bytes());
                
                Ok(TypedValue {
                    value_type: DataType::String,
                    value: Value { string: string_value },
                })
            },
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_CHAR函数
fn execute_to_char(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    let timestamp_arg = &args[0];
    let format_arg = &args[1];
    
    unsafe {
        match (timestamp_arg.value_type, format_arg.value_type) {
            (DataType::Timestamp | DataType::TimestampTZ, DataType::String) => {
                let timestamp = &timestamp_arg.value.time;
                // 提取字符串格式
                let format_str = core::str::from_utf8(&format_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                
                let result = process_to_char(timestamp, format_str)?;
                
                // 将字符串转换为TypedValue
                let mut string_value = [0; MAX_STRING_LEN];
                let len = core::cmp::min(result.len(), MAX_STRING_LEN);
                string_value[..len].copy_from_slice(result.as_bytes());
                
                Ok(TypedValue {
                    value_type: DataType::String,
                    value: Value { string: string_value },
                })
            },
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行TO_EPOCH函数
fn execute_to_epoch(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }
    
    let timestamp_arg = &args[0];
    
    unsafe {
        match timestamp_arg.value_type {
            DataType::Timestamp | DataType::TimestampTZ => {
                let timestamp = &timestamp_arg.value.time;
                let result = process_to_epoch(timestamp)?;
                
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            },
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间间隔参数
fn parse_time_interval(interval_arg: &TypedValue) -> Result<i64, QueryExecutionError> {
    unsafe {
        match interval_arg.value_type {
            // 数值形式的时间间隔（微秒）
            DataType::UInt8 => Ok(interval_arg.value.u8 as i64),
            DataType::UInt16 => Ok(interval_arg.value.u16 as i64),
            DataType::UInt32 => Ok(interval_arg.value.u32 as i64),
            DataType::UInt64 => Ok(interval_arg.value.u64 as i64),
            DataType::Int8 => Ok(interval_arg.value.i8 as i64),
            DataType::Int16 => Ok(interval_arg.value.i16 as i64),
            DataType::Int32 => Ok(interval_arg.value.i32 as i64),
            DataType::Int64 => Ok(interval_arg.value.i64),
            DataType::Float32 => Ok(interval_arg.value.float32 as i64),
            DataType::Float64 => Ok(interval_arg.value.float64 as i64),
            // 字符串形式的时间间隔，如'5 minutes'、'1 hour'等
            DataType::String => {
                let interval_str = core::str::from_utf8(&interval_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                
                parse_interval_string(interval_str)
            },
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间间隔字符串
fn parse_interval_string(interval_str: &str) -> Result<i64, QueryExecutionError> {
    // 支持的时间单位
    let units = [
        ("ns", 1),           // 纳秒
        ("us", 1),           // 微秒
        ("ms", 1000),        // 毫秒
        ("s", 1000000),      // 秒
        ("sec", 1000000),     // 秒
        ("second", 1000000),  // 秒
        ("m", 60000000),      // 分钟
        ("min", 60000000),     // 分钟
        ("minute", 60000000),  // 分钟
        ("h", 3600000000),    // 小时
        ("hr", 3600000000),    // 小时
        ("hour", 3600000000),   // 小时
        ("d", 86400000000),   // 天
        ("day", 86400000000),   // 天
        ("w", 604800000000),  // 周
        ("week", 604800000000), // 周
    ];
    
    // 去除空格并转换为小写
    let normalized = interval_str.replace(" ", "").to_lowercase();
    
    // 查找匹配的时间单位
    for (unit, factor) in &units {
        if normalized.ends_with(unit) {
            // 提取数值部分
            let num_str = &normalized[..normalized.len() - unit.len()];
            let num = num_str.parse::<i64>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            // 计算微秒数
            return Ok(num * factor);
        }
    }
    
    // 无法解析的时间间隔
    Err(QueryExecutionError::TypeMismatch)
}

/// 执行CREATE TABLE查询
fn execute_create_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL数据类型转换为RemDb DataType
    let mut fields = Vec::new();
    let mut field_constraints = Vec::new(); // 存储约束信息
    
    for (field_name, data_type_str, is_primary_key, is_not_null, is_unique, is_auto_increment, default_value) in &query.table_def {
        // 解析数据类型，支持带精度的时间类型如TIMESTAMP(6)
        let (base_type, precision) = parse_data_type_with_precision(data_type_str)?;
        
        let data_type = match base_type.as_str() {
            // 无符号整数类型
            "UINT8" | "TINYINT UNSIGNED" => DataType::UInt8,
            "UINT16" | "SMALLINT UNSIGNED" => DataType::UInt16,
            "UINT32" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED" | "INTEGER UNSIGNED" => DataType::UInt32,
            "UINT64" | "BIGINT UNSIGNED" => DataType::UInt64,
            
            // 有符号整数类型
            "INT8" | "TINYINT" => DataType::Int8,
            "INT16" | "SMALLINT" => DataType::Int16,
            "INT32" | "MEDIUMINT" | "INT" | "INTEGER" => DataType::Int32,
            "INT64" | "BIGINT" => DataType::Int64,
            
            // 浮点数类型
            "FLOAT32" | "FLOAT" => DataType::Float32,
            "FLOAT64" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => DataType::Float64,
            
            // 布尔类型
            "BOOL" | "BOOLEAN" => DataType::Bool,
            
            // 时间类型
            "TIMESTAMP" | "DATETIME" | "DATE" | "TIME" => DataType::Timestamp,
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => DataType::TimestampTZ,
            
            // 字符串类型
            "STRING" | "TEXT" | "VARCHAR" | "NVARCHAR" | "CHAR" | "CLOB" => DataType::String,
            
            _ => return Err(QueryExecutionError::TypeMismatch),
        };
        
        // 转换query_parser::Value为types::Value
        let converted_default = match default_value {
            Some(sql_val) => {
                // 检查是否是时间函数调用（用0作为占位符）
                let is_time_function = match sql_val {
                    crate::sql::Value::Integer(i) => *i == 0,
                    _ => false,
                };
                
                let current_time = if is_time_function {
                    // 获取当前时间（微秒）
                    let now = crate::types::time_utils::now_micros();
                    now as i64
                } else {
                    0
                };
                
                let types_val = match sql_val {
                    crate::sql::Value::Integer(i) => {
                        // 如果是时间函数，使用当前时间替换占位符
                        let actual_value = if is_time_function && (data_type == DataType::Timestamp || data_type == DataType::TimestampTZ) {
                            current_time
                        } else {
                            *i as i64
                        };
                        
                        match data_type {
                            DataType::UInt8 => Value { u8: actual_value as u8 },
                            DataType::UInt16 => Value { u16: actual_value as u16 },
                            DataType::UInt32 => Value { u32: actual_value as u32 },
                            DataType::UInt64 => Value { u64: actual_value as u64 },
                            DataType::Int8 => Value { i8: actual_value as i8 },
                            DataType::Int16 => Value { i16: actual_value as i16 },
                            DataType::Int32 => Value { i32: actual_value as i32 },
                            DataType::Int64 => Value { i64: actual_value },
                            DataType::Bool => Value { bool: actual_value != 0 },
                            DataType::Float32 => Value { float32: actual_value as f32 },
                            DataType::Float64 => Value { float64: actual_value as f64 },
                            DataType::Timestamp => Value { time: crate::types::db_timestamp::new(actual_value, 0, precision, 0) },
                            DataType::TimestampTZ => Value { time: crate::types::db_timestamp::new(actual_value, 0, precision, 0) },
                            DataType::String => {
                                let mut s = [0; MAX_STRING_LEN];
                                let str_val = actual_value.to_string();
                                let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                                s[..len].copy_from_slice(str_val.as_bytes());
                                Value { string: s }
                            },
                            DataType::Interval => Value { interval: crate::types::db_interval::new(actual_value, precision, 0) },
                        }
                    },
                    crate::sql::Value::Float(f) => {
                        match data_type {
                            DataType::UInt8 => Value { u8: *f as u8 },
                            DataType::UInt16 => Value { u16: *f as u16 },
                            DataType::UInt32 => Value { u32: *f as u32 },
                            DataType::UInt64 => Value { u64: *f as u64 },
                            DataType::Int8 => Value { i8: *f as i8 },
                            DataType::Int16 => Value { i16: *f as i16 },
                            DataType::Int32 => Value { i32: *f as i32 },
                            DataType::Int64 => Value { i64: *f as i64 },
                            DataType::Bool => Value { bool: *f != 0.0 },
                            DataType::Float32 => Value { float32: *f as f32 },
                            DataType::Float64 => Value { float64: *f },
                            DataType::Timestamp => Value { time: crate::types::db_timestamp::new(*f as i64, 0, precision, 0) },
                            DataType::TimestampTZ => Value { time: crate::types::db_timestamp::new(*f as i64, 0, precision, 0) },
                            DataType::String => {
                                let mut s = [0; MAX_STRING_LEN];
                                let str_val = f.to_string();
                                let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                                s[..len].copy_from_slice(str_val.as_bytes());
                                Value { string: s }
                            },
                            DataType::Interval => Value { interval: crate::types::db_interval::new(*f as i64, precision, 0) },
                        }
                    },
                    crate::sql::Value::Boolean(b) => {
                        match data_type {
                            DataType::UInt8 => Value { u8: *b as u8 },
                            DataType::UInt16 => Value { u16: *b as u16 },
                            DataType::UInt32 => Value { u32: *b as u32 },
                            DataType::UInt64 => Value { u64: *b as u64 },
                            DataType::Int8 => Value { i8: *b as i8 },
                            DataType::Int16 => Value { i16: *b as i16 },
                            DataType::Int32 => Value { i32: *b as i32 },
                            DataType::Int64 => Value { i64: *b as i64 },
                            DataType::Bool => Value { bool: *b },
                            DataType::Float32 => Value { float32: (*b as i32) as f32 },
                            DataType::Float64 => Value { float64: (*b as i32) as f64 },
                            DataType::Timestamp => Value { time: crate::types::db_timestamp::new(*b as i64, 0, precision, 0) },
                            DataType::TimestampTZ => Value { time: crate::types::db_timestamp::new(*b as i64, 0, precision, 0) },
                            DataType::String => {
                                let mut s = [0; MAX_STRING_LEN];
                                let str_val = b.to_string();
                                let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                                s[..len].copy_from_slice(str_val.as_bytes());
                                Value { string: s }
                            },
                            DataType::Interval => Value { interval: crate::types::db_interval::new(*b as i64, precision, 0) },
                        }
                    },
                    crate::sql::Value::String(s) => {
                        match data_type {
                            DataType::UInt8 => Value { u8: s.parse().unwrap_or(0) },
                            DataType::UInt16 => Value { u16: s.parse().unwrap_or(0) },
                            DataType::UInt32 => Value { u32: s.parse().unwrap_or(0) },
                            DataType::UInt64 => Value { u64: s.parse().unwrap_or(0) },
                            DataType::Int8 => Value { i8: s.parse().unwrap_or(0) },
                            DataType::Int16 => Value { i16: s.parse().unwrap_or(0) },
                            DataType::Int32 => Value { i32: s.parse().unwrap_or(0) },
                            DataType::Int64 => Value { i64: s.parse().unwrap_or(0) },
                            DataType::Bool => Value { bool: s.parse().unwrap_or(false) },
                            DataType::Float32 => Value { float32: s.parse().unwrap_or(0.0) },
                            DataType::Float64 => Value { float64: s.parse().unwrap_or(0.0) },
                            DataType::Timestamp => Value { time: crate::types::db_timestamp::new(s.parse().unwrap_or(0) as i64, 0, precision, 0) },
                            DataType::TimestampTZ => Value { time: crate::types::db_timestamp::new(s.parse().unwrap_or(0) as i64, 0, precision, 0) },
                            DataType::String => {
                                let mut buf = [0; MAX_STRING_LEN];
                                let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                                buf[..len].copy_from_slice(s.as_bytes());
                                Value { string: buf }
                            },
                            DataType::Interval => Value { interval: crate::types::db_interval::new(s.parse().unwrap_or(0) as i64, precision, 0) },
                        }
                    },
                    crate::sql::Value::Null => {
                        // 对于NULL默认值，根据数据类型生成适当的默认值
                        match data_type {
                            DataType::UInt8 => Value { u8: 0 },
                            DataType::UInt16 => Value { u16: 0 },
                            DataType::UInt32 => Value { u32: 0 },
                            DataType::UInt64 => Value { u64: 0 },
                            DataType::Int8 => Value { i8: 0 },
                            DataType::Int16 => Value { i16: 0 },
                            DataType::Int32 => Value { i32: 0 },
                            DataType::Int64 => Value { i64: 0 },
                            DataType::Bool => Value { bool: false },
                            DataType::Float32 => Value { float32: 0.0 },
                            DataType::Float64 => Value { float64: 0.0 },
                            DataType::Timestamp => Value { time: crate::types::db_timestamp::new(0, 0, precision, 0) },
                            DataType::TimestampTZ => Value { time: crate::types::db_timestamp::new(0, 0, precision, 0) },
                            DataType::String => Value { string: [0; MAX_STRING_LEN] },
                            DataType::Interval => Value { interval: crate::types::db_interval::new(0, precision, 0) },
                        }
                    },
                };
                Some(types_val)
            },
            None => None,
        };
        
        // 保存字段和约束信息
    fields.push((field_name.as_str(), data_type, converted_default));
    
    // 转换为FieldConstraint对象
    let field_constraint = crate::FieldConstraint {
        primary_key: *is_primary_key,
        not_null: *is_not_null,
        unique: *is_unique,
        auto_increment: *is_auto_increment,
    };
    field_constraints.push(field_constraint);
}

// 查找主键字段索引
let primary_key_index = query.primary_key.as_ref().and_then(|pk| {
    query.table_def.iter().position(|(name, _, _, _, _, _, _)| name == pk)
});

// 调用DdlExecutor::create_table方法，支持约束
DdlExecutor::create_table(
    db,
    &query.table_name,
    &fields,
    Some(&field_constraints),
    primary_key_index
).map_err(|e| {
    match e {
        RemDbError::TableNotFound => QueryExecutionError::TableNotFound,
        RemDbError::FieldNotFound => QueryExecutionError::FieldNotFound,
        RemDbError::TypeMismatch => QueryExecutionError::TypeMismatch,
        RemDbError::OutOfMemory => QueryExecutionError::OutOfMemory,
        _ => QueryExecutionError::InternalError,
    }
})?;
    
    // 创建结果集，返回成功消息
    let columns = vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(vec![TypedValue {
        value_type: DataType::String,
        value: Value { string: [b'0'; 64] },
    }]);
    
    Ok(result_set)
}

/// 执行CREATE INDEX查询
fn execute_create_index_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL索引类型转换为RemDb IndexType
    let index_type = match query.index_type.as_deref() {
        Some("BTREE") => IndexType::BTree,
        Some("TTREE") => IndexType::TTree,
        Some("SORTEDARRAY") => IndexType::SortedArray,
        _ => IndexType::BTree, // 默认值
    };
    
    // 调用DdlExecutor的create_index方法
    let field_name = query.index_column.as_ref().ok_or(QueryExecutionError::InvalidCondition)?;
    db.create_index(
        &query.table_name,
        field_name,
        index_type
    ).map_err(|e| {
        match e {
            RemDbError::TableNotFound => QueryExecutionError::TableNotFound,
            RemDbError::FieldNotFound => QueryExecutionError::FieldNotFound,
            _ => QueryExecutionError::InternalError,
        }
    })?;
    
    // 创建结果集，返回成功消息
    let columns = vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(vec![TypedValue {
        value_type: DataType::String,
        value: Value { string: [b'0'; 64] },
    }]);
    
    Ok(result_set)
}

/// 查找表
fn find_table_by_name<'a>(db: &'a RemDb, table_name: &str) -> Result<&'a MemoryTable, QueryExecutionError> {
    for table in db.tables.iter() {
        if let Some(table) = table {
            if table.def.name == table_name {
                return Ok(table);
            }
        }
    }
    
    Err(QueryExecutionError::TableNotFound)
}

/// 验证表达式中的字段名是否有效
fn validate_expression(table: &MemoryTable, expr: &Expression) -> Result<(), QueryExecutionError> {
    match expr {
        Expression::Field { name: field_name, .. } => {
            if !table.def.fields.iter().any(|field| field.name == field_name) {
                return Err(QueryExecutionError::FieldNotFound);
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
    }
    
    Ok(())
}

/// 验证列表达式是否有效
fn validate_columns(table: &MemoryTable, columns: &[Expression]) -> Result<(), QueryExecutionError> {
    for column in columns {
        validate_expression(table, column)?;
    }
    
    Ok(())
}



/// 执行DESCRIBE TABLE查询
fn execute_describe_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表定义（同时检查普通表和时序表）
    let mut table_def: Option<&TableDef> = None;
    
    // 查找普通表
    for table_opt in db.tables.iter() {
        if let Some(table) = table_opt {
            if table.def.name == query.table_name {
                table_def = Some(&table.def);
                break;
            }
        }
    }
    
    // 如果普通表未找到，查找时序表
    if table_def.is_none() {
        for ts_table_opt in db.time_series_tables.iter() {
            if let Some(ts_table) = ts_table_opt {
                if ts_table.def.base.name == query.table_name {
                    table_def = Some(&ts_table.def.base);
                    break;
                }
            }
        }
    }
    
    // 如果都未找到，返回错误
    let table_def = table_def.ok_or(QueryExecutionError::TableNotFound)?;
    
    // 2. 定义结果集列名
    let columns = vec![
        "Field".to_string(),
        "Type".to_string(),
        "Key".to_string(),
        "Null".to_string(),
        "Default".to_string()
    ];
    
    // 3. 创建结果集
    let mut result_set = ResultSet::new(columns.clone());
    
    // 4. 添加字段信息到结果集
    // 注意：由于describe查询返回的是表结构信息，而不是实际数据，
    // 我们需要特殊处理，将描述信息转换为Value类型
    // 使用索引迭代而非直接迭代，避免可能的无限循环
    for i in 0..table_def.fields.len() {
        let field = &table_def.fields[i];
        // 确定是否为主键
        let is_primary_key = table_def.primary_key < table_def.fields.len() && 
                             table_def.fields[table_def.primary_key].name == field.name;
        let key_str = if is_primary_key {
            "PRI"
        } else if field.unique {
            "UNI"
        } else {
            ""
        };
        
        // 确定是否允许NULL
        let null_str = if field.not_null { "NO" } else { "YES" };
        
        // 确定默认值
        let default_str = if let Some(default_val) = &field.default_value {
            // 根据字段类型格式化默认值
            match field.data_type {
                // 整数类型
                DataType::UInt8 => format!("{}", unsafe { default_val.u8 }),
                DataType::UInt16 => format!("{}", unsafe { default_val.u16 }),
                DataType::UInt32 => format!("{}", unsafe { default_val.u32 }),
                DataType::UInt64 => format!("{}", unsafe { default_val.u64 }),
                DataType::Int8 => format!("{}", unsafe { default_val.i8 }),
                DataType::Int16 => format!("{}", unsafe { default_val.i16 }),
                DataType::Int32 => format!("{}", unsafe { default_val.i32 }),
                DataType::Int64 => format!("{}", unsafe { default_val.i64 }),
                // 布尔类型
                DataType::Bool => format!("{}", unsafe { default_val.bool }),
                // 浮点数类型
                DataType::Float32 => format!("{}", unsafe { default_val.float32 }),
                DataType::Float64 => format!("{}", unsafe { default_val.float64 }),
                // 时间类型
                DataType::Timestamp => format!("{}", unsafe { default_val.time.value }),
                DataType::TimestampTZ => format!("{}", unsafe { default_val.time.value }),
                // 字符串类型
                DataType::String => {
                    let str_val = unsafe { &default_val.string };
                    String::from_utf8_lossy(str_val).trim_end_matches(char::from(0)).to_string()
                },
                // 时间间隔类型
                DataType::Interval => format!("{}", unsafe { default_val.interval.value }),
            }
        } else {
            "".to_string()
        };
        
        // 确定字段类型字符串表示
        let type_str = match field.data_type {
            crate::DataType::UInt8 => "tinyint".to_string(),
            crate::DataType::UInt16 => "smallint".to_string(),
            crate::DataType::UInt32 => "int".to_string(),
            crate::DataType::UInt64 => "bigint".to_string(),
            crate::DataType::Int8 => "tinyint".to_string(),
            crate::DataType::Int16 => "smallint".to_string(),
            crate::DataType::Int32 => "int".to_string(),
            crate::DataType::Int64 => "bigint".to_string(),
            crate::DataType::String => format!("varchar({})", field.size),
            crate::DataType::Bool => "bool".to_string(),
            crate::DataType::Timestamp => "timestamp".to_string(),
            crate::DataType::TimestampTZ => "timestamp with time zone".to_string(),
            crate::DataType::Float32 => "float".to_string(),
            crate::DataType::Float64 => "double".to_string(),
            crate::DataType::Interval => "interval".to_string(),
        };
        
        // 创建行数据
        // 由于Value是union类型，我们需要确保每个值都被正确初始化
        // 对于字符串值，我们使用string字段并确保它是一个有效的C风格字符串
        let mut field_name_val = crate::Value { string: [0u8; 64] };
        let field_name_bytes = field.name.as_bytes();
        let field_name_len = core::cmp::min(field_name_bytes.len(), 63);
        unsafe {
            field_name_val.string[..field_name_len].copy_from_slice(&field_name_bytes[..field_name_len]);
        }
        let field_name_typed_val = TypedValue {
            value_type: DataType::String,
            value: field_name_val,
        };
        
        let mut type_val = crate::Value { string: [0u8; 64] };
        let type_bytes = type_str.as_bytes();
        let type_len = core::cmp::min(type_bytes.len(), 63);
        unsafe {
            type_val.string[..type_len].copy_from_slice(&type_bytes[..type_len]);
        }
        let type_typed_val = TypedValue {
            value_type: DataType::String,
            value: type_val,
        };
        
        let mut key_val = crate::Value { string: [0u8; 64] };
        let key_bytes = key_str.as_bytes();
        let key_len = core::cmp::min(key_bytes.len(), 63);
        unsafe {
            key_val.string[..key_len].copy_from_slice(&key_bytes[..key_len]);
        }
        let key_typed_val = TypedValue {
            value_type: DataType::String,
            value: key_val,
        };
        
        let mut null_val = crate::Value { string: [0u8; 64] };
        let null_bytes = null_str.as_bytes();
        let null_len = core::cmp::min(null_bytes.len(), 63);
        unsafe {
            null_val.string[..null_len].copy_from_slice(&null_bytes[..null_len]);
        }
        let null_typed_val = TypedValue {
            value_type: DataType::String,
            value: null_val,
        };
        
        let mut default_val = crate::Value { string: [0u8; 64] };
        let default_bytes = default_str.as_bytes();
        let default_len = core::cmp::min(default_bytes.len(), 63);
        unsafe {
            default_val.string[..default_len].copy_from_slice(&default_bytes[..default_len]);
        }
        let default_typed_val = TypedValue {
            value_type: DataType::String,
            value: default_val,
        };
        
        let row_data = vec![
            field_name_typed_val, // Field name
            type_typed_val,       // Type
            key_typed_val,        // Key
            null_typed_val,       // Null
            default_typed_val,    // Default
        ];
        
        result_set.add_row(row_data);
    }
    
    Ok(result_set)
}

/// 执行INSERT查询
fn execute_insert_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要插入的表的ID
    let table_id = db.tables
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
    let table = db.get_table_mut(table_id).map_err(|_| QueryExecutionError::InternalError)?;
    
    // 3. 验证插入的字段名
    if !query.insert_columns.is_empty() {
        // 插入指定列，验证列名是否存在
        for col_name in &query.insert_columns {
            table.def.fields
                .iter()
                .position(|field| field.name == col_name)
                .ok_or(QueryExecutionError::FieldNotFound)?;
        }
    }
    
    // 4. 执行插入操作
    let mut affected_rows = 0;
    
    for values in &query.values {
        // 5. 创建记录数据缓冲区并初始化为0
        let mut record_data = vec![0; table.record_size];
        
        // 6. 将字段值写入缓冲区
        for (i, field) in table.def.fields.iter().enumerate() {
            let field_value = if !query.insert_columns.is_empty() {
                // 插入指定列
                if let Some(col_index) = query.insert_columns.iter().position(|col| col == field.name) {
                    if col_index < values.len() {
                        Some(&values[col_index])
                    } else {
                        None
                    }
                } else {
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
            
            // 如果是自动递增主键且未提供值，则生成唯一值
            if is_pk_auto_incr && field_value.is_none() {
                // 生成自动递增主键值
                // 使用表中已维护的最大主键值
                let max_pk = table.max_pk;
                
                // 生成新的主键值
                let new_pk = max_pk + 1;
                
                // 更新表的最大主键值
                table.max_pk = new_pk;
                
                // 将新的主键值写入记录
                unsafe {
                    match field.data_type {
                        DataType::UInt8 => {
                            record_data[field.offset] = new_pk as u8;
                        },
                        DataType::UInt16 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u16, new_pk as u16);
                        },
                        DataType::UInt32 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u32, new_pk as u32);
                        },
                        DataType::UInt64 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u64, new_pk);
                        },
                        DataType::Int8 => {
                            record_data[field.offset] = new_pk as i8 as u8;
                        },
                        DataType::Int16 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i16, new_pk as i16);
                        },
                        DataType::Int32 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i32, new_pk as i32);
                        },
                        DataType::Int64 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i64, new_pk as i64);
                        },
                        _ => {}
                    }
                }
            } else if let Some(sql_value) = field_value {
                // 转换并设置字段值
                set_field_value(&mut record_data, field.offset, field.data_type, field.size, sql_value)?;
            } else if let Some(default_value) = field.default_value {
                // 使用字段默认值
                // 直接写入默认值，因为default_value是types::Value类型
                unsafe {
                    match field.data_type {
                        DataType::UInt8 => {
                            record_data[field.offset] = default_value.u8;
                        },
                        DataType::UInt16 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u16, default_value.u16);
                        },
                        DataType::UInt32 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u32, default_value.u32);
                        },
                        DataType::UInt64 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut u64, default_value.u64);
                        },
                        DataType::Int8 => {
                            record_data[field.offset] = default_value.i8 as u8;
                        },
                        DataType::Int16 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i16, default_value.i16);
                        },
                        DataType::Int32 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i32, default_value.i32);
                        },
                        DataType::Int64 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut i64, default_value.i64);
                        },
                        DataType::Float32 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut f32, default_value.float32);
                        },
                        DataType::Float64 => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut f64, default_value.float64);
                        },
                        DataType::Bool => {
                            record_data[field.offset] = default_value.bool as u8;
                        },
                        DataType::Timestamp => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut crate::types::db_timestamp, default_value.time);
                        },
                        DataType::TimestampTZ => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut crate::types::db_timestamp, default_value.time);
                        },
                        DataType::String => {
                            core::ptr::copy_nonoverlapping(default_value.string.as_ptr(), record_data.as_mut_ptr().add(field.offset), field.size);
                        },
                        DataType::Interval => {
                            core::ptr::write_unaligned(record_data.as_mut_ptr().add(field.offset) as *mut crate::types::db_interval, default_value.interval);
                        },
                    }
                }
            }
        }
        
        // 7. 调用表的插入方法
        match table.insert(record_data.as_ptr()) {
            Ok(_) => affected_rows += 1,
            Err(e) => {
                match e {
                    RemDbError::DuplicateKey => {
                        if query.ignore_duplicates {
                            // 忽略重复键，继续处理下一条记录
                            continue;
                        } else {
                            return Err(QueryExecutionError::ConstraintsConflicts);
                        }
                    },
                    RemDbError::InvalidRecordSize | RemDbError::TypeMismatch => {
                        return Err(QueryExecutionError::ConstraintsConflicts);
                    },
                    RemDbError::OutOfMemory => {
                        return Err(QueryExecutionError::OutOfMemory);
                    },
                    _ => {
                        return Err(QueryExecutionError::InternalError);
                    },
                }
            },
        }
    }
    
    // 8. 创建结果集，返回受影响的行数
    let columns = vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);
    
    let row_data = vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value { u64: affected_rows as u64 },
    }];
    result_set.add_row(row_data);
    
    Ok(result_set)
}

/// 执行DELETE查询
fn execute_delete_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要删除的表的ID
    let table_id = db.tables
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
    let table_ref = db.tables[table_id].as_ref().ok_or(QueryExecutionError::TableNotFound)?;
    
    // 3. 遍历表中的所有记录，收集要删除的记录ID
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
    let table_mut = db.get_table_mut(table_id).map_err(|_| QueryExecutionError::InternalError)?;
    
    // 5. 执行删除操作
    let mut affected_rows = 0;
    for id in to_delete {
        match unsafe { table_mut.delete(id) } {
            Ok(_) => affected_rows += 1,
            Err(_) => continue, // 跳过删除失败的记录
        }
    }
    
    // 6. 创建结果集，返回受影响的行数
    let columns = vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);
    
    let row_data = vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value { u64: affected_rows as u64 },
    }];
    result_set.add_row(row_data);
    
    Ok(result_set)
}

/// 执行UPDATE查询
fn execute_update_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要更新的表的ID
    let table_id = db.tables
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
    let table_ref = db.tables[table_id].as_ref().ok_or(QueryExecutionError::TableNotFound)?;
    let record_size = table_ref.record_size;
    
    // 3. 遍历表中的所有记录，收集要更新的记录ID和它们的当前数据
    let mut to_update = Vec::new();
    
    unsafe {
        // 遍历表中的所有记录
        let iterate_result = table_ref.iterate(|id, record_ptr| {
            // 检查记录是否符合WHERE条件
            let mut matches = true;
            if let Some(where_clause) = &query.where_clause {
                matches = evaluate_condition(table_ref, record_ptr, &where_clause.condition);
            }
            
            if matches {
                // 复制记录数据到临时缓冲区
                let mut record_data = vec![0; record_size];
                core::ptr::copy_nonoverlapping(record_ptr, record_data.as_mut_ptr(), record_size);
                to_update.push((id, record_data));
            }
            
            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }
    
    // 4. 获取可变表引用（用于更新）
    let table_mut = db.get_table_mut(table_id).map_err(|_| QueryExecutionError::InternalError)?;
    
    // 5. 执行更新操作
    let mut affected_rows = 0;
    for (id, mut record_data) in to_update {
        // 遍历所有要更新的字段值对
        for (field_name, new_value) in &query.update_pairs {
            // 查找字段索引
            let field_index = table_mut.def.fields
                .iter()
                .position(|field| field.name == field_name)
                .ok_or(QueryExecutionError::FieldNotFound)?;
            
            let field = &table_mut.def.fields[field_index];
            
            // 设置新的字段值
            set_field_value(&mut record_data, field.offset, field.data_type, field.size, new_value)?;
        }
        
        // 获取记录指针并写入更新后的数据
        let record_ptr = unsafe { table_mut.get_record_ptr_mut(id) };
        unsafe {
            core::ptr::copy_nonoverlapping(record_data.as_ptr(), record_ptr, record_size);
        }
        
        // 更新记录版本号
        let status_ptr = unsafe { table_mut.get_status_ptr(id) };
        let status = unsafe { &mut *status_ptr };
        status.version += 1;
        
        affected_rows += 1;
    }
    
    // 6. 创建结果集，返回受影响的行数
    let columns = vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);
    
    let row_data = vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value { u64: affected_rows as u64 },
    }];
    result_set.add_row(row_data);
    
    Ok(result_set)
}

/// 设置字段值
fn set_field_value(record_data: &mut Vec<u8>, offset: usize, data_type: DataType, field_size: usize, sql_value: &crate::sql::Value) -> Result<(), QueryExecutionError> {
    unsafe {
        // 辅助函数：将SQL值转换为整数
        let to_integer = |sql_val: &crate::sql::Value| -> Result<i64, QueryExecutionError> {
            match sql_val {
                crate::sql::Value::Integer(i) => Ok(*i),
                crate::sql::Value::Float(f) => Ok(*f as i64),
                crate::sql::Value::Boolean(b) => Ok(*b as i64),
                crate::sql::Value::String(s) => {
                    s.parse::<i64>().map_err(|_| QueryExecutionError::TypeMismatch)
                },
                _ => Err(QueryExecutionError::TypeMismatch),
            }
        };
        
        match data_type {
            // 无符号整数类型
            DataType::UInt8 => {
                let value = to_integer(sql_value)? as u8;
                // u8不需要对齐，直接复制
                record_data[offset] = value;
            },
            DataType::UInt16 => {
                let value = to_integer(sql_value)? as u16;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u16, value);
            },
            DataType::UInt32 => {
                let value = to_integer(sql_value)? as u32;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u32, value);
            },
            DataType::UInt64 => {
                let value = to_integer(sql_value)? as u64;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u64, value);
            },
            
            // 有符号整数类型
            DataType::Int8 => {
                let value = to_integer(sql_value)? as i8;
                // i8不需要对齐，直接复制
                record_data[offset] = value as u8;
            },
            DataType::Int16 => {
                let value = to_integer(sql_value)? as i16;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i16, value);
            },
            DataType::Int32 => {
                let value = to_integer(sql_value)? as i32;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i32, value);
            },
            DataType::Int64 => {
                let value = to_integer(sql_value)?;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i64, value);
            },
            
            // 浮点数类型
            DataType::Float32 => {
                let value = match sql_value {
                    crate::sql::Value::Float(f) => *f as f32,
                    crate::sql::Value::Integer(i) => *i as f32,
                    crate::sql::Value::Boolean(b) => (*b as u8) as f32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut f32, value);
            },
            DataType::Float64 => {
                let value = match sql_value {
                    crate::sql::Value::Float(f) => *f,
                    crate::sql::Value::Integer(i) => *i as f64,
                    crate::sql::Value::Boolean(b) => (*b as u8) as f64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut f64, value);
            },
            
            // 布尔类型
            DataType::Bool => {
                let value = match sql_value {
                    crate::sql::Value::Boolean(b) => *b,
                    crate::sql::Value::Integer(i) => *i != 0,
                    crate::sql::Value::Float(f) => *f != 0.0,
                    crate::sql::Value::String(s) => {
                        s.parse::<bool>().map_err(|_| QueryExecutionError::TypeMismatch)?
                    },
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // bool不需要对齐，直接复制
                record_data[offset] = value as u8;
            },
            
            // 时间戳类型
            DataType::Timestamp => {
                // 处理时间函数调用和普通时间值
                let timestamp = match sql_value {
                    // 处理时间函数调用（占位符值为0）
                    crate::sql::Value::Integer(i) if *i == 0 => {
                        // 获取当前时间（微秒）
                        let now = crate::types::time_utils::now_micros() as i64;
                        crate::types::db_timestamp::new(now, 0, 6, 0)
                    },
                    // 处理普通时间值
                    _ => {
                        let value = to_integer(sql_value)?;
                        crate::types::db_timestamp::new(value, 0, 6, 0)
                    },
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut crate::types::db_timestamp, timestamp);
            },
            DataType::TimestampTZ => {
                // 处理时间函数调用和普通时间值
                let timestamp = match sql_value {
                    // 处理时间函数调用（占位符值为0）
                    crate::sql::Value::Integer(i) if *i == 0 => {
                        // 获取当前时间（微秒）
                        let now = crate::types::time_utils::now_micros() as i64;
                        crate::types::db_timestamp::new(now, 0, 6, 0)
                    },
                    // 处理普通时间值
                    _ => {
                        let value = to_integer(sql_value)?;
                        crate::types::db_timestamp::new(value, 0, 6, 0)
                    },
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut crate::types::db_timestamp, timestamp);
            },
            
            // 字符串类型
            DataType::String => {
                let str_value = match sql_value {
                    crate::sql::Value::String(s) => s,
                    crate::sql::Value::Integer(i) => &i.to_string(),
                    crate::sql::Value::Float(f) => &f.to_string(),
                    crate::sql::Value::Boolean(b) => &b.to_string(),
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
            },
            // 时间间隔类型
            DataType::Interval => {
                let interval_value = to_integer(sql_value)?;
                let interval = crate::types::db_interval::new(interval_value, 6, 0); // 默认精度6（微秒）
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut crate::types::db_interval, interval);
            },
        }
    }
    
    Ok(())
}

/// 执行CREATE TIMESERIES TABLE查询
fn execute_create_time_series_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 时序表创建逻辑：
    // 1. 必须包含一个TIMESTAMP类型的time_field
    // 2. 必须包含一个数值类型的value_field
    // 3. 可以包含多个标签字段
    
    // 解析字段定义，查找时间字段、值字段和标签字段
    let mut time_field = None;
    let mut value_field = None;
    let mut tag_fields = Vec::new();
    
    for (field_name, data_type_str, _, _, _, _, _) in &query.table_def {
        let data_type = match data_type_str.to_uppercase().as_str() {
            "TIMESTAMP" | "DATETIME" | "DATE" | "TIME" => crate::DataType::Timestamp,
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => crate::DataType::TimestampTZ,
            "UINT8" | "TINYINT UNSIGNED" => crate::DataType::UInt8,
            "UINT16" | "SMALLINT UNSIGNED" => crate::DataType::UInt16,
            "UINT32" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED" | "INTEGER UNSIGNED" => crate::DataType::UInt32,
            "UINT64" | "BIGINT UNSIGNED" => crate::DataType::UInt64,
            "INT8" | "TINYINT" => crate::DataType::Int8,
            "INT16" | "SMALLINT" => crate::DataType::Int16,
            "INT32" | "MEDIUMINT" | "INT" | "INTEGER" => crate::DataType::Int32,
            "INT64" | "BIGINT" => crate::DataType::Int64,
            "FLOAT32" | "FLOAT" => crate::DataType::Float32,
            "FLOAT64" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => crate::DataType::Float64,
            "BOOL" | "BOOLEAN" => crate::DataType::Bool,
            "STRING" | "TEXT" | "VARCHAR" | "NVARCHAR" | "CHAR" | "CLOB" => crate::DataType::String,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };
        
        match data_type {
            // 时间字段：TIMESTAMP或TIMESTAMPTZ类型
            crate::DataType::Timestamp | crate::DataType::TimestampTZ => {
                if time_field.is_none() {
                    time_field = Some(field_name.as_str());
                } else {
                    // 只能有一个时间字段
                    return Err(QueryExecutionError::InternalError);
                }
            },
            // 值字段：数值类型
            crate::DataType::UInt8 | crate::DataType::UInt16 | crate::DataType::UInt32 | 
            crate::DataType::UInt64 | crate::DataType::Int8 | crate::DataType::Int16 | 
            crate::DataType::Int32 | crate::DataType::Int64 | crate::DataType::Float32 | 
            crate::DataType::Float64 => {
                if value_field.is_none() {
                    value_field = Some(field_name.as_str());
                }
            },
            // 标签字段：其他类型（通常是字符串或布尔值）
            _ => {
                tag_fields.push(field_name.as_str());
            }
        }
    }
    
    // 验证必须的字段
    let time_field = time_field.ok_or(QueryExecutionError::InternalError)?;
    let value_field = value_field.ok_or(QueryExecutionError::InternalError)?;
    
    // 调用RemDb的create_time_series_table方法创建时序表
    db.create_time_series_table(
        &query.table_name,
        time_field,
        value_field,
        &tag_fields,
        None
    ).map_err(|e| {
        match e {
            crate::RemDbError::OutOfMemory => QueryExecutionError::OutOfMemory,
            _ => QueryExecutionError::InternalError,
        }
    })?;
    
    // 创建结果集，返回成功消息
    let columns = vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(vec![TypedValue {
        value_type: crate::DataType::String,
        value: crate::Value { string: [b'0'; 64] },
    }]);
    
    Ok(result_set)
}

/// 评估条件
unsafe fn evaluate_condition(table: &MemoryTable, record_ptr: *const u8, condition: &Condition) -> bool {
    match condition {
        Condition::Comparison(comp) => evaluate_comparison(table, record_ptr, comp),
        Condition::Between(between) => evaluate_between(table, record_ptr, between),
        Condition::And(left, right) => {
            evaluate_condition(table, record_ptr, left) && 
            evaluate_condition(table, record_ptr, right)
        },
        Condition::Or(left, right) => {
            evaluate_condition(table, record_ptr, left) || 
            evaluate_condition(table, record_ptr, right)
        },
    }
}

/// 评估BETWEEN条件
unsafe fn evaluate_between(table: &MemoryTable, record_ptr: *const u8, between: &BetweenCondition) -> bool {
    // 获取字段索引
    let field_index = match table.def.fields
        .iter()
        .position(|field| field.name == &between.field) {
        Some(index) => index,
        None => return false, // 字段不存在，条件不成立
    };
    
    let field_type = table.def.fields[field_index].data_type;
    
    // 获取字段值
    match get_field_value(table, record_ptr, &between.field) {
        Ok(field_value) => {
            // BETWEEN条件：field_value >= min_value AND field_value <= max_value
            let is_greater_or_equal = compare_values(&field_value.value, field_type, &ComparisonOperator::GreaterThanOrEqual, &between.min_value);
            let is_less_or_equal = compare_values(&field_value.value, field_type, &ComparisonOperator::LessThanOrEqual, &between.max_value);
            is_greater_or_equal && is_less_or_equal
        },
        Err(_) => false,
    }
}

/// 评估比较条件
unsafe fn evaluate_comparison(table: &MemoryTable, record_ptr: *const u8, comp: &ComparisonCondition) -> bool {
    // 获取字段索引
    let field_index = match table.def.fields
        .iter()
        .position(|field| field.name == &comp.field) {
        Some(index) => index,
        None => return false, // 字段不存在，条件不成立
    };
    
    let field_type = table.def.fields[field_index].data_type;
    
    // 获取字段值
    match get_field_value(table, record_ptr, &comp.field) {
        Ok(field_value) => {
            // 比较字段值和条件值，传入字段类型
            compare_values(&field_value.value, field_type, &comp.operator, &comp.value)
        },
        Err(_) => false,
    }
}

/// 比较两个值 - 修复了类型不匹配的bug
fn compare_values(field_value: &Value, field_type: DataType, operator: &ComparisonOperator, condition_value: &crate::sql::Value) -> bool {
    // 根据字段类型从Value union中读取正确的字段值，然后与条件值进行比较
    match field_type {
        // 无符号整数类型
        DataType::UInt8 => {
            let f_val = unsafe { field_value.u8 }; // 读取u8字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u8;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::UInt16 => {
            let f_val = unsafe { field_value.u16 }; // 读取u16字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u16;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::UInt32 => {
            let f_val = unsafe { field_value.u32 }; // 读取u32字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u32;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::UInt64 => {
            let f_val = unsafe { field_value.u64 }; // 读取u64字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        
        // 有符号整数类型
        DataType::Int8 => {
            let f_val = unsafe { field_value.i8 }; // 读取i8字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i8;
                    // 调试输出
                    println!("Int8 comparison: field_value={}, condition_value={}, operator={:?}", f_val, c_val, operator);
                    let result = compare_numbers(f_val, c_val, operator);
                    println!("Comparison result: {}", result);
                    result
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::Int16 => {
            let f_val = unsafe { field_value.i16 }; // 读取i16字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i16;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::Int32 => {
            let f_val = unsafe { field_value.i32 }; // 读取i32字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i32;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::Int64 => {
            let f_val = unsafe { field_value.i64 }; // 读取i64字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        
        // 浮点数类型
        DataType::Float32 => {
            let f_val = unsafe { field_value.float32 }; // 读取float32字段
            match condition_value {
                crate::sql::Value::Float(c_float) => {
                    compare_numbers(f_val as f64, *c_float, operator)
                },
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val as f64, *c_int as f64, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::Float64 => {
            let f_val = unsafe { field_value.float64 }; // 读取float64字段
            match condition_value {
                crate::sql::Value::Float(c_float) => {
                    compare_numbers(f_val, *c_float, operator)
                },
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val, *c_int as f64, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        
        // 布尔类型
        DataType::Bool => {
            let f_val = unsafe { field_value.bool }; // 读取bool字段
            match condition_value {
                crate::sql::Value::Boolean(c_bool) => {
                    compare_booleans(f_val, *c_bool, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        
        // 时间戳类型
        DataType::Timestamp => {
            let f_val = unsafe { field_value.time.value } as u64; // 读取时间值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        DataType::TimestampTZ => {
            let f_val = unsafe { field_value.time.value } as u64; // 读取时间值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        
        // 字符串类型
        DataType::String => {
            let f_str = unsafe { &field_value.string }; // 读取string字段
            let f_str = String::from_utf8_lossy(f_str).trim_end_matches(char::from(0)).to_string();
            match condition_value {
                crate::sql::Value::String(c_str) => {
                    compare_strings(&f_str, c_str, operator)
                },
                _ => false, // 类型不匹配
            }
        },
        // 时间间隔类型
        DataType::Interval => {
            let f_val = unsafe { field_value.interval.value } as u64; // 读取时间间隔值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                },
                _ => false, // 类型不匹配
            }
        },
    }
}

/// 比较数字
fn compare_numbers<T: PartialOrd>(f: T, c: T, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        ComparisonOperator::GreaterThan => f > c,
        ComparisonOperator::GreaterThanOrEqual => f >= c,
        ComparisonOperator::LessThan => f < c,
        ComparisonOperator::LessThanOrEqual => f <= c,
        _ => false,
    }
}

/// 比较布尔值
fn compare_booleans(f: bool, c: bool, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        _ => false,
    }
}

/// 比较字符串
fn compare_strings(f: &str, c: &str, operator: &ComparisonOperator) -> bool {
    match operator {
        ComparisonOperator::Equal => f == c,
        ComparisonOperator::NotEqual => f != c,
        ComparisonOperator::GreaterThan => f > c,
        ComparisonOperator::GreaterThanOrEqual => f >= c,
        ComparisonOperator::LessThan => f < c,
        ComparisonOperator::LessThanOrEqual => f <= c,
        _ => false,
    }
}

/// 处理AT TIME ZONE操作符
/// 将timestamp转换为指定时区的timestamp
fn process_at_time_zone(timestamp: &crate::types::db_timestamp, timezone_spec: &str) -> Result<crate::types::db_timestamp, QueryExecutionError> {
    // 解析时区规范
    let tz_offset = if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0].parse::<i32>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1].parse::<i32>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            ((hours * 3600) + (minutes * 60)) as i16
        } else {
            return Err(QueryExecutionError::TypeMismatch);
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec)
            .ok_or(QueryExecutionError::TypeMismatch)?
    };
    
    // 转换时间戳到指定时区
    Ok(crate::types::convert_timezone(timestamp, tz_offset))
}

/// 处理TIMEZONE()函数
/// 获取指定时区的偏移量
fn process_timezone_function(timezone_spec: &str) -> Result<i16, QueryExecutionError> {
    // 解析时区规范
    if timezone_spec.starts_with('+') || timezone_spec.starts_with('-') {
        // 处理时区偏移格式，如 '+08:00' 或 '-05:30'
        let parts: Vec<&str> = timezone_spec.split(':').collect();
        if parts.len() == 2 {
            let hours = parts[0].parse::<i32>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            let minutes = parts[1].parse::<i32>().map_err(|_| QueryExecutionError::TypeMismatch)?;
            Ok(((hours * 3600) + (minutes * 60)) as i16)
        } else {
            Err(QueryExecutionError::TypeMismatch)
        }
    } else {
        // 处理时区名称格式，如 'UTC', 'Asia/Shanghai'
        crate::types::get_timezone_offset(timezone_spec)
            .ok_or(QueryExecutionError::TypeMismatch)
    }
}

/// 处理TO_CHAR()函数
/// 将时间戳转换为指定格式的字符串
fn process_to_char(timestamp: &crate::types::db_timestamp, format: &str) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_char(timestamp, format))
}

/// 处理TO_ISO8601()函数
/// 将时间戳转换为ISO 8601格式的字符串
fn process_to_iso8601(timestamp: &crate::types::db_timestamp) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_iso8601(timestamp))
}

/// 处理TO_EPOCH()函数
/// 将时间戳转换为epoch秒数
fn process_to_epoch(timestamp: &crate::types::db_timestamp) -> Result<f64, QueryExecutionError> {
    Ok(crate::types::time_format::to_epoch(timestamp))
}

/// 对行进行排序
fn sort_rows(rows: &mut Vec<Vec<TypedValue>>, table: &MemoryTable, order_by: &OrderByClause) -> Result<(), QueryExecutionError> {
    // 查找排序字段在表中的索引
    let field_index = table.def.fields
        .iter()
        .position(|field| field.name == order_by.field)
        .ok_or(QueryExecutionError::FieldNotFound)?;
    
    let field_type = table.def.fields[field_index].data_type;
    
    // 对行进行排序
    rows.sort_by(|a, b| {
        // 查找排序字段在返回列中的索引
        // 遍历表的所有字段，找到在返回列中对应的索引
        let mut sort_col_index = 0;
        for (i, field) in table.def.fields.iter().enumerate() {
            if field.name == order_by.field {
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
            },
            DataType::UInt16 => {
                let a_val = unsafe { val_a.value.u16 };
                let b_val = unsafe { val_b.value.u16 };
                a_val.cmp(&b_val)
            },
            DataType::UInt32 => {
                let a_val = unsafe { val_a.value.u32 };
                let b_val = unsafe { val_b.value.u32 };
                a_val.cmp(&b_val)
            },
            DataType::UInt64 => {
                let a_val = unsafe { val_a.value.u64 };
                let b_val = unsafe { val_b.value.u64 };
                a_val.cmp(&b_val)
            },
            
            // 有符号整数类型
            DataType::Int8 => {
                let a_val = unsafe { val_a.value.i8 };
                let b_val = unsafe { val_b.value.i8 };
                a_val.cmp(&b_val)
            },
            DataType::Int16 => {
                let a_val = unsafe { val_a.value.i16 };
                let b_val = unsafe { val_b.value.i16 };
                a_val.cmp(&b_val)
            },
            DataType::Int32 => {
                let a_val = unsafe { val_a.value.i32 };
                let b_val = unsafe { val_b.value.i32 };
                a_val.cmp(&b_val)
            },
            DataType::Int64 => {
                let a_val = unsafe { val_a.value.i64 };
                let b_val = unsafe { val_b.value.i64 };
                a_val.cmp(&b_val)
            },
            
            // 浮点数类型
            DataType::Float32 => {
                let a_val = unsafe { val_a.value.float32 };
                let b_val = unsafe { val_b.value.float32 };
                a_val.partial_cmp(&b_val).unwrap_or(core::cmp::Ordering::Equal)
            },
            DataType::Float64 => {
                let a_val = unsafe { val_a.value.float64 };
                let b_val = unsafe { val_b.value.float64 };
                a_val.partial_cmp(&b_val).unwrap_or(core::cmp::Ordering::Equal)
            },
            
            // 布尔类型
            DataType::Bool => {
                let a_val = unsafe { val_a.value.bool };
                let b_val = unsafe { val_b.value.bool };
                a_val.cmp(&b_val)
            },
            
            // 时间戳类型
            DataType::Timestamp => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            },
            DataType::TimestampTZ => {
                let a_val = unsafe { val_a.value.time.value };
                let b_val = unsafe { val_b.value.time.value };
                a_val.cmp(&b_val)
            },
            
            // 字符串类型
            DataType::String => {
                let a_str = unsafe { &val_a.value.string };
                let b_str = unsafe { &val_b.value.string };
                
                let a_str = String::from_utf8_lossy(a_str).trim_end_matches(char::from(0)).to_string();
                let b_str = String::from_utf8_lossy(b_str).trim_end_matches(char::from(0)).to_string();
                
                a_str.cmp(&b_str)
            },
            // 时间间隔类型
            DataType::Interval => {
                let a_val = unsafe { val_a.value.interval.value };
                let b_val = unsafe { val_b.value.interval.value };
                a_val.cmp(&b_val)
            },
        };
        
        // 根据排序方向调整结果
        match order_by.direction {
            crate::sql::OrderDirection::Ascending => comparison,
            crate::sql::OrderDirection::Descending => comparison.reverse(),
        }
    });
    
    Ok(())
}

/// 获取字段值
unsafe fn get_field_value(table: &MemoryTable, record_ptr: *const u8, field_name: &str) -> Result<TypedValue, QueryExecutionError> {
    // 查找字段索引
    let field_index = table.def.fields
        .iter()
        .position(|field| field.name == field_name)
        .ok_or(QueryExecutionError::FieldNotFound)?;
    
    let field = &table.def.fields[field_index];
    // 获取字段值
    let value = table.get_field(record_ptr, field_index)
        .map_err(|_| QueryExecutionError::FieldNotFound)?;
    
    Ok(TypedValue {
        value_type: field.data_type,
        value,
    })
}
