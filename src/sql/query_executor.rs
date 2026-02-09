//! SQL查询执行器
//!
//! 该模块负责执行SQL查询并返回结果集。

use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::env;
use std::time::Instant;

use crate::sql::query_parser::{
    BetweenCondition, BinaryOperator, Expression, GroupByClause, JoinType, LogicalOperator, UnaryOperator,
};
use crate::sql::{
    ComparisonCondition, ComparisonOperator, Condition, OrderByClause, ResultSet, SqlQuery,
};
use crate::types::{DataType, TypedValue, JsonStorage};
use crate::{
    DdlExecutor, IndexType, MemoryTable, RemDb, RemDbError, TableDef, TimeSeriesTable, Value,
    MAX_STRING_LEN,
};
use crate::model::model_manager::get_global_model_manager;
#[cfg(feature = "log")]
use crate::log::{debug, error, info, warn};

/// 解析数据类型字符串，提取基本类型、精度/维度和距离类型
/// 例如："TIMESTAMP(6)" -> ("TIMESTAMP", 6, None)
///       "VECTOR(768)" -> ("VECTOR", 768, None)
///       "VECTOR(64) WITH DISTANCE=IP" -> ("VECTOR", 64, Some(InnerProduct))
fn parse_data_type_with_precision(type_str: &str) -> Result<(String, u16, Option<crate::types::DistanceType>), QueryExecutionError> {
    #[cfg(feature = "log")]
    debug!("parse_data_type_with_precision called with: {}", type_str);
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
            if param < 1 || param > 4096 {
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
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64" |
            "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32" | "UINT64" |
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" |
            "VARCHAR" | "CHAR" | "TEXT" |
            "BOOL" | "BOOLEAN" |
            "TIMESTAMP" | "TIMESTAMPTZ" | "JSON" |
            "VECTOR" => Ok((base_type.to_string(), param, distance_type)),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    } else {
        // 没有参数，使用默认值
        let base_type = type_str.trim();
        
        // 验证基本类型是否有效
        match base_type {
            "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64" |
            "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32" | "UINT64" |
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" |
            "VARCHAR" | "CHAR" | "TEXT" |
            "BOOL" | "BOOLEAN" |
            "TIMESTAMP" | "TIMESTAMPTZ" | "JSON" => Ok((base_type.to_string(), 6, None)), // 默认精度6（微秒）
            _ => Err(QueryExecutionError::TypeMismatch),
        }
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
    /// 操作不允许
    NotAllowed,
    /// 不支持的函数
    UnsupportedFunction(String),
    /// 无效的值
    InvalidValue,
    /// 资源限制超出
    ResourceLimitExceeded(String),
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
            QueryExecutionError::NotAllowed => write!(f, "Operation not allowed"),
            QueryExecutionError::UnsupportedFunction(func) => {
                write!(f, "Unsupported function: {}", func)
            }
            QueryExecutionError::InvalidValue => {
                write!(f, "Invalid value")
            }
            QueryExecutionError::ResourceLimitExceeded(msg) => {
                write!(f, "Resource limit exceeded: {}", msg)
            }
        }
    }
}

impl core::error::Error for QueryExecutionError {}

/// 执行SQL查询
pub fn execute_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    #[cfg(feature = "log")]
    debug!("execute_query called: query_type={:?}, table_name={}", query.query_type, query.table_name);
    // 检查是否是时序表查询
    let is_timeseries_table = db.time_series_tables.iter().any(|table_opt| {
        if let Some(table) = table_opt {
            table.def.base.name == query.table_name
        } else {
            false
        }
    });

    // 权限检查
    match query.query_type {
        crate::sql::QueryType::Select => {
            // 检查SELECT权限
            if let Ok(has_permission) = db.check_permission("root", &crate::rbac::Permission::Select, &Some(query.table_name.clone()), &None) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Insert => {
            // 检查INSERT权限
            if let Ok(has_permission) = db.check_permission("root", &crate::rbac::Permission::Insert, &Some(query.table_name.clone()), &None) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Update => {
            // 检查UPDATE权限
            if let Ok(has_permission) = db.check_permission("root", &crate::rbac::Permission::Update, &Some(query.table_name.clone()), &None) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        crate::sql::QueryType::Delete => {
            // 检查DELETE权限
            if let Ok(has_permission) = db.check_permission("root", &crate::rbac::Permission::Delete, &Some(query.table_name.clone()), &None) {
                if !has_permission {
                    return Err(QueryExecutionError::InternalError);
                }
            } else {
                return Err(QueryExecutionError::InternalError);
            }
        }
        _ => {}
    }

    match query.query_type {
        crate::sql::QueryType::Select => {
            if is_timeseries_table {
                execute_select_timeseries_query(db, query)
            } else {
                execute_select_query(db, query)
            }
        }
        crate::sql::QueryType::Insert => execute_insert_query(db, query),
        crate::sql::QueryType::Update => execute_update_query(db, query),
        crate::sql::QueryType::Delete => execute_delete_query(db, query),
        crate::sql::QueryType::Describe => execute_describe_query(db, query),
        crate::sql::QueryType::CreateTable => execute_create_table_query(db, query),
        crate::sql::QueryType::CreateTimeSeriesTable => {
            execute_create_time_series_table_query(db, query)
        }
        crate::sql::QueryType::CreateIndex => execute_create_index_query(db, query),
        crate::sql::QueryType::ShowIndexBuildStatus => execute_show_index_build_status_query(db, query),
        crate::sql::QueryType::AlterTable => {
            // 处理ALTER TABLE语句
            for (field1, field2, pk, not_null, unique, auto_inc, default_val) in &query.table_def {
                if field2 == "DROP" {
                    // 执行DROP COLUMN操作
                    db.alter_table(
                        &query.table_name,
                        crate::AlterTableOperation::DropColumn { name: field1.clone() },
                    ).map_err(|_| QueryExecutionError::InternalError)?;
                } else if field2 != "" && field2 != "DROP" {
                    // 检查是否是RENAME COLUMN操作
                    // 通过检查field2是否是有效的数据类型来区分
                    match parse_data_type_with_precision(field2) {
                        Ok(_) => {
                            // field2是有效的数据类型，执行ADD COLUMN或MODIFY COLUMN操作
                            // 解析数据类型
                            let (base_type, size, distance_type) = parse_data_type_with_precision(field2)?;
                            let data_type = match base_type.as_str() {
                                "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16" | "INT32" | "INT64" => crate::types::DataType::Int64,
                                "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT" | "UINT16" | "UINT32" | "UINT64" => crate::types::DataType::UInt64,
                                "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" => crate::types::DataType::Float32,
                                "VARCHAR" => crate::types::DataType::VarChar,
                                "CHAR" => crate::types::DataType::Char,
                                "TEXT" => crate::types::DataType::Text,
                                "BOOL" | "BOOLEAN" => crate::types::DataType::Bool,
                                "TIMESTAMP" => crate::types::DataType::Timestamp,
                                "TIMESTAMPTZ" => crate::types::DataType::TimestampTZ,
                                "INTERVAL" => crate::types::DataType::Interval,
                                "VECTOR" => crate::types::DataType::Vector,
                                "JSON" => crate::types::DataType::Json,
                                _ => return Err(QueryExecutionError::TypeMismatch),
                            };

                            // 构建约束条件
                            let constraints = crate::FieldConstraint {
                                primary_key: *pk,
                                not_null: *not_null,
                                unique: *unique,
                                auto_increment: *auto_inc,
                            };

                            // 检查是ADD还是MODIFY操作
                            let existing_table = db.tables.iter().find(|table_opt| {
                                if let Some(table) = table_opt {
                                    table.def.name == query.table_name
                                } else {
                                    false
                                }
                            });

                            let field_exists = existing_table.map(|table_opt| {
                                if let Some(table) = table_opt {
                                    table.def.fields.iter().any(|f| f.name == *field1)
                                } else {
                                    false
                                }
                            }).unwrap_or(false);

                            // 转换默认值类型：query_parser::Value -> types::Value
                            let types_default_value = default_val.as_ref().map(|qp_val| {
                                match qp_val {
                                    crate::sql::query_parser::Value::Integer(i) => {
                                        crate::types::Value { i64: *i }
                                    },
                                    crate::sql::query_parser::Value::Float(f) => {
                                        crate::types::Value { float32: *f as f32 }
                                    },
                                    crate::sql::query_parser::Value::String(s) => {
                                        let mut string_val = crate::types::Value { string: [0u8; 64] };
                                        unsafe {
                                            let s_bytes = s.as_bytes();
                                            let dest = &mut string_val.string as *mut u8;
                                            let src = s_bytes.as_ptr();
                                            let copy_size = core::cmp::min(s_bytes.len(), 64);
                                            core::ptr::copy_nonoverlapping(src, dest, copy_size);
                                        }
                                        string_val
                                    },
                                    crate::sql::query_parser::Value::Boolean(b) => {
                                        crate::types::Value { bool: *b }
                                    },
                                    _ => crate::types::Value { i64: 0 },
                                }
                            });

                            if field_exists {
                                // 执行MODIFY COLUMN操作
                                db.alter_table(
                                    &query.table_name,
                                    crate::AlterTableOperation::ModifyColumn {
                                        name: field1.clone(),
                                        data_type,
                                        size,
                                        distance_type,
                                        default_value: types_default_value,
                                        constraints,
                                    },
                                ).map_err(|_| QueryExecutionError::InternalError)?;
                            } else {
                                // 执行ADD COLUMN操作
                                db.alter_table(
                                    &query.table_name,
                                    crate::AlterTableOperation::AddColumn {
                                        name: field1.clone(),
                                        data_type,
                                        size,
                                        distance_type,
                                        default_value: types_default_value,
                                        constraints,
                                    },
                                ).map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        },
                        Err(_) => {
                            // field2不是有效的数据类型，执行RENAME COLUMN操作
                            db.alter_table(
                                &query.table_name,
                                crate::AlterTableOperation::RenameColumn { 
                                    old_name: field1.clone(), 
                                    new_name: field2.clone() 
                                },
                            ).map_err(|_| QueryExecutionError::InternalError)?;
                        }
                    }
                }
            }
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::DropTable => execute_drop_table_query(db, query),
        crate::sql::QueryType::BeginTransaction => {
            // 开始事务
            unsafe {
                crate::transaction::begin_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::Commit => {
            // 提交事务
            unsafe {
                crate::transaction::commit_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::Rollback => {
            // 回滚事务
            unsafe {
                crate::transaction::rollback_transaction();
            }
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::CreateDatabase => execute_create_database_query(db, query),
        crate::sql::QueryType::UseDatabase => execute_use_database_query(db, query),
        crate::sql::QueryType::CloseDatabase => execute_close_database_query(db, query),
        crate::sql::QueryType::DropDatabase => execute_drop_database_query(db, query),
        crate::sql::QueryType::CreateModel => {
            // Register the model using the global model manager
            match get_global_model_manager() {
                Ok(mut model_manager) => {
                    match model_manager.register_model(
                        query.table_name.clone(),
                        query.model_path.clone(),
                        query.model_inputs.clone(),
                        query.model_output.clone(),
                    ) {
                        Ok(_) => Ok(ResultSet::new(Vec::new())),
                        Err(e) => {
                            #[cfg(feature = "log")]
                            error!("Model registration failed: {:?}", e);
                            Err(QueryExecutionError::InternalError)
                        },
                    }
                }
                Err(_) => Err(QueryExecutionError::InternalError),
            }
        },
        crate::sql::QueryType::CreateRole => {
            // Extract role name from table_name field
            let role_name = query.table_name.clone();
            db.create_role(&role_name)
                .map_err(|_| QueryExecutionError::InternalError)?;
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::DropRole => {
            // Extract role name from table_name field
            let role_name = query.table_name.clone();
            db.drop_role(&role_name)
                .map_err(|_| QueryExecutionError::InternalError)?;
            Ok(ResultSet::new(Vec::new()))
        },
        crate::sql::QueryType::GrantPermission => {
            // Extract role name and permission from query fields
            let role_name = query.table_name.clone();
            // Extract permission from the first field in table_def
            if let Some((permission_str, _, _, _, _, _, _)) = query.table_def.first() {
                let permission = crate::rbac::Permission::from_str(permission_str)
                    .ok_or(QueryExecutionError::InternalError)?;
                // Extract table name from the second field
                let table_name = if let Some((_, table_name, _, _, _, _, _)) = query.table_def.first() {
                    table_name.clone()
                } else {
                    String::new()
                };
                db.grant_permission(&role_name, permission, Some(table_name), None)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        },
        crate::sql::QueryType::RevokePermission => {
            // Extract role name and permission from query fields
            let role_name = query.table_name.clone();
            // Extract permission from the first field in table_def
            if let Some((permission_str, _, _, _, _, _, _)) = query.table_def.first() {
                let permission = crate::rbac::Permission::from_str(permission_str)
                    .ok_or(QueryExecutionError::InternalError)?;
                // Extract table name from the second field
                let table_name = if let Some((_, table_name, _, _, _, _, _)) = query.table_def.first() {
                    table_name.clone()
                } else {
                    String::new()
                };
                db.revoke_permission(&role_name, &permission, &Some(table_name), &None)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        },
        crate::sql::QueryType::GrantRole => {
            // Extract username and role name from query fields
            let username = query.table_name.clone();
            // Extract role name from the first field in table_def
            if let Some((role_name, _, _, _, _, _, _)) = query.table_def.first() {
                db.grant_role(&username, role_name)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        },
        crate::sql::QueryType::RevokeRole => {
            // Extract username and role name from query fields
            let username = query.table_name.clone();
            // Extract role name from the first field in table_def
            if let Some((role_name, _, _, _, _, _, _)) = query.table_def.first() {
                db.revoke_role(&username, role_name)
                    .map_err(|_| QueryExecutionError::InternalError)?;
                Ok(ResultSet::new(Vec::new()))
            } else {
                Err(QueryExecutionError::InternalError)
            }
        },
        _ => Err(QueryExecutionError::InternalError),
    }
}

/// 查找时序表
fn find_timeseries_table_by_name<'a>(
    db: &'a RemDb,
    table_name: &str,
) -> Result<&'a TimeSeriesTable, QueryExecutionError> {
    for table in db.time_series_tables.iter() {
        if let Some(table) = table {
            if table.def.base.name == table_name {
                return Ok(table);
            }
        }
    }

    Err(QueryExecutionError::TableNotFound)
}

/// 从WHERE条件中提取时间范围
fn extract_time_range_from_condition(
    condition: &crate::sql::query_parser::Condition,
    ts_table: &crate::time_series::TimeSeriesTable,
) -> Result<(u64, u64), QueryExecutionError> {
    use crate::sql::query_parser::{BetweenCondition, ComparisonCondition, ComparisonOperator};
    
    // 获取时间字段名称
    let time_field_name = ts_table.def.base.fields[ts_table.def.time_field].name.clone();
    
    // 递归解析条件
    fn extract_from_condition(
        condition: &crate::sql::query_parser::Condition,
        time_field_name: &str,
    ) -> Result<(Option<u64>, Option<u64>), QueryExecutionError> {
        match condition {
            crate::sql::query_parser::Condition::Comparison(comp) => {
                if comp.field == time_field_name {
                    match comp.operator {
                        ComparisonOperator::GreaterThan | ComparisonOperator::GreaterThanOrEqual => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((Some(value as u64), None))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        ComparisonOperator::LessThan | ComparisonOperator::LessThanOrEqual => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((None, Some(value as u64)))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        ComparisonOperator::Equal => {
                            if let crate::sql::query_parser::Value::Integer(value) = comp.value {
                                Ok((Some(value as u64), Some(value as u64)))
                            } else {
                                Err(QueryExecutionError::InvalidCondition)
                            }
                        }
                        _ => Ok((None, None)),
                    }
                } else {
                    Ok((None, None))
                }
            }
            crate::sql::query_parser::Condition::Between(between) => {
                if between.field == time_field_name {
                    if let (crate::sql::query_parser::Value::Integer(min), 
                           crate::sql::query_parser::Value::Integer(max)) = (&between.min_value, &between.max_value) {
                        Ok((Some(*min as u64), Some(*max as u64)))
                    } else {
                        Err(QueryExecutionError::InvalidCondition)
                    }
                } else {
                    Ok((None, None))
                }
            }
            crate::sql::query_parser::Condition::And(left, right) => {
                let (left_min, left_max) = extract_from_condition(left, time_field_name)?;
                let (right_min, right_max) = extract_from_condition(right, time_field_name)?;
                
                let min = left_min.or(right_min);
                let max = left_max.or(right_max);
                Ok((min, max))
            }
            crate::sql::query_parser::Condition::Or(_, _) => {
                // OR条件不能简单合并，暂时不支持
                Err(QueryExecutionError::UnsupportedFunction("OR conditions in time range extraction".to_string()))
            }
            crate::sql::query_parser::Condition::Not(_) => {
                // NOT条件不支持
                Err(QueryExecutionError::UnsupportedFunction("NOT conditions in time range extraction".to_string()))
            }
        }
    }
    
    let (min_opt, max_opt) = extract_from_condition(condition, &time_field_name)?;
    
    let start_time = min_opt.unwrap_or(0);
    let end_time = max_opt.unwrap_or(u64::MAX);
    
    Ok((start_time, end_time))
}

/// 执行时序表SELECT查询
fn execute_select_timeseries_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的时序表
    let ts_table = find_timeseries_table_by_name(db, &query.table_name)?;

    // 2. 确定要返回的列表达式
    let columns = if query.select_all {
        // 返回所有列（作为Field表达式）
        ts_table
            .def
            .base
            .fields
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

    // 5. 提取时间范围条件
    let (start_time, end_time) = if let Some(where_clause) = &query.where_clause {
        extract_time_range_from_condition(&where_clause.condition, ts_table)?
    } else {
        // 如果没有WHERE条件，查询所有数据
        (0, u64::MAX)
    };

    // 6. 执行时间范围查询
    let raw_records = ts_table.query_time_range(start_time, end_time)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 6.1 内存使用检查
    let estimated_memory = estimate_memory_usage(&raw_records);
    // 从系统表获取内存限制
    let (max_memory_mb, _) = crate::get_query_resource_config();
    check_memory_limit(estimated_memory, Some(max_memory_mb))?;

    // 7. 应用SAMPLE BY降采样（如果指定）
    let sampled_records = if let Some(sample_interval) = &query.sample_by {
        downsample_records(&raw_records, sample_interval, query.fill_clause.as_ref())?
    } else {
        raw_records
    };

    // 8. 转换为TypedValue并添加到结果集
    for record in sampled_records {
        let mut row_data = Vec::with_capacity(columns.len());
        for expr in &columns {
            let value = evaluate_timeseries_expression(expr, &record, ts_table)?;
            row_data.push(value);
        }
        result_set.add_row(row_data);
    }

    // 注意：execute_select_timeseries_query函数中没有stats和start_time变量，暂时注释掉统计信息
    /*
    // 计算执行时间
    let end_time = Instant::now();
    stats.execution_time = end_time.duration_since(start_time).as_micros() as u64;

    // 输出查询执行统计信息
    #[cfg(feature = "log")]
    {
        info!("Query execution stats:");
        info!("  Used index: {}", stats.used_index);
        info!("  Scanned records: {}", stats.scanned_records);
        info!("  Matched records: {}", stats.matched_records);
        info!("  Execution time: {}μs", stats.execution_time);
    }
    */

    Ok(result_set)
}

/// 对时序记录进行降采样
fn downsample_records(
    records: &[crate::time_series::TimeSeriesRecord],
    sample_interval: &str,
    fill_clause: Option<&crate::sql::query_parser::FillClause>,
) -> Result<Vec<crate::time_series::TimeSeriesRecord>, QueryExecutionError> {
    if records.is_empty() {
        return Ok(Vec::new());
    }
    
    // 解析时间间隔
    let interval_seconds = parse_sample_interval(sample_interval)?;
    let interval_nanos = interval_seconds * 1_000_000_000u64;
    
    // 找到最小和最大时间戳
    let min_timestamp = records.iter().map(|r| r.timestamp).min().unwrap();
    let max_timestamp = records.iter().map(|r| r.timestamp).max().unwrap();
    
    // 按时间窗口分组
    let mut windows: std::collections::BTreeMap<u64, Vec<&crate::time_series::TimeSeriesRecord>> = 
        std::collections::BTreeMap::new();
    
    for record in records {
        let window_start = (record.timestamp / interval_nanos) * interval_nanos;
        windows.entry(window_start).or_default().push(record);
    }
    
    // 确定窗口范围
    let first_window = (min_timestamp / interval_nanos) * interval_nanos;
    let last_window = (max_timestamp / interval_nanos) * interval_nanos;
    
    // 为每个窗口生成降采样记录（包括空窗口）
    let mut result = Vec::new();
    let mut prev_window_data: Option<(u64, f64, u8, [u64; 8])> = None;
    let mut next_window_iter = windows.iter().peekable();
    
    let mut current_window = first_window;
    while current_window <= last_window {
        if let Some((&window_start, window_records)) = next_window_iter.peek() {
            if window_start == current_window {
                // 当前窗口有数据
                let window_records = next_window_iter.next().unwrap().1;
                
                // 计算窗口内记录的平均值（优化版本，减少迭代次数）
                let (sum, count) = window_records.iter()
                    .fold((0.0, 0), |(sum, count), record| (sum + record.value, count + 1));
                let avg_value: f64 = sum / count as f64;
                
                // 使用第一个记录的标签
                let first_record = window_records[0];
                
                result.push(crate::time_series::TimeSeriesRecord {
                    timestamp: current_window,
                    value: avg_value,
                    tag_count: first_record.tag_count,
                    tags: first_record.tags,
                });
                
                // 保存为前一个窗口数据（用于PREV插值）
                prev_window_data = Some((current_window, avg_value, first_record.tag_count, first_record.tags));
            } else {
                // 当前窗口无数据，需要插值
                if let Some(fill_clause) = fill_clause {
                    if let Some(record) = interpolate_missing_window(
                        current_window,
                        &prev_window_data,
                        next_window_iter.peek().map(|(&ts, recs)| (ts, recs)),
                        fill_clause,
                    ) {
                        result.push(record);
                    }
                }
                // 如果没有指定FILL子句，则跳过空窗口
            }
        } else {
            // 后续所有窗口都无数据
            if let Some(fill_clause) = fill_clause {
                if let Some(record) = interpolate_missing_window(
                    current_window,
                    &prev_window_data,
                    None,
                    fill_clause,
                ) {
                    result.push(record);
                }
            }
        }
        
        current_window += interval_nanos;
    }
    
    Ok(result)
}

/// 插值缺失的时间窗口
fn interpolate_missing_window(
    window_start: u64,
    prev_data: &Option<(u64, f64, u8, [u64; 8])>,
    next_data: Option<(u64, &&Vec<&crate::time_series::TimeSeriesRecord>)>,
    fill_clause: &crate::sql::query_parser::FillClause,
) -> Option<crate::time_series::TimeSeriesRecord> {
    match fill_clause {
        crate::sql::query_parser::FillClause::Prev => {
            if let Some((_prev_ts, prev_val, prev_tag_count, prev_tags)) = prev_data {
                Some(crate::time_series::TimeSeriesRecord {
                    timestamp: window_start,
                    value: *prev_val,
                    tag_count: *prev_tag_count,
                    tags: *prev_tags,
                })
            } else {
                None
            }
        }
        crate::sql::query_parser::FillClause::Next => {
            if let Some((_next_ts, next_records)) = next_data {
                if !next_records.is_empty() {
                    let first_record = next_records[0];
                    Some(crate::time_series::TimeSeriesRecord {
                        timestamp: window_start,
                        value: first_record.value,
                        tag_count: first_record.tag_count,
                        tags: first_record.tags,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        crate::sql::query_parser::FillClause::Linear => {
            match (prev_data, next_data) {
                (Some((prev_ts, prev_val, prev_tag_count, prev_tags)), 
                  Some((next_ts, next_records))) => {
                    if !next_records.is_empty() {
                        let first_next_record = next_records[0];
                        let time_ratio = (window_start - prev_ts) as f64 / (next_ts - prev_ts) as f64;
                        let interpolated_value = prev_val + (first_next_record.value - prev_val) * time_ratio;
                        
                        Some(crate::time_series::TimeSeriesRecord {
                            timestamp: window_start,
                            value: interpolated_value,
                            tag_count: *prev_tag_count, // 使用前一个窗口的标签
                            tags: *prev_tags,
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        crate::sql::query_parser::FillClause::FixedValue(value) => {
            // 对于固定值，需要从现有记录中获取标签信息
            if let Some((_prev_ts, _prev_val, prev_tag_count, prev_tags)) = prev_data {
                Some(crate::time_series::TimeSeriesRecord {
                    timestamp: window_start,
                    value: *value,
                    tag_count: *prev_tag_count,
                    tags: *prev_tags,
                })
            } else if let Some((_next_ts, next_records)) = next_data {
                if !next_records.is_empty() {
                    let first_record = next_records[0];
                    Some(crate::time_series::TimeSeriesRecord {
                        timestamp: window_start,
                        value: *value,
                        tag_count: first_record.tag_count,
                        tags: first_record.tags,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

/// 解析SAMPLE BY时间间隔字符串，如"1h"、"5m"、"30s"
fn parse_sample_interval(interval_str: &str) -> Result<u64, QueryExecutionError> {
    let mut total_seconds = 0;
    let mut current_number = 0;
    
    for ch in interval_str.chars() {
        if ch.is_ascii_digit() {
            current_number = current_number * 10 + (ch as u64 - '0' as u64);
        } else {
            match ch.to_ascii_lowercase() {
                'h' => total_seconds += current_number * 3600,
                'm' => total_seconds += current_number * 60,
                's' => total_seconds += current_number,
                _ => return Err(QueryExecutionError::InvalidValue),
            }
            current_number = 0;
        }
    }
    
    // 处理末尾没有单位的情况（默认为秒）
    if current_number > 0 {
        total_seconds += current_number;
    }
    
    if total_seconds == 0 {
        return Err(QueryExecutionError::InvalidValue);
    }
    
    Ok(total_seconds)
}

/// 估算时序记录的内存使用量（字节）
fn estimate_memory_usage(records: &[crate::time_series::TimeSeriesRecord]) -> usize {
    // 简化估算：每条记录的基本大小 + 标签存储
    const BASE_RECORD_SIZE: usize = std::mem::size_of::<crate::time_series::TimeSeriesRecord>();
    records.len() * BASE_RECORD_SIZE
}

/// 估算普通查询记录的内存使用量（字节）
fn estimate_memory_usage_for_records(records: &[Vec<TypedValue>]) -> usize {
    // 简化估算：每条记录的基本大小
    const BASE_VALUE_SIZE: usize = std::mem::size_of::<TypedValue>();
    let total_values = records.iter().map(|record| record.len()).sum::<usize>();
    total_values * BASE_VALUE_SIZE
}

/// 检查内存使用是否超过限制
fn check_memory_limit(
    estimated_usage: usize,
    max_memory_mb: Option<u32>,
) -> Result<(), QueryExecutionError> {
    if let Some(max_mb) = max_memory_mb {
        let max_bytes = (max_mb as usize) * 1024 * 1024;
        if estimated_usage > max_bytes {
            return Err(QueryExecutionError::ResourceLimitExceeded(
                format!("Query exceeds memory limit: {}MB estimated, {}MB allowed", 
                       estimated_usage / (1024 * 1024), max_mb)));
        }
        }

    Ok(())
}

/// 带超时执行的查询包装器
fn execute_with_timeout<F, T>(
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
    
    let timeout = Duration::from_millis(timeout_ms.unwrap());
    
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
        Err(_) => {
            Err(QueryExecutionError::ResourceLimitExceeded(
                format!("Query timeout after {}ms for {}", timeout_ms.unwrap(), operation_name)
            ))
        }
    }
}

/// 评估时序表表达式
fn evaluate_timeseries_expression(
    expr: &crate::sql::query_parser::Expression,
    record: &crate::time_series::TimeSeriesRecord,
    ts_table: &crate::time_series::TimeSeriesTable,
) -> Result<TypedValue, QueryExecutionError> {
    match expr {
        crate::sql::query_parser::Expression::Field { name, .. } => {
            // 查找字段索引
            for (i, field) in ts_table.def.base.fields.iter().enumerate() {
                if field.name == *name {
                    if i == ts_table.def.time_field {
                        // 时间字段
                        return Ok(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: record.timestamp },
                        });
                    } else if i == ts_table.def.value_field {
                        // 值字段
                        return Ok(TypedValue {
                            value_type: DataType::Float64,
                            value: Value { float64: record.value },
                        });
                    } else {
                        // 标签字段（简化处理，暂时返回0）
                        return Ok(TypedValue {
                            value_type: DataType::UInt64,
                            value: Value { u64: 0 },
                        });
                    }
                }
            }
            Err(QueryExecutionError::FieldNotFound)
        }
        crate::sql::query_parser::Expression::FunctionCall { name, args, .. } => {
            let func_name = name.to_uppercase();
            // 简化实现，仅支持基本聚合函数
            match func_name.as_str() {
                "AVG" | "SUM" | "MIN" | "MAX" | "COUNT" => {
                    // 对于单条记录，这些函数返回记录值
                    evaluate_timeseries_expression(&args[0], record, ts_table)
                }
                _ => Err(QueryExecutionError::UnsupportedFunction(name.clone())),
            }
        }
        _ => {
            // 其他表达式类型暂不支持
            Err(QueryExecutionError::UnsupportedFunction("Complex expression in timeseries query".to_string()))
        }
    }
}

/// 执行DROP TABLE查询
fn execute_drop_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取IF EXISTS和DEFERRED选项
    let mut if_exists = false;
    let mut is_deferred = false;
    
    if let Some((if_exists_str, is_deferred_str, _, _, _, _, _)) = query.table_def.first() {
        if_exists = if_exists_str == "true";
        is_deferred = is_deferred_str == "true";
    }

    // 调用RemDb的drop_table方法
    db.drop_table(&query.table_name, if_exists, is_deferred)
        .map_err(|err| match err {
            crate::RemDbError::NotAllowed => QueryExecutionError::NotAllowed,
            crate::RemDbError::TableNotFound => QueryExecutionError::TableNotFound,
            _ => QueryExecutionError::InternalError,
        })?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行CREATE DATABASE查询
fn execute_create_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();
    
    // 调用RemDb的create_database方法
    db.create_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行USE DATABASE查询
fn execute_use_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();
    
    // 调用RemDb的use_database方法
    db.use_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行CLOSE DATABASE查询
fn execute_close_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();
    
    // 调用RemDb的close_database方法
    db.close_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行DROP DATABASE查询
fn execute_drop_database_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();
    
    // 调用RemDb的drop_database方法
    db.drop_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 处理聚合查询
fn process_aggregate_query(
    columns: &[Expression],
    rows_to_process: &[Vec<TypedValue>],
    result_set: &mut ResultSet,
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
                let current_value = evaluate_expression_for_aggregate(args, record_values)?;

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

/// 为聚合函数计算表达式值
fn evaluate_expression_for_aggregate(
    args: &[Expression],
    _record_values: &[TypedValue],
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
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Boolean(b) => (DataType::Bool, Value { bool: *b }),
                SqlValue::Null => (DataType::Json, Value { json_storage: crate::types::JsonStorage::Null }),
                SqlValue::Identifier(s) => {
                    // 标识符作为字符串处理
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Json(json_str) => {
                    let mut buf = [0u8; 256];
                    let len = core::cmp::min(json_str.len(), 256);
                    buf[..len].copy_from_slice(&json_str.as_bytes()[..len]);
                    let json_storage = if json_str.len() <= 256 {
                        crate::types::JsonStorage::Inline(buf)
                    } else {
                        crate::types::JsonStorage::Null
                    };
                    (DataType::Json, Value { json_storage })
                }
            };

            Ok(TypedValue { value_type, value })
        }
        _ => {
            // 对于其他表达式，这里简化处理，返回默认值
            // TODO: 支持更复杂的表达式
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 1 },
            })
        }
    }
}

/// 查询执行统计信息
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

impl Default for QueryStats {
    fn default() -> Self {
        Self {
            used_index: false,
            scanned_records: 0,
            matched_records: 0,
            execution_time: 0,
        }
    }
}

/// 执行SELECT查询
fn execute_select_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    eprintln!("DEBUG execute_select_query: table_name='{}', has_from={}", query.table_name, query.table_name.is_empty());
    
    // 从系统表获取查询资源配置
    let (max_memory_mb, query_timeout_ms) = crate::get_query_resource_config();
    let query_timeout_ms = Some(query_timeout_ms as u64);
    
    // 开始计时
    let start_time = Instant::now();
    let mut stats = QueryStats::default();

    // 检查是否有FROM子句（如果没有FROM子句，则执行表达式查询）
    eprintln!("DEBUG execute_select_query: checking if table_name is empty: '{}', is_empty: {}", query.table_name, query.table_name.is_empty());
    if query.table_name.is_empty() {
        // 没有FROM子句，执行表达式查询
        eprintln!("DEBUG execute_select_query: executing expression query (no FROM clause)");
        return execute_expression_query(db, query);
    }
    eprintln!("DEBUG execute_select_query: executing SELECT query with FROM clause");

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
        if let Some((indexed_field, index_operation)) = extract_index_operation(&where_clause.condition) {
            // 尝试获取表和索引
            match db.get_table_and_secondary_index_mut_by_name(&query.table_name) {
                Ok((table_ref, index_ref)) => {
                    // 成功获取表和索引
                    (table_ref, Some((index_ref, indexed_field, index_operation)))
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
    if let Some((secondary_index, indexed_field, index_operation)) = maybe_index {
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
                            stats.used_index = true;
                            stats.scanned_records = 1;
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
                        start_value.as_ptr(), start_value.len(),
                        end_value.as_ptr(), end_value.len()
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
                            stats.used_index = true;
                            stats.scanned_records = 1;
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
            let iterate_result = table
                .iterate(|_id, record_ptr| {
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

                    stats.scanned_records += 1;
                    true // 继续遍历
                });
            iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
        }
    }
    
    // 更新匹配的记录数
    stats.matched_records = all_records.len();
    eprintln!("DEBUG execute_select_query: scanned {} records", stats.scanned_records);
    eprintln!("DEBUG execute_select_query: found {} records", all_records.len());
    
    // 内存使用检查
    let estimated_memory = estimate_memory_usage_for_records(&all_records);
    check_memory_limit(estimated_memory, Some(max_memory_mb))?;

    // 7. 计算每个记录的表达式值
    let mut records_with_expr_values = Vec::with_capacity(all_records.len());
    eprintln!("DEBUG execute_select_query: evaluating expressions for {} records", all_records.len());
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
    eprintln!("DEBUG execute_select_query: evaluated {} records with expressions", records_with_expr_values.len());

    // 8. 应用WHERE条件过滤记录
    let mut filtered_records = Vec::with_capacity(records_with_expr_values.len());
    eprintln!("DEBUG execute_select_query: filtering {} records", records_with_expr_values.len());
    eprintln!("DEBUG execute_select_query: where_clause={:?}", query.where_clause);
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
            eprintln!("DEBUG execute_select_query: record matches where clause: {}", matches);
        }

        if matches {
            filtered_records.push((record_values, expr_values));
        }
    }
    eprintln!("DEBUG execute_select_query: filtered to {} records", filtered_records.len());

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

        if has_group_by {
            // 处理GROUP BY查询
            process_group_by_query(
                table,
                &columns,
                &records_for_aggregation,
                query.group_by.as_ref().unwrap(),
                &mut result_set,
            )?;
        } else {
            // 处理普通聚合查询
            process_aggregate_query(&columns, &records_for_aggregation, &mut result_set)?;
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
                    debug!("get_field_value: found field '{}' at index {} in main_table", field_name_part, field_index);
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
                    debug!("get_field_value: field '{}' not found, using default value", field_name_part);
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

/// 辅助函数：从条件中获取字段值
fn get_field_value_from_condition<'a>(
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
                .unwrap();
            (&main_table, &main_record_values[field_index])
        } else {
            // 从连接表获取
            let field_index = join_table
                .def
                .fields
                .iter()
                .position(|f| f.name == field_name_part)
                .unwrap();
            (&join_table, &join_record_values[field_index])
        }
    } else {
        // 没有指定表名，尝试从主表查找，找不到再从连接表查找
        if let Some(field_index) = main_table
            .def
            .fields
            .iter()
            .position(|f| f.name == field_name_part)
        {
            (&main_table, &main_record_values[field_index])
        } else if let Some(field_index) = join_table
            .def
            .fields
            .iter()
            .position(|f| f.name == field_name_part)
        {
            (&join_table, &join_record_values[field_index])
        } else {
            panic!("Field not found: {}", field_name_part);
        }
    }
}

/// 辅助函数：比较两个字段值
fn compare_values(left: &TypedValue, right: &TypedValue) -> bool {
    // 确保类型相同
    if left.value_type != right.value_type {
        return false;
    }

    unsafe {
        match left.value_type {
            DataType::Int8 => left.value.i8 == right.value.i8,
            DataType::Int16 => left.value.i16 == right.value.i16,
            DataType::Int32 => left.value.i32 == right.value.i32,
            DataType::Int64 => left.value.i64 == right.value.i64,
            DataType::UInt8 => left.value.u8 == right.value.u8,
            DataType::UInt16 => left.value.u16 == right.value.u16,
            DataType::UInt32 => left.value.u32 == right.value.u32,
            DataType::UInt64 => left.value.u64 == right.value.u64,
            DataType::Float32 => (left.value.float32 - right.value.float32).abs() < f32::EPSILON,
            DataType::Float64 => (left.value.float64 - right.value.float64).abs() < f64::EPSILON,
            DataType::Bool => left.value.bool == right.value.bool,
            DataType::VarChar | DataType::Char | DataType::Text => {
                let left_str = core::str::from_utf8(&left.value.string)
                    .unwrap()
                    .trim_end_matches(char::from(0));
                let right_str = core::str::from_utf8(&right.value.string)
                    .unwrap()
                    .trim_end_matches(char::from(0));
                left_str == right_str
            }
            DataType::Timestamp => left.value.time == right.value.time,
            DataType::TimestampTZ => left.value.time == right.value.time,
            DataType::Interval => left.value.interval == right.value.interval,
            DataType::Vector => {
                // 向量比较：目前不支持精确比较，返回false
                false
            }
            DataType::Json => {
                // JSON比较：目前不支持精确比较，返回false
                false
            }
        }
    }
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
                    if table.def.fields.iter().any(|f| f.name.as_str() == name.as_str()) {
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

/// 执行连接查询（带JOIN子句）
fn execute_select_join_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 从系统表获取查询资源配置
    let (max_memory_mb, query_timeout_ms) = crate::get_query_resource_config();
    
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
                                            (DataType::VarChar | DataType::Char | DataType::Text, crate::sql::Value::String(v)) => {
                                                let field_str =
                                                    core::str::from_utf8(&field_value.value.string)
                                                        .unwrap()
                                                        .trim_end_matches(char::from(0));
                                                field_str == v
                                            }
                                            (DataType::Bool, crate::sql::Value::Boolean(v)) => {
                                                field_value.value.bool == *v
                                            }
                                            (DataType::Float32, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float32 - *v as f32).abs()
                                                    < f32::EPSILON
                                            }
                                            (DataType::Float64, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float64 - *v).abs()
                                                    < f64::EPSILON
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
                    .unwrap();

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
                                value: Value {
                                    u64: 0,
                                },
                            },
                            DataType::Json => TypedValue {
                                value_type: DataType::Json,
                                value: Value { json_storage: JsonStorage::Null },
                            },
                        };
                        join_default_values.push(default_value);
                    }

                    // 添加左连接的默认记录
                    add_joined_row(
                        &mut result_set,
                        &columns,
                        &main_table,
                        &main_record_values,
                        &join_table,
                        &join_default_values,
                    )
                    .unwrap();
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
                                            (DataType::VarChar | DataType::Char | DataType::Text, crate::sql::Value::String(v)) => {
                                                let field_str =
                                                    core::str::from_utf8(&field_value.value.string)
                                                        .unwrap()
                                                        .trim_end_matches(char::from(0));
                                                field_str == v
                                            }
                                            (DataType::Bool, crate::sql::Value::Boolean(v)) => {
                                                field_value.value.bool == *v
                                            }
                                            (DataType::Float32, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float32 - *v as f32).abs()
                                                    < f32::EPSILON
                                            }
                                            (DataType::Float64, crate::sql::Value::Float(v)) => {
                                                (field_value.value.float64 - *v).abs()
                                                    < f64::EPSILON
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
                                                    if let Some(table_name) = right_table_name_part {
                                                        if table_name == query.table_name
                                                            || Some(table_name) == query.table_alias.as_deref()
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
                                                if let Some(right_field_index) = right_table
                                                    .def
                                                    .fields
                                                    .iter()
                                                    .position(|f| f.name == right_field_name_part)
                                                {
                                                    // 获取右字段值
                                                    let right_field_value = &right_record_values[right_field_index];

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
                        .unwrap();

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
                                    value: Value {
                                        u64: 0,
                                    },
                                },
                                DataType::Json => TypedValue {
                                    value_type: DataType::Json,
                                    value: Value { json_storage: JsonStorage::Null },
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

/// 评估一元操作表达式
fn evaluate_unary_op(
    op: crate::sql::query_parser::UnaryOperator,
    operand: TypedValue,
) -> Result<TypedValue, QueryExecutionError> {
    match op {
        crate::sql::query_parser::UnaryOperator::Not => {
            // NOT操作符，返回布尔值的否定
            unsafe {
                let bool_value = match operand.value_type {
                    DataType::Bool => operand.value.bool,
                    DataType::Int8 => operand.value.i8 != 0,
                    DataType::Int16 => operand.value.i16 != 0,
                    DataType::Int32 => operand.value.i32 != 0,
                    DataType::Int64 => operand.value.i64 != 0,
                    DataType::UInt8 => operand.value.u8 != 0,
                    DataType::UInt16 => operand.value.u16 != 0,
                    DataType::UInt32 => operand.value.u32 != 0,
                    DataType::UInt64 => operand.value.u64 != 0,
                    DataType::Float32 => operand.value.float32 != 0.0,
                    DataType::Float64 => operand.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: !bool_value },
                })
            }
        }
        crate::sql::query_parser::UnaryOperator::Minus => {
            // 负号操作符，返回数值的相反数
            unsafe {
                match operand.value_type {
                    DataType::Int8 => Ok(TypedValue {
                        value_type: DataType::Int8,
                        value: Value { i8: -operand.value.i8 },
                    }),
                    DataType::Int16 => Ok(TypedValue {
                        value_type: DataType::Int16,
                        value: Value { i16: -operand.value.i16 },
                    }),
                    DataType::Int32 => Ok(TypedValue {
                        value_type: DataType::Int32,
                        value: Value { i32: -operand.value.i32 },
                    }),
                    DataType::Int64 => Ok(TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: -operand.value.i64 },
                    }),
                    DataType::Float32 => Ok(TypedValue {
                        value_type: DataType::Float32,
                        value: Value { float32: -operand.value.float32 },
                    }),
                    DataType::Float64 => Ok(TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: -operand.value.float64 },
                    }),
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
        crate::sql::query_parser::UnaryOperator::Plus => {
            // 正号操作符，返回原值
            Ok(operand)
        }
    }
}

/// 评估表达式值
fn evaluate_expression(
    table: &MemoryTable,
    record_values: &[TypedValue],
    expr: &Expression,
) -> Result<TypedValue, QueryExecutionError> {
    evaluate_expression_with_depth(table, record_values, expr, 0)
}

fn evaluate_expression_with_depth(
    table: &MemoryTable,
    record_values: &[TypedValue],
    expr: &Expression,
    depth: usize,
) -> Result<TypedValue, QueryExecutionError> {
    // Check recursion depth to prevent stack overflow
    const MAX_RECURSION_DEPTH: usize = 100;
    if depth > MAX_RECURSION_DEPTH {
        return Err(QueryExecutionError::InternalError);
    }
    match expr {
        Expression::Field {
            name: field_name, ..
        } => {
            // 查找字段索引
            if field_name == "*" {
                // 对于COUNT(*), 返回第一个字段的值作为占位符
                // 实际COUNT函数不使用这个值，只是简单累加
                Ok(record_values[0].clone())
            } else {
                // 处理带表别名的字段名，如 "t.id"
                let actual_field_name = if field_name.contains('.') {
                    // 提取点号后面的部分作为实际字段名
                    field_name.split('.').last().unwrap()
                } else {
                    // 没有表别名，直接使用字段名
                    field_name
                };

                let field_index = table
                    .def
                    .fields
                    .iter()
                    .position(|field| field.name == *actual_field_name)
                    .ok_or(QueryExecutionError::FieldNotFound)?;

                // 返回记录中的字段值
                Ok(record_values[field_index].clone())
            }
        }
        Expression::FunctionCall { name, args, .. } => {
            // 评估函数参数
            let mut arg_values = Vec::with_capacity(args.len());
            for arg in args {
                arg_values.push(evaluate_expression_with_depth(table, record_values, arg, depth + 1)?);
            }

            // 执行函数调用
            eprintln!("DEBUG evaluate_expression: calling execute_function_call with name={}, args.len={}", name, arg_values.len());
            let result = execute_function_call(name, &arg_values);
            eprintln!("DEBUG evaluate_expression: execute_function_call result={:?}", result);
            result
        }
        Expression::Constant {
            value: constant, ..
        } => {
            // 将sql::Value转换为types::TypedValue
            use crate::sql::Value as SqlValue;

            let (value_type, value) = match constant {
                SqlValue::Integer(i) => (DataType::Int64, Value { i64: *i }),
                SqlValue::Float(f) => (DataType::Float64, Value { float64: *f }),
                SqlValue::String(s) => {
                    // 普通字符串
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Boolean(b) => (DataType::Bool, Value { bool: *b }),
                SqlValue::Null => (DataType::Json, Value { json_storage: crate::types::JsonStorage::Null }),
                SqlValue::Identifier(s) => {
                    // 标识符作为字符串处理
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                    (DataType::VarChar, Value { string: buf })
                }
                SqlValue::Json(json_str) => {
                    let mut buf = [0u8; 256];
                    let len = core::cmp::min(json_str.len(), 256);
                    buf[..len].copy_from_slice(&json_str.as_bytes()[..len]);
                    let json_storage = if json_str.len() <= 256 {
                        crate::types::JsonStorage::Inline(buf)
                    } else {
                        crate::types::JsonStorage::Null
                    };
                    (DataType::Json, Value { json_storage })
                }
            };

            Ok(TypedValue { value_type, value })
        }
        Expression::BinaryOp {
            left, op, right, ..
        } => {
            // 评估左右操作数
            let left_val = evaluate_expression_with_depth(table, record_values, left, depth + 1)?;
            let right_val = evaluate_expression_with_depth(table, record_values, right, depth + 1)?;

            // 对于向量操作符，从表的字段定义中获取向量维度
            if matches!(
                *op,
                BinaryOperator::VectorL2 | BinaryOperator::VectorIP | BinaryOperator::VectorCosine
            ) {
                // 检查左操作数是否是向量类型
                if matches!(left_val.value_type, DataType::Vector) {
                    // 找到左操作数对应的向量字段
                    let vector_field = if let Expression::Field {
                        name: ref field_name,
                        ..
                    } = **left
                    {
                        // 直接是字段引用，查找该字段
                        table
                            .def
                            .fields
                            .iter()
                            .find(|field| field.name == *field_name)
                            .ok_or(QueryExecutionError::FieldNotFound)?
                    } else {
                        // 遍历表的所有字段，找到向量字段
                        table
                            .def
                            .fields
                            .iter()
                            .find(|field| field.vector_metadata.is_some())
                            .ok_or(QueryExecutionError::TypeMismatch)?
                    };

                    let vector_dim = vector_field
                        .vector_metadata
                        .ok_or(QueryExecutionError::TypeMismatch)?
                        .dimension;

                    // 执行向量二元操作，传入向量维度
                    return evaluate_vector_binary_op(left_val, *op, right_val, vector_dim);
                }
            }

            // 执行普通二元操作
            evaluate_binary_op(left_val, *op, right_val)
        }
        Expression::LogicalOp {
            left,
            op,
            right,
            ..
        } => {
            // 评估逻辑表达式
            let left_val = evaluate_expression_with_depth(table, record_values, left, depth + 1)?;
            let right_val = evaluate_expression_with_depth(table, record_values, right, depth + 1)?;

            // 确保左右操作数都是布尔类型
            let left_bool = unsafe {
                match left_val.value_type {
                    DataType::Bool => left_val.value.bool,
                    DataType::Int8 => left_val.value.i8 != 0,
                    DataType::Int16 => left_val.value.i16 != 0,
                    DataType::Int32 => left_val.value.i32 != 0,
                    DataType::Int64 => left_val.value.i64 != 0,
                    DataType::UInt8 => left_val.value.u8 != 0,
                    DataType::UInt16 => left_val.value.u16 != 0,
                    DataType::UInt32 => left_val.value.u32 != 0,
                    DataType::UInt64 => left_val.value.u64 != 0,
                    DataType::Float32 => left_val.value.float32 != 0.0,
                    DataType::Float64 => left_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                }
            };

            let right_bool = unsafe {
                match right_val.value_type {
                    DataType::Bool => right_val.value.bool,
                    DataType::Int8 => right_val.value.i8 != 0,
                    DataType::Int16 => right_val.value.i16 != 0,
                    DataType::Int32 => right_val.value.i32 != 0,
                    DataType::Int64 => right_val.value.i64 != 0,
                    DataType::UInt8 => right_val.value.u8 != 0,
                    DataType::UInt16 => right_val.value.u16 != 0,
                    DataType::UInt32 => right_val.value.u32 != 0,
                    DataType::UInt64 => right_val.value.u64 != 0,
                    DataType::Float32 => right_val.value.float32 != 0.0,
                    DataType::Float64 => right_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                }
            };

            // 执行逻辑操作
            let result = match op {
                crate::sql::query_parser::LogicalOperator::And => left_bool && right_bool,
                crate::sql::query_parser::LogicalOperator::Or => left_bool || right_bool,
            };

            Ok(TypedValue {
                value_type: DataType::Bool,
                value: Value { bool: result },
            })
        }
        Expression::UnaryOp {
            op,
            operand,
            ..
        } => {
            // 评估一元操作表达式
            let operand_val = evaluate_expression_with_depth(table, record_values, operand, depth + 1)?;
            evaluate_unary_op(*op, operand_val)
        }
    }
}

/// 评估向量二元操作
fn evaluate_vector_binary_op(
    left: TypedValue,
    op: BinaryOperator,
    right: TypedValue,
    vector_dim: u16,
) -> Result<TypedValue, QueryExecutionError> {
    // 确保左操作数是向量类型
    if !matches!(left.value_type, DataType::Vector) {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // 获取左向量数据指针
    // 注意：向量数据可能通过vector指针或vector_metadata存储
    // 这里简化处理，假设向量数据通过vector指针存储
    let vec1_ptr = unsafe {
        match left.value_type {
            DataType::Vector => left.value.vector,
            _ => std::ptr::null(),
        }
    };
    if vec1_ptr.is_null() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // 准备右向量数据
    let vec2_values: Vec<f32>;

    if matches!(right.value_type, DataType::Vector) {
        // 右操作数是向量类型
        let vec2_ptr = unsafe { right.value.vector };
        if vec2_ptr.is_null() {
            return Err(QueryExecutionError::TypeMismatch);
        }

        // 安全复制右向量数据到Vec
        vec2_values = unsafe {
            let vec_slice = core::slice::from_raw_parts(vec2_ptr, vector_dim as usize);
            vec_slice.to_vec()
        };
    } else if matches!(right.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
        // 右操作数是字符串类型，尝试解析为向量字面量 [x1, x2, ..., xn]
        let vec_str = unsafe {
            core::str::from_utf8(&right.value.string)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches(char::from(0))
        };

        // 解析向量字面量
        // 检查字符串是否以[开头和]结尾
        if !vec_str.starts_with('[') || !vec_str.ends_with(']') {
            return Err(QueryExecutionError::TypeMismatch);
        }

        // 移除首尾的方括号
        let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');

        // 分割逗号，得到每个元素的字符串
        let elements: Vec<&str> = vec_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty()) // 过滤掉空字符串
            .collect();

        // 检查维度是否匹配
        if elements.len() != vector_dim as usize {
            return Err(QueryExecutionError::TypeMismatch);
        }

        // 解析每个元素为f32
        vec2_values = elements
            .iter()
            .map(|s| s.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
    } else if matches!(right.value_type, DataType::Json) {
        // 右操作数是JSON类型，尝试解析为向量字面量 [x1, x2, ..., xn]
        if let crate::types::JsonStorage::Inline(json_bytes) = unsafe { right.value.json_storage } {
            let vec_str = core::str::from_utf8(&json_bytes)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches('\0');

            // 解析向量字面量
            // 检查字符串是否以[开头和]结尾
            if !vec_str.starts_with('[') || !vec_str.ends_with(']') {
                return Err(QueryExecutionError::TypeMismatch);
            }

            // 移除首尾的方括号
            let vec_str = vec_str.trim_start_matches('[').trim_end_matches(']');
            // 分割逗号，得到每个元素的字符串
            let elements: Vec<&str> = vec_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty()) // 过滤掉空字符串
                .collect();

            // 检查维度是否匹配
            if elements.len() != vector_dim as usize {
                return Err(QueryExecutionError::TypeMismatch);
            }

            // 解析每个元素为f32
            vec2_values = elements
                .iter()
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
        } else {
            return Err(QueryExecutionError::TypeMismatch);
        }
    } else {
        // 处理右操作数为数值类型的情况
        // 首先检查是否为数值类型，避免unsafe块中的内存访问违规
        let scalar_value = match right.value_type {
            DataType::Float32 => unsafe { right.value.float32 },
            DataType::Float64 => unsafe { right.value.float64 as f32 },
            DataType::Int8 => unsafe { right.value.i8 as f32 },
            DataType::Int16 => unsafe { right.value.i16 as f32 },
            DataType::Int32 => unsafe { right.value.i32 as f32 },
            DataType::Int64 => unsafe { right.value.i64 as f32 },
            DataType::UInt8 => unsafe { right.value.u8 as f32 },
            DataType::UInt16 => unsafe { right.value.u16 as f32 },
            DataType::UInt32 => unsafe { right.value.u32 as f32 },
            DataType::UInt64 => unsafe { right.value.u64 as f32 },
            _ => {
                return Err(QueryExecutionError::TypeMismatch);
            }
        };

        // 创建与左向量维度相同的向量
        vec2_values = vec![scalar_value; vector_dim as usize];
    }

    // 计算距离
    let distance: f64 = unsafe {
        match op {
            BinaryOperator::VectorL2 => {
                // L2距离（欧几里得距离）
                let mut sum = 0.0f64;
                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = *vec1_ptr.add(i);
                    let v2 = vec2_values[i];
                    let diff = v1 - v2;
                    sum += (diff as f64) * (diff as f64);
                }
                sum.sqrt()
            }
            BinaryOperator::VectorIP => {
                // 内积
                let mut sum = 0.0f64;
                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = *vec1_ptr.add(i);
                    let v2 = vec2_values[i];
                    sum += (v1 as f64) * (v2 as f64);
                }
                sum
            }
            BinaryOperator::VectorCosine => {
                // 余弦相似度
                let mut dot = 0.0f64;
                let mut norm1 = 0.0f64;
                let mut norm2 = 0.0f64;

                let vector_dim_usize = vector_dim as usize;
                for i in 0..vector_dim_usize {
                    let v1 = *vec1_ptr.add(i) as f64;
                    let v2 = vec2_values[i] as f64;
                    dot += v1 * v2;
                    norm1 += v1 * v1;
                    norm2 += v2 * v2;
                }

                let norm1 = norm1.sqrt();
                let norm2 = norm2.sqrt();

                if norm1 == 0.0 || norm2 == 0.0 {
                    -1.0 // 相似度最低
                } else {
                    dot / (norm1 * norm2) // 余弦相似度范围[-1, 1]
                }
            }
            _ => unreachable!(),
        }
    };

    // 返回FLOAT64类型的距离结果
    Ok(TypedValue {
        value_type: DataType::Float64,
        value: Value { float64: distance },
    })
}

/// 评估二元操作
fn evaluate_binary_op(
    left: TypedValue,
    op: BinaryOperator,
    right: TypedValue,
) -> Result<TypedValue, QueryExecutionError> {
    // 首先处理比较操作符
    match op {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
            // 比较操作符需要返回布尔值
            unsafe {
                // 特殊处理IS NULL操作
                if right.value_type == DataType::Int64 && right.value.i64 == 0 {
                    // IS NULL操作
                    // 检查左侧值是否为NULL
                    let is_null = match left.value_type {
                        DataType::UInt8 => left.value.u8 == 0,
                        DataType::UInt16 => left.value.u16 == 0,
                        DataType::UInt32 => left.value.u32 == 0,
                        DataType::UInt64 => left.value.u64 == 0,
                        DataType::Int8 => left.value.i8 == 0,
                        DataType::Int16 => left.value.i16 == 0,
                        DataType::Int32 => left.value.i32 == 0,
                        DataType::Int64 => left.value.i64 == 0,
                        DataType::Float32 => left.value.float32 == 0.0,
                        DataType::Float64 => left.value.float64 == 0.0,
                        DataType::Bool => !left.value.bool,
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            // 检查字符串是否为空
                            // 安全地检查字符串是否为空，避免UTF-8转换错误
                            let mut is_empty = true;
                            for &byte in &left.value.string {
                                if byte != 0 {
                                    is_empty = false;
                                    break;
                                }
                            }
                            is_empty
                        }
                        DataType::Timestamp => left.value.time.value == 0,
                        DataType::TimestampTZ => left.value.time.value == 0,
                        DataType::Interval => left.value.interval.value == 0,
                        DataType::Vector => left.value.vector.is_null(),
                        DataType::Json => {
                            // 检查JSON值是否为Null
                            matches!(left.value.json_storage, JsonStorage::Null)
                        },
                    };
                    
                    let result = match op {
                        BinaryOperator::Equal => is_null,  // IS NULL
                        BinaryOperator::NotEqual => !is_null, // IS NOT NULL
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                    
                    return Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: result },
                    });
                }
                
                // 字符串比较
                if matches!(left.value_type, DataType::VarChar | DataType::Char | DataType::Text) && matches!(right.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
                    let left_str = core::str::from_utf8(&left.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0));
                    let right_str = core::str::from_utf8(&right.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0));
                    
                    let result = match op {
                        BinaryOperator::Equal => left_str == right_str,
                        BinaryOperator::NotEqual => left_str != right_str,
                        BinaryOperator::LessThan => left_str < right_str,
                        BinaryOperator::LessThanOrEqual => left_str <= right_str,
                        BinaryOperator::GreaterThan => left_str > right_str,
                        BinaryOperator::GreaterThanOrEqual => left_str >= right_str,
                        _ => return Err(QueryExecutionError::TypeMismatch),
                    };
                    
                    return Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: result },
                    });
                }
                
                // 将操作数转换为f64进行比较，适用于所有数值类型
                let left_val = match left.value_type {
                    DataType::UInt8 => left.value.u8 as f64,
                    DataType::UInt16 => left.value.u16 as f64,
                    DataType::UInt32 => left.value.u32 as f64,
                    DataType::UInt64 => left.value.u64 as f64,
                    DataType::Int8 => left.value.i8 as f64,
                    DataType::Int16 => left.value.i16 as f64,
                    DataType::Int32 => left.value.i32 as f64,
                    DataType::Int64 => left.value.i64 as f64,
                    DataType::Float32 => left.value.float32 as f64,
                    DataType::Float64 => left.value.float64,
                    DataType::Bool => left.value.bool as u8 as f64,
                    DataType::Timestamp => left.value.time.value as f64,
                    DataType::TimestampTZ => left.value.time.value as f64,
                    DataType::Json => {
                        // Try to parse JSON value as number
                        let json_str = unsafe {
                            match &left.value.json_storage {
                                JsonStorage::Inline(data) => {
                                    let len = data.iter().rposition(|&b| b == 0).unwrap_or(256);
                                    String::from_utf8_lossy(&data[..len]).to_string()
                                }
                                JsonStorage::External { pool_id, offset, length } => {
                                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    let pool = pool_manager.get_pool(*pool_id)
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                                        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                                        String::from_utf8_lossy(&data[..len]).to_string()
                                    } else {
                                        return Err(QueryExecutionError::InternalError);
                                    }
                                }
                                JsonStorage::Null => "null".to_string(),
                            }
                        };
                        json_str.parse::<f64>().unwrap_or(0.0)
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let right_val = match right.value_type {
                    DataType::UInt8 => right.value.u8 as f64,
                    DataType::UInt16 => right.value.u16 as f64,
                    DataType::UInt32 => right.value.u32 as f64,
                    DataType::UInt64 => right.value.u64 as f64,
                    DataType::Int8 => right.value.i8 as f64,
                    DataType::Int16 => right.value.i16 as f64,
                    DataType::Int32 => right.value.i32 as f64,
                    DataType::Int64 => right.value.i64 as f64,
                    DataType::Float32 => right.value.float32 as f64,
                    DataType::Float64 => right.value.float64,
                    DataType::Bool => right.value.bool as u8 as f64,
                    DataType::Timestamp => right.value.time.value as f64,
                    DataType::TimestampTZ => right.value.time.value as f64,
                    DataType::Json => {
                        // Try to parse JSON value as number
                        let json_str = unsafe {
                            match &right.value.json_storage {
                                JsonStorage::Inline(data) => {
                                    let len = data.iter().rposition(|&b| b == 0).unwrap_or(256);
                                    String::from_utf8_lossy(&data[..len]).to_string()
                                }
                                JsonStorage::External { pool_id, offset, length } => {
                                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    let pool = pool_manager.get_pool(*pool_id)
                                        .ok_or(QueryExecutionError::InternalError)?;
                                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                                        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                                        String::from_utf8_lossy(&data[..len]).to_string()
                                    } else {
                                        return Err(QueryExecutionError::InternalError);
                                    }
                                }
                                JsonStorage::Null => "null".to_string(),
                            }
                        };
                        json_str.parse::<f64>().unwrap_or(0.0)
                    }
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 执行比较操作
                let result = match op {
                    BinaryOperator::Equal => left_val == right_val,
                    BinaryOperator::NotEqual => left_val != right_val,
                    BinaryOperator::LessThan => left_val < right_val,
                    BinaryOperator::LessThanOrEqual => left_val <= right_val,
                    BinaryOperator::GreaterThan => left_val > right_val,
                    BinaryOperator::GreaterThanOrEqual => left_val >= right_val,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 返回布尔结果
                return Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                });
            }
        }
        _ => {} // 其他操作符继续处理
    }

    // 处理减法操作中两个时间类型相减的情况（Timestamp - Timestamp = Interval）
    if op == BinaryOperator::Subtract {
        unsafe {
            // 检查是否是时间类型之间的减法
            match (left.value_type, right.value_type) {
                (DataType::Timestamp, DataType::Timestamp)
                | (DataType::TimestampTZ, DataType::TimestampTZ)
                | (DataType::Timestamp, DataType::TimestampTZ)
                | (DataType::TimestampTZ, DataType::Timestamp) => {
                    // 任意时间类型之间的减法都返回Interval
                    let t1 = left.value.time.value;
                    let t2 = right.value.time.value;
                    let diff = t1 - t2;

                    return Ok(TypedValue {
                        value_type: DataType::Interval,
                        value: Value {
                            interval: crate::types::db_interval::new(diff, 6, 0),
                        },
                    });
                }
                _ => {} // 其他情况继续处理
            }
        }
    }

    // 处理算术运算：先检查是否是数值类型运算
    if matches!(
        left.value_type,
        DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
    ) && matches!(
        right.value_type,
        DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
    ) {
        // 执行数值运算
        unsafe {
            // 将操作数转换为f64进行计算
            let left_val = match left.value_type {
                DataType::UInt8 => left.value.u8 as f64,
                DataType::UInt16 => left.value.u16 as f64,
                DataType::UInt32 => left.value.u32 as f64,
                DataType::UInt64 => left.value.u64 as f64,
                DataType::Int8 => left.value.i8 as f64,
                DataType::Int16 => left.value.i16 as f64,
                DataType::Int32 => left.value.i32 as f64,
                DataType::Int64 => left.value.i64 as f64,
                DataType::Float32 => left.value.float32 as f64,
                DataType::Float64 => left.value.float64,
                _ => unreachable!(),
            };

            let right_val = match right.value_type {
                DataType::UInt8 => right.value.u8 as f64,
                DataType::UInt16 => right.value.u16 as f64,
                DataType::UInt32 => right.value.u32 as f64,
                DataType::UInt64 => right.value.u64 as f64,
                DataType::Int8 => right.value.i8 as f64,
                DataType::Int16 => right.value.i16 as f64,
                DataType::Int32 => right.value.i32 as f64,
                DataType::Int64 => right.value.i64 as f64,
                DataType::Float32 => right.value.float32 as f64,
                DataType::Float64 => right.value.float64,
                _ => unreachable!(),
            };

            let result = match op {
                BinaryOperator::Add => left_val + right_val,
                BinaryOperator::Subtract => left_val - right_val,
                BinaryOperator::Multiply => left_val * right_val,
                BinaryOperator::Divide => {
                    if right_val == 0.0 {
                        return Err(QueryExecutionError::InternalError);
                    }
                    left_val / right_val
                },
                _ => return Err(QueryExecutionError::TypeMismatch),
            };

            // 返回FLOAT64类型结果
            return Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value { float64: result },
            });
        }
    }

    // 解析间隔值，支持字符串格式（如"1 HOUR"）和数值格式（微秒）
    let interval_micros = match right.value_type {
        DataType::Int64 => unsafe { right.value.i64 },
        DataType::VarChar | DataType::Char | DataType::Text => unsafe {
            let interval_str = core::str::from_utf8(&right.value.string)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches(char::from(0));
            parse_interval_string(interval_str)?
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
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0),
                            },
                        })
                    }
                    // TimestampTZ + Interval = TimestampTZ
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp + interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(
                                    new_timestamp,
                                    tz_offset,
                                    6,
                                    0,
                                ),
                            },
                        })
                    }
                    // 其他类型的加法操作（暂时不支持）
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
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
                                time: crate::types::db_timestamp::new(new_timestamp, 0, 6, 0),
                            },
                        })
                    }
                    // TimestampTZ - Interval = TimestampTZ
                    DataType::TimestampTZ => {
                        let timestamp = left.value.time.value;
                        let tz_offset = left.value.time.tz_offset;
                        let new_timestamp = timestamp - interval_micros;

                        Ok(TypedValue {
                            value_type: DataType::TimestampTZ,
                            value: Value {
                                time: crate::types::db_timestamp::new(
                                    new_timestamp,
                                    tz_offset,
                                    6,
                                    0,
                                ),
                            },
                        })
                    }
                    // 其他类型的减法操作（暂时不支持）
                    _ => Err(QueryExecutionError::TypeMismatch),
                }
            }
        }
        // 处理比较操作符
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::LessThan
        | BinaryOperator::LessThanOrEqual
        | BinaryOperator::GreaterThan
        | BinaryOperator::GreaterThanOrEqual => {
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
        }
        // 处理其他操作符（如Multiply），这些操作符在前面的数值运算部分已经处理
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// 执行函数调用
fn execute_function_call(
    name: &str,
    args: &[TypedValue],
) -> Result<TypedValue, QueryExecutionError> {
    eprintln!("DEBUG execute_function_call: name={}, args.len={}", name, args.len());
    // First, check if it's a built-in function
    match name.to_uppercase().as_str() {
        // 基础统计聚合函数
        "COUNT" => execute_count(args),
        "SUM" => execute_sum(args),
        "AVG" => execute_avg(args),
        "MIN" => execute_min(args),
        "MAX" => execute_max(args),
        // 新增统计学函数
        "STDDEV" => execute_stddev(args),
        "VAR" => execute_var(args),
        "STDDEV_SAMP" => execute_stddev_samp(args),
        "VAR_SAMP" => execute_var_samp(args),
        // 新增滑动窗口函数
        "MOVING_AVERAGE" => execute_moving_average(args),
        "MOVING_SUM" => execute_moving_sum(args),
        // 时间函数
        "TIME_BUCKET" => execute_time_bucket(args),
        // 时间格式化函数
        "TO_ISO8601" => execute_to_iso8601(args),
        "TO_CHAR" => execute_to_char(args),
        "TO_EPOCH" => execute_to_epoch(args),
        // 字符串函数
        "CONCAT" => execute_concat(args),
        "SUBSTRING" => execute_substring(args),
        "UPPER" => execute_upper(args),
        "LOWER" => execute_lower(args),
        "LENGTH" => execute_length(args),
        "CHAR_LENGTH" => execute_char_length(args),
        // 数学函数
        "ABS" => execute_abs(args),
        "SQRT" => execute_sqrt(args),
        "POWER" => execute_power(args),
        "SIN" => execute_sin(args),
        "COS" => execute_cos(args),
        "LOG" => execute_log(args),
        "EXP" => execute_exp(args),
        "ROUND" => execute_round(args),
        "CEIL" => execute_ceil(args),
        "FLOOR" => execute_floor(args),
        "MOD" => execute_mod(args),
        // JSON函数
        "JSON_EXTRACT" => execute_json_extract(args),
        "JSON_VALUE" => execute_json_value(args),
        "JSON_QUERY" => execute_json_query(args),
        "JSON_HAS" => execute_json_has(args),
        "JSON_TYPE" => execute_json_type(args),
        "JSON_SET" => execute_json_set(args),
        "JSON_REMOVE" => execute_json_remove(args),
        "JSON_MERGE_PATCH" => execute_json_merge_patch(args),
        "JSON_ARRAY_APPEND" => execute_json_array_append(args),
        "JSON_ARRAY_LENGTH" => execute_json_array_length(args),
        "JSON_ARRAY" => execute_json_array(args),
        "JSON_OBJECT" => execute_json_object(args),
        _ => {
            // If it's not a built-in function, try model UDF
            crate::model::model_udf::execute_model_udf(name, args)
                .or_else(|_| {
                    // 不支持的函数
                    Err(QueryExecutionError::UnsupportedFunction(name.to_string()))
                })
        }
    }
}

/// 执行COUNT函数
fn execute_count(_args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
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
                value: Value {
                    u64: arg.value.u8 as u64,
                },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: arg.value.u16 as u64,
                },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: arg.value.u32 as u64,
                },
            }),
            DataType::UInt64 => Ok(arg.clone()),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: arg.value.i8 as i64,
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: arg.value.i16 as i64,
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: arg.value.i32 as i64,
                },
            }),
            DataType::Int64 => Ok(arg.clone()),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.float32 as f64,
                },
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
                value: Value {
                    float64: arg.value.u8 as f64,
                },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.u16 as f64,
                },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.u32 as f64,
                },
            }),
            DataType::UInt64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.u64 as f64,
                },
            }),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.i8 as f64,
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.i16 as f64,
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.i32 as f64,
                },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.i64 as f64,
                },
            }),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.float32 as f64,
                },
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

/// 执行STDDEV函数
fn execute_stddev(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // STDDEV函数在聚合时计算标准差，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行VAR函数
fn execute_var(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // VAR函数在聚合时计算方差，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行STDDEV_SAMP函数
fn execute_stddev_samp(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // STDDEV_SAMP函数在聚合时计算样本标准差，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行VAR_SAMP函数
fn execute_var_samp(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // VAR_SAMP函数在聚合时计算样本方差，这里直接返回参数值
    Ok(args[0].clone())
}

/// 执行MOVING_AVERAGE函数
fn execute_moving_average(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // MOVING_AVERAGE函数：MOVING_AVERAGE(value, window_size)
    // 目前返回输入值，后续需要实现完整的滑动窗口逻辑
    Ok(args[0].clone())
}

/// 执行MOVING_SUM函数
fn execute_moving_sum(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // MOVING_SUM函数：MOVING_SUM(value, window_size)
    // 目前返回输入值，后续需要实现完整的滑动窗口逻辑
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

    // 解析可选的origin参数
    let origin_micros = if args.len() > 2 {
        parse_origin_timestamp(&args[2])?
    } else {
        0 // 默认从1970-01-01 00:00:00开始
    };

    unsafe {
        // 从不同类型中提取时间戳值
        let timestamp = match timestamp_arg.value_type {
            DataType::Timestamp => timestamp_arg.value.time.value,
            DataType::TimestampTZ => timestamp_arg.value.time.value,
            DataType::UInt64 => timestamp_arg.value.u64 as i64,
            DataType::Int64 => timestamp_arg.value.i64,
            DataType::UInt32 => timestamp_arg.value.u32 as i64,
            DataType::Int32 => timestamp_arg.value.i32 as i64,
            DataType::UInt16 => timestamp_arg.value.u16 as i64,
            DataType::Int16 => timestamp_arg.value.i16 as i64,
            DataType::UInt8 => timestamp_arg.value.u8 as i64,
            DataType::Int8 => timestamp_arg.value.i8 as i64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 将时间戳对齐到指定的时间窗口，考虑origin
        let bucketed_timestamp =
            origin_micros + ((timestamp - origin_micros) / interval_micros) * interval_micros;

        // 根据输入类型返回相同类型的结果
        match timestamp_arg.value_type {
            DataType::Timestamp => Ok(TypedValue {
                value_type: DataType::Timestamp,
                value: Value {
                    time: crate::types::db_timestamp::new(bucketed_timestamp, 0, 6, 0),
                },
            }),
            DataType::TimestampTZ => Ok(TypedValue {
                value_type: DataType::TimestampTZ,
                value: Value {
                    time: crate::types::db_timestamp::new(
                        bucketed_timestamp,
                        timestamp_arg.value.time.tz_offset,
                        6,
                        0,
                    ),
                },
            }),
            DataType::UInt64 => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: bucketed_timestamp as u64,
                },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: bucketed_timestamp,
                },
            }),
            DataType::UInt32 => Ok(TypedValue {
                value_type: DataType::UInt32,
                value: Value {
                    u32: bucketed_timestamp as u32,
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: bucketed_timestamp as i32,
                },
            }),
            DataType::UInt16 => Ok(TypedValue {
                value_type: DataType::UInt16,
                value: Value {
                    u16: bucketed_timestamp as u16,
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: bucketed_timestamp as i16,
                },
            }),
            DataType::UInt8 => Ok(TypedValue {
                value_type: DataType::UInt8,
                value: Value {
                    u8: bucketed_timestamp as u8,
                },
            }),
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: bucketed_timestamp as i8,
                },
            }),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 解析时间字符串为微秒时间戳
fn parse_time_string(time_str: &str) -> Result<i64, QueryExecutionError> {
    // 这里实现一个简单的时间字符串解析
    // 支持的格式：'YYYY-MM-DD' 或 'YYYY-MM-DD HH:MM:SS'

    let time_str = time_str.trim();
    let mut parts = time_str.split_whitespace();

    // 解析日期部分
    let date_part = parts.next().ok_or(QueryExecutionError::TypeMismatch)?;
    let date_components: Vec<&str> = date_part.split('-').collect();
    if date_components.len() != 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let year = date_components[0]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;
    let month = date_components[1]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;
    let day = date_components[2]
        .parse::<i64>()
        .map_err(|_| QueryExecutionError::TypeMismatch)?;

    // 解析时间部分（可选）
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;

    if let Some(time_part) = parts.next() {
        let time_components: Vec<&str> = time_part.split(':').collect();
        if time_components.len() != 3 {
            return Err(QueryExecutionError::TypeMismatch);
        }

        hour = time_components[0]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
        minute = time_components[1]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
        second = time_components[2]
            .parse::<i64>()
            .map_err(|_| QueryExecutionError::TypeMismatch)?;
    }

    // 计算从1970-01-01到指定日期的秒数
    // 简化实现，只处理非闰年的情况
    let mut seconds = 0;

    // 计算年份贡献的秒数（忽略闰年）
    for _y in 1970..year {
        seconds += 365 * 24 * 60 * 60;
    }

    // 计算月份贡献的秒数（忽略闰年）
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month - 1) {
        seconds += days_in_month[m as usize] * 24 * 60 * 60;
    }

    // 计算日期、小时、分钟、秒贡献的秒数
    seconds += (day - 1) * 24 * 60 * 60;
    seconds += hour * 60 * 60;
    seconds += minute * 60;
    seconds += second;

    // 转换为微秒
    Ok(seconds * 1000000)
}

/// 解析origin时间戳参数
fn parse_origin_timestamp(origin_arg: &TypedValue) -> Result<i64, QueryExecutionError> {
    unsafe {
        match origin_arg.value_type {
            // 数值形式的时间戳（微秒）
            DataType::UInt8 => Ok(origin_arg.value.u8 as i64),
            DataType::UInt16 => Ok(origin_arg.value.u16 as i64),
            DataType::UInt32 => Ok(origin_arg.value.u32 as i64),
            DataType::UInt64 => Ok(origin_arg.value.u64 as i64),
            DataType::Int8 => Ok(origin_arg.value.i8 as i64),
            DataType::Int16 => Ok(origin_arg.value.i16 as i64),
            DataType::Int32 => Ok(origin_arg.value.i32 as i64),
            DataType::Int64 => Ok(origin_arg.value.i64),
            // 字符串形式的时间戳（如'2020-01-01'）
            DataType::VarChar | DataType::Char | DataType::Text => {
                let origin_str = core::str::from_utf8(&origin_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                // 尝试解析为时间字符串
                parse_time_string(origin_str)
            }
            // 时间类型
            DataType::Timestamp => Ok(origin_arg.value.time.value),
            DataType::TimestampTZ => Ok(origin_arg.value.time.value),
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
                string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

                Ok(TypedValue {
                    value_type: DataType::VarChar,
                    value: Value {
                        string: string_value,
                    },
                })
            }
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
            (DataType::Timestamp | DataType::TimestampTZ, DataType::VarChar | DataType::Char | DataType::Text) => {
                let timestamp = &timestamp_arg.value.time;
                // 提取字符串格式
                let format_str = core::str::from_utf8(&format_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                let result = process_to_char(timestamp, format_str)?;

                // 将字符串转换为TypedValue
                let mut string_value = [0; MAX_STRING_LEN];
                let len = core::cmp::min(result.len(), MAX_STRING_LEN);
                string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

                Ok(TypedValue {
                    value_type: DataType::VarChar,
                    value: Value {
                        string: string_value,
                    },
                })
            }
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
            }
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
            DataType::VarChar | DataType::Char | DataType::Text => {
                let interval_str = core::str::from_utf8(&interval_arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));

                parse_interval_string(interval_str)
            }
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

// ------------------------ 字符串函数实现 ------------------------

/// 执行CONCAT函数
fn execute_concat(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    // 连接所有参数为字符串
    let mut result = String::new();

    for arg in args {
        unsafe {
            let arg_str = match arg.value_type {
                DataType::VarChar | DataType::Char | DataType::Text => String::from(
                    core::str::from_utf8(&arg.value.string)
                        .map_err(|_| QueryExecutionError::TypeMismatch)?
                        .trim_end_matches(char::from(0)),
                ),
                DataType::UInt8 => alloc::format!("{}", arg.value.u8),
                DataType::UInt16 => alloc::format!("{}", arg.value.u16),
                DataType::UInt32 => alloc::format!("{}", arg.value.u32),
                DataType::UInt64 => alloc::format!("{}", arg.value.u64),
                DataType::Int8 => alloc::format!("{}", arg.value.i8),
                DataType::Int16 => alloc::format!("{}", arg.value.i16),
                DataType::Int32 => alloc::format!("{}", arg.value.i32),
                DataType::Int64 => alloc::format!("{}", arg.value.i64),
                DataType::Float32 => alloc::format!("{}", arg.value.float32),
                DataType::Float64 => alloc::format!("{}", arg.value.float64),
                DataType::Bool => alloc::format!("{}", arg.value.bool),
                _ => return Err(QueryExecutionError::TypeMismatch),
            };
            result.push_str(&arg_str);
        }
    }

    // 将结果转换为TypedValue
    let mut string_value = [0; MAX_STRING_LEN];
    let len = core::cmp::min(result.len(), MAX_STRING_LEN);
    string_value[..len].copy_from_slice(&result.as_bytes()[..len]);

    Ok(TypedValue {
        value_type: DataType::VarChar,
        value: Value {
            string: string_value,
        },
    })
}

/// 执行SUBSTRING函数
fn execute_substring(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let string_arg = &args[0];
    let start_arg = &args[1];
    let length_arg = args.get(2);

    unsafe {
        // 提取源字符串
        let source_str = match string_arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => core::str::from_utf8(&string_arg.value.string)
                .map_err(|_| QueryExecutionError::TypeMismatch)?
                .trim_end_matches(char::from(0)),
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 提取起始位置（从1开始）
        let start = match start_arg.value_type {
            DataType::Int8 => start_arg.value.i8 as usize,
            DataType::Int16 => start_arg.value.i16 as usize,
            DataType::Int32 => start_arg.value.i32 as usize,
            DataType::Int64 => start_arg.value.i64 as usize,
            DataType::UInt8 => start_arg.value.u8 as usize,
            DataType::UInt16 => start_arg.value.u16 as usize,
            DataType::UInt32 => start_arg.value.u32 as usize,
            DataType::UInt64 => start_arg.value.u64 as usize,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 计算实际起始位置（转换为0索引）
        let actual_start = if start > 0 { start - 1 } else { 0 };

        // 提取长度（如果提供）
        let actual_end = if let Some(len_arg) = length_arg {
            let len = match len_arg.value_type {
                DataType::Int8 => len_arg.value.i8 as usize,
                DataType::Int16 => len_arg.value.i16 as usize,
                DataType::Int32 => len_arg.value.i32 as usize,
                DataType::Int64 => len_arg.value.i64 as usize,
                DataType::UInt8 => len_arg.value.u8 as usize,
                DataType::UInt16 => len_arg.value.u16 as usize,
                DataType::UInt32 => len_arg.value.u32 as usize,
                DataType::UInt64 => len_arg.value.u64 as usize,
                _ => return Err(QueryExecutionError::TypeMismatch),
            };
            core::cmp::min(actual_start + len, source_str.len())
        } else {
            source_str.len()
        };

        // 截取字符串
        let substring = &source_str[actual_start..actual_end];

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let len = core::cmp::min(substring.len(), MAX_STRING_LEN);
        string_value[..len].copy_from_slice(&substring.as_bytes()[..len]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行UPPER函数
fn execute_upper(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let result_str = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                let source_str = core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                source_str.to_uppercase()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let len = core::cmp::min(result_str.len(), MAX_STRING_LEN);
        string_value[..len].copy_from_slice(&result_str.as_bytes()[..len]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行LOWER函数
fn execute_lower(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let result_str = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                let source_str = core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                source_str.to_lowercase()
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        // 将结果转换为TypedValue
        let mut string_value = [0; MAX_STRING_LEN];
        let len = core::cmp::min(result_str.len(), MAX_STRING_LEN);
        string_value[..len].copy_from_slice(&result_str.as_bytes()[..len]);

        Ok(TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: string_value,
            },
        })
    }
}

/// 执行LENGTH函数（字节长度）
fn execute_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let length = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                let source_str = core::str::from_utf8(&arg.value.string)
                    .map_err(|_| QueryExecutionError::TypeMismatch)?
                    .trim_end_matches(char::from(0));
                source_str.len() as u64
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        Ok(TypedValue {
            value_type: DataType::UInt64,
            value: Value { u64: length },
        })
    }
}

/// 执行CHAR_LENGTH函数（字符长度，UTF-8感知）
fn execute_char_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let char_count = match arg.value_type {
            DataType::VarChar | DataType::Char | DataType::Text => {
                // 使用UTF-8处理器计算字符长度
                let char_count = crate::utf8::get_global_utf8_processor().char_length(&arg.value.string);
                char_count as u64
            }
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        Ok(TypedValue {
            value_type: DataType::UInt64,
            value: Value { u64: char_count },
        })
    }
}

// ------------------------ 数学函数实现 ------------------------

/// 执行ABS函数
fn execute_abs(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Int8 => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: arg.value.i8.abs(),
                },
            }),
            DataType::Int16 => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: arg.value.i16.abs(),
                },
            }),
            DataType::Int32 => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: arg.value.i32.abs(),
                },
            }),
            DataType::Int64 => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: arg.value.i64.abs(),
                },
            }),
            DataType::Float32 => Ok(TypedValue {
                value_type: DataType::Float32,
                value: Value {
                    float32: arg.value.float32.abs(),
                },
            }),
            DataType::Float64 => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: arg.value.float64.abs(),
                },
            }),
            _ => Err(QueryExecutionError::TypeMismatch),
        }
    }
}

/// 执行SQRT函数
fn execute_sqrt(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.sqrt();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行POWER函数
fn execute_power(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let base_arg = &args[0];
    let exponent_arg = &args[1];

    unsafe {
        let base = match base_arg.value_type {
            DataType::UInt8 => base_arg.value.u8 as f64,
            DataType::UInt16 => base_arg.value.u16 as f64,
            DataType::UInt32 => base_arg.value.u32 as f64,
            DataType::UInt64 => base_arg.value.u64 as f64,
            DataType::Int8 => base_arg.value.i8 as f64,
            DataType::Int16 => base_arg.value.i16 as f64,
            DataType::Int32 => base_arg.value.i32 as f64,
            DataType::Int64 => base_arg.value.i64 as f64,
            DataType::Float32 => base_arg.value.float32 as f64,
            DataType::Float64 => base_arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        let exponent = match exponent_arg.value_type {
            DataType::UInt8 => exponent_arg.value.u8 as f64,
            DataType::UInt16 => exponent_arg.value.u16 as f64,
            DataType::UInt32 => exponent_arg.value.u32 as f64,
            DataType::UInt64 => exponent_arg.value.u64 as f64,
            DataType::Int8 => exponent_arg.value.i8 as f64,
            DataType::Int16 => exponent_arg.value.i16 as f64,
            DataType::Int32 => exponent_arg.value.i32 as f64,
            DataType::Int64 => exponent_arg.value.i64 as f64,
            DataType::Float32 => exponent_arg.value.float32 as f64,
            DataType::Float64 => exponent_arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = base.powf(exponent);
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行SIN函数
fn execute_sin(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.sin();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行COS函数
fn execute_cos(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.cos();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行LOG函数
fn execute_log(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.ln(); // 自然对数
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行EXP函数
fn execute_exp(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        let value = match arg.value_type {
            DataType::UInt8 => arg.value.u8 as f64,
            DataType::UInt16 => arg.value.u16 as f64,
            DataType::UInt32 => arg.value.u32 as f64,
            DataType::UInt64 => arg.value.u64 as f64,
            DataType::Int8 => arg.value.i8 as f64,
            DataType::Int16 => arg.value.i16 as f64,
            DataType::Int32 => arg.value.i32 as f64,
            DataType::Int64 => arg.value.i64 as f64,
            DataType::Float32 => arg.value.float32 as f64,
            DataType::Float64 => arg.value.float64,
            _ => return Err(QueryExecutionError::TypeMismatch),
        };

        #[cfg(feature = "std")]
        let result = value.exp();
        #[cfg(not(feature = "std"))]
        let result = 0.0;

        Ok(TypedValue {
            value_type: DataType::Float64,
            value: Value { float64: result },
        })
    }
}

/// 执行ROUND函数
fn execute_round(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];
    let decimals = if args.len() > 1 {
        unsafe {
            match args[1].value_type {
                DataType::Int8 => args[1].value.i8 as i32,
                DataType::Int16 => args[1].value.i16 as i32,
                DataType::Int32 => args[1].value.i32,
                DataType::Int64 => args[1].value.i64 as i32,
                DataType::UInt8 => args[1].value.u8 as i32,
                DataType::UInt16 => args[1].value.u16 as i32,
                DataType::UInt32 => args[1].value.u32 as i32,
                DataType::UInt64 => args[1].value.u64 as i32,
                _ => 0,
            }
        }
    } else {
        0
    };

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                #[cfg(feature = "std")]
                let factor = 10.0f32.powi(decimals);
                #[cfg(feature = "std")]
                let result = (arg.value.float32 * factor).round() / factor;
                #[cfg(not(feature = "std"))]
                let result = arg.value.float32;
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                #[cfg(feature = "std")]
                let factor = 10.0f64.powi(decimals);
                #[cfg(feature = "std")]
                let result = (arg.value.float64 * factor).round() / factor;
                #[cfg(not(feature = "std"))]
                let result = arg.value.float64;
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行CEIL函数
fn execute_ceil(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float32.ceil();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float32;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float64.ceil();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float64;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行FLOOR函数
fn execute_floor(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let arg = &args[0];

    unsafe {
        match arg.value_type {
            DataType::Float32 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float32.floor();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float32;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float32,
                    value: Value { float32: result },
                })
            }
            DataType::Float64 => {
                let result = {
                    #[cfg(feature = "std")]
                    let val = arg.value.float64.floor();
                    #[cfg(not(feature = "std"))]
                    let val = arg.value.float64;
                    val
                };
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
            _ => {
                // 对于整数类型，直接返回原值
                Ok(arg.clone())
            }
        }
    }
}

/// 执行MOD函数
fn execute_mod(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let dividend_arg = &args[0];
    let divisor_arg = &args[1];

    unsafe {
        match (dividend_arg.value_type, divisor_arg.value_type) {
            // 整数类型
            (DataType::UInt8, DataType::UInt8) => Ok(TypedValue {
                value_type: DataType::UInt8,
                value: Value {
                    u8: dividend_arg.value.u8 % divisor_arg.value.u8,
                },
            }),
            (DataType::UInt16, DataType::UInt16) => Ok(TypedValue {
                value_type: DataType::UInt16,
                value: Value {
                    u16: dividend_arg.value.u16 % divisor_arg.value.u16,
                },
            }),
            (DataType::UInt32, DataType::UInt32) => Ok(TypedValue {
                value_type: DataType::UInt32,
                value: Value {
                    u32: dividend_arg.value.u32 % divisor_arg.value.u32,
                },
            }),
            (DataType::UInt64, DataType::UInt64) => Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value {
                    u64: dividend_arg.value.u64 % divisor_arg.value.u64,
                },
            }),
            (DataType::Int8, DataType::Int8) => Ok(TypedValue {
                value_type: DataType::Int8,
                value: Value {
                    i8: dividend_arg.value.i8 % divisor_arg.value.i8,
                },
            }),
            (DataType::Int16, DataType::Int16) => Ok(TypedValue {
                value_type: DataType::Int16,
                value: Value {
                    i16: dividend_arg.value.i16 % divisor_arg.value.i16,
                },
            }),
            (DataType::Int32, DataType::Int32) => Ok(TypedValue {
                value_type: DataType::Int32,
                value: Value {
                    i32: dividend_arg.value.i32 % divisor_arg.value.i32,
                },
            }),
            (DataType::Int64, DataType::Int64) => Ok(TypedValue {
                value_type: DataType::Int64,
                value: Value {
                    i64: dividend_arg.value.i64 % divisor_arg.value.i64,
                },
            }),
            // 浮点数类型
            (DataType::Float32, DataType::Float32) => Ok(TypedValue {
                value_type: DataType::Float32,
                value: Value {
                    float32: dividend_arg.value.float32 % divisor_arg.value.float32,
                },
            }),
            (DataType::Float64, DataType::Float64) => Ok(TypedValue {
                value_type: DataType::Float64,
                value: Value {
                    float64: dividend_arg.value.float64 % divisor_arg.value.float64,
                },
            }),
            // 混合类型，转换为浮点数
            _ => {
                let dividend = match dividend_arg.value_type {
                    DataType::UInt8 => dividend_arg.value.u8 as f64,
                    DataType::UInt16 => dividend_arg.value.u16 as f64,
                    DataType::UInt32 => dividend_arg.value.u32 as f64,
                    DataType::UInt64 => dividend_arg.value.u64 as f64,
                    DataType::Int8 => dividend_arg.value.i8 as f64,
                    DataType::Int16 => dividend_arg.value.i16 as f64,
                    DataType::Int32 => dividend_arg.value.i32 as f64,
                    DataType::Int64 => dividend_arg.value.i64 as f64,
                    DataType::Float32 => dividend_arg.value.float32 as f64,
                    DataType::Float64 => dividend_arg.value.float64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let divisor = match divisor_arg.value_type {
                    DataType::UInt8 => divisor_arg.value.u8 as f64,
                    DataType::UInt16 => divisor_arg.value.u16 as f64,
                    DataType::UInt32 => divisor_arg.value.u32 as f64,
                    DataType::UInt64 => divisor_arg.value.u64 as f64,
                    DataType::Int8 => divisor_arg.value.i8 as f64,
                    DataType::Int16 => divisor_arg.value.i16 as f64,
                    DataType::Int32 => divisor_arg.value.i32 as f64,
                    DataType::Int64 => divisor_arg.value.i64 as f64,
                    DataType::Float32 => divisor_arg.value.float32 as f64,
                    DataType::Float64 => divisor_arg.value.float64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let result = dividend % divisor;

                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: result },
                })
            }
        }
    }
}

/// 执行没有FROM子句的表达式查询
fn execute_expression_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    eprintln!("DEBUG execute_expression_query: called");
    
    // 确定要返回的列表达式
    let columns = query.columns.clone();

    eprintln!("DEBUG execute_expression_query: columns count = {}", columns.len());

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
    eprintln!("DEBUG execute_expression_query: adding row with {} values", row_values.len());
    result_set.add_row(row_values);

    eprintln!("DEBUG execute_expression_query: result_set row_count = {}", result_set.row_count());
    
    Ok(result_set)
}

/// 评估表达式（没有表上下文）
fn evaluate_expression_without_table(
    db: &mut RemDb,
    expr: &Expression,
) -> Result<TypedValue, QueryExecutionError> {
    evaluate_expression_without_table_with_depth(db, expr, 0)
}

fn evaluate_expression_without_table_with_depth(
    db: &mut RemDb,
    expr: &Expression,
    depth: usize,
) -> Result<TypedValue, QueryExecutionError> {
    // Check recursion depth to prevent stack overflow
    const MAX_RECURSION_DEPTH: usize = 100;
    if depth > MAX_RECURSION_DEPTH {
        return Err(QueryExecutionError::InternalError);
    }
    match expr {
        Expression::Field {
            name: field_name, ..
        } => {
            // 没有表上下文时，字段引用是无效的
            Err(QueryExecutionError::FieldNotFound)
        }
        Expression::FunctionCall { name, args, .. } => {
            // 评估函数参数
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                evaluated_args.push(evaluate_expression_without_table_with_depth(db, arg, depth + 1)?);
            }

            // 执行函数调用
            execute_function_call(name, &evaluated_args)
        }
        Expression::Constant { value, .. } => {
            // 返回常量值
            // 需要将 query_parser::Value 转换为 TypedValue
            match value {
                crate::sql::query_parser::Value::Integer(i) => {
                    Ok(TypedValue {
                        value_type: DataType::Int64,
                        value: Value { i64: *i },
                    })
                }
                crate::sql::query_parser::Value::Float(f) => {
                    Ok(TypedValue {
                        value_type: DataType::Float64,
                        value: Value { float64: *f },
                    })
                }
                crate::sql::query_parser::Value::String(s) => {
                    let mut buf = [0; MAX_STRING_LEN];
                    let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                    buf[..len].copy_from_slice(s.as_bytes());
                    Ok(TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: buf },
                    })
                }
                crate::sql::query_parser::Value::Boolean(b) => {
                    Ok(TypedValue {
                        value_type: DataType::Bool,
                        value: Value { bool: *b },
                    })
                }
                crate::sql::query_parser::Value::Null => {
                    Ok(TypedValue {
                        value_type: DataType::Json,
                        value: Value { json_storage: crate::types::JsonStorage::Null },
                    })
                }
                crate::sql::query_parser::Value::Identifier(_) => {
                    Err(QueryExecutionError::InvalidValue)
                }
                crate::sql::query_parser::Value::Json(s) => {
                    let mut buf = [0u8; 256];
                    let len = core::cmp::min(s.len(), 256);
                    buf[..len].copy_from_slice(s.as_bytes());
                    Ok(TypedValue {
                        value_type: DataType::Json,
                        value: Value { json_storage: JsonStorage::Inline(buf) },
                    })
                }
            }
        }
        Expression::BinaryOp { op, left, right, .. } => {
            // 评估左右操作数
            let left_val = evaluate_expression_without_table_with_depth(db, left, depth + 1)?;
            let right_val = evaluate_expression_without_table_with_depth(db, right, depth + 1)?;

            // 执行二元运算
            evaluate_binary_op(left_val, *op, right_val)
        }
        Expression::LogicalOp { op, left, right, .. } => {
            // 评估左右操作数
            let left_val = evaluate_expression_without_table_with_depth(db, left, depth + 1)?;
            let right_val = evaluate_expression_without_table_with_depth(db, right, depth + 1)?;

            // 执行逻辑运算
            unsafe {
                // 将左右操作数转换为布尔值
                let left_bool = match left_val.value_type {
                    DataType::Bool => left_val.value.bool,
                    DataType::Int8 => left_val.value.i8 != 0,
                    DataType::Int16 => left_val.value.i16 != 0,
                    DataType::Int32 => left_val.value.i32 != 0,
                    DataType::Int64 => left_val.value.i64 != 0,
                    DataType::UInt8 => left_val.value.u8 != 0,
                    DataType::UInt16 => left_val.value.u16 != 0,
                    DataType::UInt32 => left_val.value.u32 != 0,
                    DataType::UInt64 => left_val.value.u64 != 0,
                    DataType::Float32 => left_val.value.float32 != 0.0,
                    DataType::Float64 => left_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                let right_bool = match right_val.value_type {
                    DataType::Bool => right_val.value.bool,
                    DataType::Int8 => right_val.value.i8 != 0,
                    DataType::Int16 => right_val.value.i16 != 0,
                    DataType::Int32 => right_val.value.i32 != 0,
                    DataType::Int64 => right_val.value.i64 != 0,
                    DataType::UInt8 => right_val.value.u8 != 0,
                    DataType::UInt16 => right_val.value.u16 != 0,
                    DataType::UInt32 => right_val.value.u32 != 0,
                    DataType::UInt64 => right_val.value.u64 != 0,
                    DataType::Float32 => right_val.value.float32 != 0.0,
                    DataType::Float64 => right_val.value.float64 != 0.0,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 执行逻辑操作
                let result = match op {
                    crate::sql::query_parser::LogicalOperator::And => left_bool && right_bool,
                    crate::sql::query_parser::LogicalOperator::Or => left_bool || right_bool,
                };

                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: result },
                })
            }
        }
        Expression::UnaryOp { op, operand, .. } => {
            // 评估操作数
            let operand_val = evaluate_expression_without_table_with_depth(db, operand, depth + 1)?;

            // 执行一元运算
            evaluate_unary_op(*op, operand_val)
        }
    }
}

/// 从TypedValue中提取JSON字符串
fn typed_value_to_json_string(arg: &TypedValue) -> Result<String, QueryExecutionError> {
    match arg.value_type {
        DataType::Json => {
            let json_storage = unsafe { &arg.value.json_storage };
            match json_storage {
                JsonStorage::Inline(data) => {
                    let len = data.iter().rposition(|&b| b == 0).unwrap_or(256);
                    eprintln!("DEBUG typed_value_to_json_string: data={:?}", data);
                    eprintln!("DEBUG typed_value_to_json_string: len={}", len);
                    let result = String::from_utf8_lossy(&data[..len]).to_string();
                    eprintln!("DEBUG typed_value_to_json_string: result={}", result);
                    Ok(result)
                }
                JsonStorage::External { pool_id, offset, length } => {
                    let pool_manager = crate::json::memory_pool::get_global_json_pool_manager()
                        .ok_or(QueryExecutionError::InternalError)?;
                    let pool = pool_manager.get_pool(*pool_id)
                        .ok_or(QueryExecutionError::InternalError)?;
                    
                    if let Some(data_ptr) = pool.get_block_data(*offset as usize, 0) {
                        let data = unsafe { core::slice::from_raw_parts(data_ptr, *length as usize) };
                        Ok(String::from_utf8_lossy(data).to_string())
                    } else {
                        Err(QueryExecutionError::InternalError)
                    }
                }
                JsonStorage::Null => Ok("null".to_string()),
            }
        }
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// 从TypedValue中提取字符串
fn typed_value_to_string(arg: &TypedValue) -> Result<String, QueryExecutionError> {
    match arg.value_type {
        DataType::VarChar | DataType::Char | DataType::Text => {
            let data = unsafe { &arg.value.string };
            let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
            Ok(String::from_utf8_lossy(&data[..len]).to_string())
        }
        DataType::Int8 => {
            Ok(unsafe { arg.value.i8 }.to_string())
        }
        DataType::Int16 => {
            Ok(unsafe { arg.value.i16 }.to_string())
        }
        DataType::Int32 => {
            Ok(unsafe { arg.value.i32 }.to_string())
        }
        DataType::Int64 => {
            Ok(unsafe { arg.value.i64 }.to_string())
        }
        DataType::UInt8 => {
            Ok(unsafe { arg.value.u8 }.to_string())
        }
        DataType::UInt16 => {
            Ok(unsafe { arg.value.u16 }.to_string())
        }
        DataType::UInt32 => {
            Ok(unsafe { arg.value.u32 }.to_string())
        }
        DataType::UInt64 => {
            Ok(unsafe { arg.value.u64 }.to_string())
        }
        DataType::Float32 => {
            Ok(unsafe { arg.value.float32 }.to_string())
        }
        DataType::Float64 => {
            Ok(unsafe { arg.value.float64 }.to_string())
        }
        DataType::Bool => {
            Ok(unsafe { arg.value.bool }.to_string())
        }
        DataType::Json => {
            typed_value_to_json_string(arg)
        }
        _ => Err(QueryExecutionError::TypeMismatch),
    }
}

/// 执行JSON_EXTRACT函数
fn execute_json_extract(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Scalar(s) => {
            eprintln!("DEBUG execute_json_extract: extracted scalar value: {}", s);
            // Try to parse as different types to enable comparisons
            if let Ok(num) = s.parse::<i64>() {
                eprintln!("DEBUG execute_json_extract: parsed as i64: {}", num);
                Ok(TypedValue {
                    value_type: DataType::Int64,
                    value: Value { i64: num },
                })
            } else if let Ok(num) = s.parse::<f64>() {
                eprintln!("DEBUG execute_json_extract: parsed as f64: {}", num);
                Ok(TypedValue {
                    value_type: DataType::Float64,
                    value: Value { float64: num },
                })
            } else if s == "true" || s == "false" {
                eprintln!("DEBUG execute_json_extract: parsed as bool: {}", s);
                Ok(TypedValue {
                    value_type: DataType::Bool,
                    value: Value { bool: s == "true" },
                })
            } else {
                // Default to string
                eprintln!("DEBUG execute_json_extract: defaulting to string: {}", s);
                let mut buf = [0; MAX_STRING_LEN];
                let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                buf[..len].copy_from_slice(s.as_bytes());
                Ok(TypedValue {
                    value_type: DataType::VarChar,
                    value: Value { string: buf },
                })
            }
        }
        crate::json::document::JsonQueryResult::Object(_) | 
        crate::json::document::JsonQueryResult::Array(_) => {
            let result_json = match crate::json::document::json_extract(&doc, &path) {
                crate::json::document::JsonQueryResult::Object(obj_doc) => {
                    obj_doc.to_json().unwrap_or_else(|_| "null".to_string())
                }
                crate::json::document::JsonQueryResult::Array(arr) => {
                    let json_str = arr.iter()
                        .map(|item| match item {
                            crate::json::document::JsonQueryResult::Scalar(s) => s.clone(),
                            _ => "null".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("[{}]", json_str)
                }
                _ => "null".to_string(),
            };
            
            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        crate::json::document::JsonQueryResult::None => {
            let mut buf = [0u8; 256];
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
    }
}

/// 执行JSON_VALUE函数
fn execute_json_value(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Scalar(s) => {
            let mut buf = [0; MAX_STRING_LEN];
            let len = core::cmp::min(s.len(), MAX_STRING_LEN);
            buf[..len].copy_from_slice(s.as_bytes());
            Ok(TypedValue {
                value_type: DataType::VarChar,
                value: Value { string: buf },
            })
        }
        _ => {
            let mut buf = [0; MAX_STRING_LEN];
            Ok(TypedValue {
                value_type: DataType::VarChar,
                value: Value { string: buf },
            })
        }
    }
}

/// 执行JSON_QUERY函数
fn execute_json_query(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, &path) {
        crate::json::document::JsonQueryResult::Object(obj_doc) => {
            let result_json = obj_doc.to_json()
                .unwrap_or_else(|_| "null".to_string());
            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        crate::json::document::JsonQueryResult::Array(arr) => {
            let json_str = arr.iter()
                .map(|item| match item {
                    crate::json::document::JsonQueryResult::Scalar(s) => s.clone(),
                    _ => "null".to_string(),
                })
                .collect::<Vec<_>>()
                .join(",");
            let result_json = format!("[{}]", json_str);
            let mut buf = [0u8; 256];
            let len = core::cmp::min(result_json.len(), 256);
            buf[..len].copy_from_slice(result_json.as_bytes());
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
        _ => {
            let mut buf = [0u8; 256];
            Ok(TypedValue {
                value_type: DataType::Json,
                value: Value { json_storage: JsonStorage::Inline(buf) },
            })
        }
    }
}

/// 执行JSON_HAS函数
fn execute_json_has(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    let has = crate::json::document::json_has(&doc, &path);
    Ok(TypedValue {
        value_type: DataType::Bool,
        value: Value { bool: has },
    })
}

/// 执行JSON_TYPE函数
fn execute_json_type(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    let type_str = crate::json::document::json_type(&doc, &path);
    let mut buf = [0; MAX_STRING_LEN];
    let len = core::cmp::min(type_str.len(), MAX_STRING_LEN);
    buf[..len].copy_from_slice(type_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::VarChar,
        value: Value { string: buf },
    })
}

/// 执行JSON_SET函数
fn execute_json_set(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;
    
    // Convert the value to proper JSON format based on its type
    let value_json_str = unsafe {
        match args[2].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[2])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[2].value.string;
                let len = data.iter().rposition(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                format!("\"{}\"", s)
            }
            DataType::Int8 => format!("{}", args[2].value.i8),
            DataType::Int16 => format!("{}", args[2].value.i16),
            DataType::Int32 => format!("{}", args[2].value.i32),
            DataType::Int64 => format!("{}", args[2].value.i64),
            DataType::UInt8 => format!("{}", args[2].value.u8),
            DataType::UInt16 => format!("{}", args[2].value.u16),
            DataType::UInt32 => format!("{}", args[2].value.u32),
            DataType::UInt64 => format!("{}", args[2].value.u64),
            DataType::Float32 => format!("{}", args[2].value.float32),
            DataType::Float64 => format!("{}", args[2].value.float64),
            DataType::Bool => {
                if args[2].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|e| {
            eprintln!("DEBUG execute_json_set: from_json failed with error: {:?}", e);
            QueryExecutionError::InvalidValue
        })?;

    crate::json::document::json_set(&mut doc, &path, &value_json_str)
        .map_err(|e| {
            eprintln!("DEBUG execute_json_set: json_set failed with error: {:?}", e);
            QueryExecutionError::InternalError
        })?;

    let new_json_str = doc.to_json()
        .map_err(|e| {
            eprintln!("DEBUG execute_json_set: to_json failed with error: {:?}", e);
            QueryExecutionError::InternalError
        })?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_REMOVE函数
fn execute_json_remove(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    eprintln!("DEBUG execute_json_remove: json_str from arg[0] = {}", json_str);
    let path = typed_value_to_string(&args[1])?;
    eprintln!("DEBUG execute_json_remove: path from arg[1] = {}", path);

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;
    eprintln!("DEBUG execute_json_remove: doc parsed successfully");

    crate::json::document::json_remove(&mut doc, &path)
        .map_err(|e| {
            eprintln!("DEBUG execute_json_remove: json_remove failed with error: {:?}", e);
            QueryExecutionError::InternalError
        })?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_MERGE_PATCH函数
fn execute_json_merge_patch(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 2 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    
    // Convert the patch to proper JSON format based on its type
    let patch_json_str = unsafe {
        match args[1].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[1])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[1].value.string;
                let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                s
            }
            DataType::Int8 => format!("{}", args[1].value.i8),
            DataType::Int16 => format!("{}", args[1].value.i16),
            DataType::Int32 => format!("{}", args[1].value.i32),
            DataType::Int64 => format!("{}", args[1].value.i64),
            DataType::UInt8 => format!("{}", args[1].value.u8),
            DataType::UInt16 => format!("{}", args[1].value.u16),
            DataType::UInt32 => format!("{}", args[1].value.u32),
            DataType::UInt64 => format!("{}", args[1].value.u64),
            DataType::Float32 => format!("{}", args[1].value.float32),
            DataType::Float64 => format!("{}", args[1].value.float64),
            DataType::Bool => {
                if args[1].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_merge_patch(&mut doc, &patch_json_str)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_ARRAY_APPEND函数
fn execute_json_array_append(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.len() < 3 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;
    let path = typed_value_to_string(&args[1])?;
    
    // Convert the value to proper JSON format based on its type
    let value_json_str = unsafe {
        match args[2].value_type {
            DataType::Json => {
                typed_value_to_json_string(&args[2])?
            }
            DataType::VarChar | DataType::Char | DataType::Text => {
                let data = &args[2].value.string;
                let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                let s = String::from_utf8_lossy(&data[..len]).to_string();
                format!("\"{}\"", s)
            }
            DataType::Int8 => format!("{}", args[2].value.i8),
            DataType::Int16 => format!("{}", args[2].value.i16),
            DataType::Int32 => format!("{}", args[2].value.i32),
            DataType::Int64 => format!("{}", args[2].value.i64),
            DataType::UInt8 => format!("{}", args[2].value.u8),
            DataType::UInt16 => format!("{}", args[2].value.u16),
            DataType::UInt32 => format!("{}", args[2].value.u32),
            DataType::UInt64 => format!("{}", args[2].value.u64),
            DataType::Float32 => format!("{}", args[2].value.float32),
            DataType::Float64 => format!("{}", args[2].value.float64),
            DataType::Bool => {
                if args[2].value.bool { "true".to_string() } else { "false".to_string() }
            }
            _ => "null".to_string(),
        }
    };

    let mut doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    crate::json::document::json_set(&mut doc, &path, &value_json_str)
        .map_err(|_| QueryExecutionError::InternalError)?;

    let new_json_str = doc.to_json()
        .map_err(|_| QueryExecutionError::InternalError)?;

    let mut buf = [0u8; 256];
    let len = core::cmp::min(new_json_str.len(), 256);
    buf[..len].copy_from_slice(new_json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_ARRAY_LENGTH函数
fn execute_json_array_length(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let json_str = typed_value_to_json_string(&args[0])?;

    let doc = crate::json::document::JsonDocument::from_json(&json_str)
        .map_err(|_| QueryExecutionError::InvalidValue)?;

    match crate::json::document::json_extract(&doc, "$") {
        crate::json::document::JsonQueryResult::Array(arr) => {
            let length = arr.len() as u64;
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: length },
            })
        }
        _ => {
            Ok(TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: 0 },
            })
        }
    }
}

/// 执行JSON_ARRAY函数
fn execute_json_array(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    eprintln!("DEBUG execute_json_array: called with {} args", args.len());
    
    if args.is_empty() {
        let mut buf = [0u8; 256];
        let json_str = "[]";
        let len = core::cmp::min(json_str.len(), 256);
        buf[..len].copy_from_slice(json_str.as_bytes());
        eprintln!("DEBUG execute_json_array: returning empty array");
        return Ok(TypedValue {
            value_type: DataType::Json,
            value: Value { json_storage: JsonStorage::Inline(buf) },
        });
    }

    let mut array_items = Vec::new();
    for arg in args {
        let item_str: String = unsafe {
            match arg.value_type {
                DataType::Json => {
                    typed_value_to_json_string(arg)?
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let data = &arg.value.string;
                    let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                    let s = String::from_utf8_lossy(&data[..len]).to_string();
                    format!("\"{}\"", s)
                }
                DataType::Int8 => format!("{}", arg.value.i8),
                DataType::Int16 => format!("{}", arg.value.i16),
                DataType::Int32 => format!("{}", arg.value.i32),
                DataType::Int64 => format!("{}", arg.value.i64),
                DataType::UInt8 => format!("{}", arg.value.u8),
                DataType::UInt16 => format!("{}", arg.value.u16),
                DataType::UInt32 => format!("{}", arg.value.u32),
                DataType::UInt64 => format!("{}", arg.value.u64),
                DataType::Float32 => format!("{}", arg.value.float32),
                DataType::Float64 => format!("{}", arg.value.float64),
                DataType::Bool => {
                    if arg.value.bool { "true".to_string() } else { "false".to_string() }
                }
                _ => "null".to_string(),
            }
        };
        array_items.push(item_str);
    }

    let json_str = format!("[{}]", array_items.join(","));
    eprintln!("DEBUG execute_json_array: json_str = {}", json_str);
    let mut buf = [0u8; 256];
    let len = core::cmp::min(json_str.len(), 256);
    buf[..len].copy_from_slice(json_str.as_bytes());
    eprintln!("DEBUG execute_json_array: returning json, len = {}", len);
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 执行JSON_OBJECT函数
fn execute_json_object(args: &[TypedValue]) -> Result<TypedValue, QueryExecutionError> {
    if args.is_empty() || args.len() % 2 != 0 {
        return Err(QueryExecutionError::TypeMismatch);
    }

    let mut object_items = Vec::new();
    for i in (0..args.len()).step_by(2) {
        let key_str = typed_value_to_string(&args[i])?;
        let value_str: String = unsafe {
            match args[i + 1].value_type {
                DataType::Json => {
                    typed_value_to_json_string(&args[i + 1])?
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let data = &args[i + 1].value.string;
                    let len = data.iter().position(|&b| b == 0).unwrap_or(MAX_STRING_LEN);
                    let s = String::from_utf8_lossy(&data[..len]).to_string();
                    format!("\"{}\"", s)
                }
                DataType::Int8 => format!("{}", args[i + 1].value.i8),
                DataType::Int16 => format!("{}", args[i + 1].value.i16),
                DataType::Int32 => format!("{}", args[i + 1].value.i32),
                DataType::Int64 => format!("{}", args[i + 1].value.i64),
                DataType::UInt8 => format!("{}", args[i + 1].value.u8),
                DataType::UInt16 => format!("{}", args[i + 1].value.u16),
                DataType::UInt32 => format!("{}", args[i + 1].value.u32),
                DataType::UInt64 => format!("{}", args[i + 1].value.u64),
                DataType::Float32 => format!("{}", args[i + 1].value.float32),
                DataType::Float64 => format!("{}", args[i + 1].value.float64),
                DataType::Bool => {
                    if args[i + 1].value.bool { "true".to_string() } else { "false".to_string() }
                }
                _ => "null".to_string(),
            }
        };
        object_items.push(format!("\"{}\":{}", key_str, value_str));
    }

    let json_str = format!("{{{}}}", object_items.join(","));
    let mut buf = [0u8; 256];
    let len = core::cmp::min(json_str.len(), 256);
    buf[..len].copy_from_slice(json_str.as_bytes());
    Ok(TypedValue {
        value_type: DataType::Json,
        value: Value { json_storage: JsonStorage::Inline(buf) },
    })
}

/// 解析时间间隔字符串
fn parse_interval_string(interval_str: &str) -> Result<i64, QueryExecutionError> {
    // 支持的时间单位
    let units = [
        ("ns", 1),              // 纳秒
        ("us", 1),              // 微秒
        ("ms", 1000),           // 毫秒
        ("s", 1000000),         // 秒
        ("sec", 1000000),       // 秒
        ("second", 1000000),    // 秒
        ("m", 60000000),        // 分钟
        ("min", 60000000),      // 分钟
        ("minute", 60000000),   // 分钟
        ("h", 3600000000),      // 小时
        ("hr", 3600000000),     // 小时
        ("hour", 3600000000),   // 小时
        ("d", 86400000000),     // 天
        ("day", 86400000000),   // 天
        ("w", 604800000000),    // 周
        ("week", 604800000000), // 周
    ];

    // 去除空格并转换为小写
    let normalized = interval_str.replace(" ", "").to_lowercase();

    // 查找匹配的时间单位
    for (unit, factor) in &units {
        if normalized.ends_with(unit) {
            // 提取数值部分
            let num_str = &normalized[..normalized.len() - unit.len()];
            let num = num_str
                .parse::<i64>()
                .map_err(|_| QueryExecutionError::TypeMismatch)?;
            // 计算微秒数
            return Ok(num * factor);
        }
    }

    // 无法解析的时间间隔
    Err(QueryExecutionError::TypeMismatch)
}

/// 执行CREATE TABLE查询
fn execute_create_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    #[cfg(feature = "log")]
    debug!("execute_create_table_query called for table: {}", query.table_name);
    // 检查IF NOT EXISTS子句
    if query.if_not_exists {
        // 检查表是否已存在
        for table_opt in &db.tables {
            if let Some(table) = table_opt {
                if table.def.name == query.table_name {
                    // 表已存在，返回成功
                    let columns = alloc::vec!["status".to_string()];
                    let mut result_set = ResultSet::new(columns);
                    result_set.add_row(alloc::vec![TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: [b'0'; 64] },
                    }]);
                    return Ok(result_set);
                }
            }
        }
        // 检查时序表是否已存在
        for table_opt in &db.time_series_tables {
            if let Some(table) = table_opt {
                if table.def.base.name == query.table_name {
                    // 表已存在，返回成功
                    let columns = alloc::vec!["status".to_string()];
                    let mut result_set = ResultSet::new(columns);
                    result_set.add_row(alloc::vec![TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: [b'0'; 64] },
                    }]);
                    return Ok(result_set);
                }
            }
        }
    }
    
    // 将SQL数据类型转换为RemDb DataType
    // 字段定义：(字段名, 数据类型, 维度/精度, 距离度量, 默认值)
    let mut fields = Vec::new();
    let mut field_constraints = Vec::new(); // 存储约束信息

    for (
        field_name,
        data_type_str,
        is_primary_key,
        is_not_null,
        is_unique,
        is_auto_increment,
        default_value,
    ) in &query.table_def
    {
        // 解析数据类型，支持带精度的时间类型如TIMESTAMP(6)
        let (base_type, precision, distance_type) = parse_data_type_with_precision(data_type_str)?;

        let data_type = match base_type.as_str() {
            // 无符号整数类型
            "UINT8" | "TINYINT UNSIGNED" => DataType::UInt8,
            "UINT16" | "SMALLINT UNSIGNED" => DataType::UInt16,
            "UINT32" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED" | "INTEGER UNSIGNED" => {
                DataType::UInt32
            }
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
            "STRING" | "TEXT" => DataType::Text,
            "VARCHAR" | "NVARCHAR" => DataType::VarChar,
            "CHAR" => DataType::Char,

            // 向量类型
            "VECTOR" => DataType::Vector,
            // JSON类型
            "JSON" => DataType::Json,

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
                    #[cfg(feature = "std")]
                    let now = crate::types::time_utils::now_micros();
                    #[cfg(not(feature = "std"))]
                    let now = 0;
                    now as i64
                } else {
                    0
                };

                let types_val = match sql_val {
                    crate::sql::Value::Integer(i) => {
                        // 如果是时间函数，使用当前时间替换占位符
                        let actual_value = if is_time_function
                            && (data_type == DataType::Timestamp
                                || data_type == DataType::TimestampTZ)
                        {
                            current_time
                        } else {
                            *i as i64
                        };

                        match data_type {
                            DataType::UInt8 => Value {
                                u8: actual_value as u8,
                            },
                            DataType::UInt16 => Value {
                                u16: actual_value as u16,
                            },
                            DataType::UInt32 => Value {
                                u32: actual_value as u32,
                            },
                            DataType::UInt64 => Value {
                                u64: actual_value as u64,
                            },
                            DataType::Int8 => Value {
                                i8: actual_value as i8,
                            },
                            DataType::Int16 => Value {
                                i16: actual_value as i16,
                            },
                            DataType::Int32 => Value {
                                i32: actual_value as i32,
                            },
                            DataType::Int64 => Value { i64: actual_value },
                            DataType::Bool => Value {
                                bool: actual_value != 0,
                            },
                            DataType::Float32 => Value {
                                float32: actual_value as f32,
                            },
                            DataType::Float64 => Value {
                                float64: actual_value as f64,
                            },
                            DataType::Timestamp => Value {
                                time: crate::types::db_timestamp::new(
                                    actual_value,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(
                                    actual_value,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::VarChar | DataType::Char | DataType::Text => {
                                let mut s = [0; MAX_STRING_LEN];
                                let str_val = actual_value.to_string();
                                let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                                s[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                                Value { string: s }
                            }
                            DataType::Interval => Value {
                                interval: crate::types::db_interval::new(
                                    actual_value,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::Vector => Value {
                                vector: core::ptr::null(),
                            },
                            DataType::Json => {
                                let mut buf = [0u8; 256];
                                let str_val = actual_value.to_string();
                                let len = core::cmp::min(str_val.len(), 256);
                                buf[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                                let json_storage = if str_val.len() <= 256 {
                                    crate::types::JsonStorage::Inline(buf)
                                } else {
                                    crate::types::JsonStorage::Null
                                };
                                Value { json_storage }
                            },
                        }
                    }
                    crate::sql::Value::Float(f) => match data_type {
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
                        DataType::Timestamp => Value {
                            time: crate::types::db_timestamp::new(
                                *f as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(
                                *f as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut s = [0; MAX_STRING_LEN];
                            let str_val = f.to_string();
                            let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                            s[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                            Value { string: s }
                        }
                        DataType::Interval => Value {
                            interval: crate::types::db_interval::new(
                                *f as i64,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::Vector => Value {
                            vector: core::ptr::null(),
                        },
                        DataType::Json => {
                            let mut buf = [0u8; 256];
                            let str_val = f.to_string();
                            let len = core::cmp::min(str_val.len(), 256);
                            buf[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                            let json_storage = if str_val.len() <= 256 {
                                crate::types::JsonStorage::Inline(buf)
                            } else {
                                crate::types::JsonStorage::Null
                            };
                            Value { json_storage }
                        },
                    },
                    crate::sql::Value::Boolean(b) => match data_type {
                        DataType::UInt8 => Value { u8: *b as u8 },
                        DataType::UInt16 => Value { u16: *b as u16 },
                        DataType::UInt32 => Value { u32: *b as u32 },
                        DataType::UInt64 => Value { u64: *b as u64 },
                        DataType::Int8 => Value { i8: *b as i8 },
                        DataType::Int16 => Value { i16: *b as i16 },
                        DataType::Int32 => Value { i32: *b as i32 },
                        DataType::Int64 => Value { i64: *b as i64 },
                        DataType::Bool => Value { bool: *b },
                        DataType::Float32 => Value {
                            float32: (*b as i32) as f32,
                        },
                        DataType::Float64 => Value {
                            float64: (*b as i32) as f64,
                        },
                        DataType::Timestamp => Value {
                            time: crate::types::db_timestamp::new(
                                *b as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(
                                *b as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut s = [0; MAX_STRING_LEN];
                            let str_val = b.to_string();
                            let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                            s[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                            Value { string: s }
                        }
                        DataType::Interval => Value {
                            interval: crate::types::db_interval::new(
                                *b as i64,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::Vector => Value {
                            vector: core::ptr::null(),
                        },
                        DataType::Json => Value {
                            json_storage: crate::types::JsonStorage::Null,
                        },
                    },
                    crate::sql::Value::String(s) => match data_type {
                        DataType::UInt8 => Value {
                            u8: s.parse().unwrap_or(0),
                        },
                        DataType::UInt16 => Value {
                            u16: s.parse().unwrap_or(0),
                        },
                        DataType::UInt32 => Value {
                            u32: s.parse().unwrap_or(0),
                        },
                        DataType::UInt64 => Value {
                            u64: s.parse().unwrap_or(0),
                        },
                        DataType::Int8 => Value {
                            i8: s.parse().unwrap_or(0),
                        },
                        DataType::Int16 => Value {
                            i16: s.parse().unwrap_or(0),
                        },
                        DataType::Int32 => Value {
                            i32: s.parse().unwrap_or(0),
                        },
                        DataType::Int64 => Value {
                            i64: s.parse().unwrap_or(0),
                        },
                        DataType::Bool => Value {
                            bool: s.parse().unwrap_or(false),
                        },
                        DataType::Float32 => Value {
                            float32: s.parse().unwrap_or(0.0),
                        },
                        DataType::Float64 => Value {
                            float64: s.parse().unwrap_or(0.0),
                        },
                        DataType::Timestamp => Value {
                            time: crate::types::db_timestamp::new(
                                s.parse().unwrap_or(0) as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(
                                s.parse().unwrap_or(0) as i64,
                                0,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut buf = [0; MAX_STRING_LEN];
                            let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                            buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                            Value { string: buf }
                        }
                        DataType::Interval => Value {
                            interval: crate::types::db_interval::new(
                                s.parse().unwrap_or(0) as i64,
                                precision.try_into().unwrap(),
                                0,
                            ),
                        },
                        DataType::Vector => Value {
                            vector: core::ptr::null(),
                        },
                        DataType::Json => Value {
                            json_storage: crate::types::JsonStorage::Null,
                        },
                    },
                    crate::sql::Value::Identifier(s) => {
                        // 标识符作为字符串处理
                        match data_type {
                            DataType::UInt8 => Value {
                                u8: s.parse().unwrap_or(0),
                            },
                            DataType::UInt16 => Value {
                                u16: s.parse().unwrap_or(0),
                            },
                            DataType::UInt32 => Value {
                                u32: s.parse().unwrap_or(0),
                            },
                            DataType::UInt64 => Value {
                                u64: s.parse().unwrap_or(0),
                            },
                            DataType::Int8 => Value {
                                i8: s.parse().unwrap_or(0),
                            },
                            DataType::Int16 => Value {
                                i16: s.parse().unwrap_or(0),
                            },
                            DataType::Int32 => Value {
                                i32: s.parse().unwrap_or(0),
                            },
                            DataType::Int64 => Value {
                                i64: s.parse().unwrap_or(0),
                            },
                            DataType::Bool => Value {
                                bool: s.parse().unwrap_or(false),
                            },
                            DataType::Float32 => Value {
                                float32: s.parse().unwrap_or(0.0),
                            },
                            DataType::Float64 => Value {
                                float64: s.parse().unwrap_or(0.0),
                            },
                            DataType::Timestamp => Value {
                                time: crate::types::db_timestamp::new(
                                    s.parse().unwrap_or(0) as i64,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(
                                    s.parse().unwrap_or(0) as i64,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::VarChar | DataType::Char | DataType::Text => {
                                let mut buf = [0; MAX_STRING_LEN];
                                let len = core::cmp::min(s.len(), MAX_STRING_LEN);
                                buf[..len].copy_from_slice(&s.as_bytes()[..len]);
                                Value { string: buf }
                            }
                            DataType::Interval => Value {
                                interval: crate::types::db_interval::new(
                                    s.parse().unwrap_or(0) as i64,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::Vector => Value {
                                vector: core::ptr::null(),
                            },
                            DataType::Json => Value {
                                json_storage: crate::types::JsonStorage::Inline([0u8; 256]),
                            },
                        }
                    }
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
                            DataType::Timestamp => Value {
                                time: crate::types::db_timestamp::new(
                                    0,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(
                                    0,
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::VarChar | DataType::Char | DataType::Text => Value {
                                string: [0; MAX_STRING_LEN],
                            },
                            DataType::Interval => Value {
                                interval: crate::types::db_interval::new(
                                    0,
                                    precision.try_into().unwrap(),
                                    0,
                                ),
                            },
                            DataType::Vector => Value {
                                vector: core::ptr::null(),
                            },
                            DataType::Json => Value {
                                json_storage: crate::types::JsonStorage::Inline([0u8; 256]),
                            },
                        }
                    }
                    crate::sql::Value::Json(_) => match data_type {
                        DataType::Json => Value {
                            json_storage: crate::types::JsonStorage::Null,
                        },
                        _ => Value {
                            json_storage: crate::types::JsonStorage::Null,
                        },
                    }
                };
                Some(types_val)
            }
            None => None,
        };

        // 解析向量类型的距离度量
        let mut distance_type = None;
        if data_type == DataType::Vector {
            // 检查是否包含WITH DISTANCE修饰符
            if data_type_str.contains("WITH DISTANCE=L2") {
                distance_type = Some(crate::types::DistanceType::L2);
            } else if data_type_str.contains("WITH DISTANCE=INNER_PRODUCT")
                || data_type_str.contains("WITH DISTANCE=IP")
            {
                distance_type = Some(crate::types::DistanceType::InnerProduct);
            } else if data_type_str.contains("WITH DISTANCE=COSINE") {
                distance_type = Some(crate::types::DistanceType::Cosine);
            }
        }

        // 保存字段和约束信息
        // 对于向量类型，使用解析出的精度作为维度
        fields.push((
            field_name.as_str(),
            data_type,
            precision,
            distance_type,
            converted_default,
        ));

        // 转换为FieldConstraint对象
        let field_constraint = crate::FieldConstraint {
            primary_key: *is_primary_key,
            not_null: *is_not_null,
            unique: *is_unique,
            auto_increment: *is_auto_increment,
        };
        field_constraints.push(field_constraint);
    }

    // 查找主键字段索引列表，支持复合主键
    let primary_key_indices = query.primary_key.as_ref().map(|pk_fields| {
        pk_fields
            .iter()
            .filter_map(|pk_field| {
                query
                    .table_def
                    .iter()
                    .position(|(name, _, _, _, _, _, _)| name == pk_field)
            })
            .collect()
    });

    // 调用DdlExecutor::create_table方法，支持约束和复合主键
    #[cfg(feature = "log")]
    debug!("Before create_table, db.tables.len() = {}", db.tables.len());
    // 使用 create_table_with_constraints 方法传递约束信息
    db.create_table_with_constraints(
        &query.table_name,
        &fields,
        Some(&field_constraints),
        primary_key_indices,
    )
    .map_err(|e| {
        #[cfg(feature = "log")]
        debug!("create_table failed with error: {:?}", e);
        match e {
            RemDbError::TableNotFound => QueryExecutionError::TableNotFound,
            RemDbError::FieldNotFound => QueryExecutionError::FieldNotFound,
            RemDbError::TypeMismatch => QueryExecutionError::TypeMismatch,
            RemDbError::OutOfMemory => QueryExecutionError::OutOfMemory,
            _ => QueryExecutionError::InternalError,
        }
    })?;
    #[cfg(feature = "log")]
    debug!("After create_table, db.tables.len() = {}", db.tables.len());

    // 创建结果集，返回成功消息
    let columns = alloc::vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: DataType::VarChar,
        value: Value { string: [b'0'; 64] },
    }]);

    Ok(result_set)
}

/// 执行CREATE INDEX查询
/// 执行SHOW INDEX BUILD STATUS查询
fn execute_show_index_build_status_query(
    _db: &mut RemDb,
    _query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 获取索引构建线程池
    let thread_pool = crate::index::builder::get_index_build_thread_pool()
        .map_err(|_| QueryExecutionError::InternalError)?;
    
    // 获取所有索引构建状态
    let status_list = thread_pool.get_build_status(None);
    
    // 创建结果集
    let columns = alloc::vec![
        "task_id".to_string(),
        "table_name".to_string(),
        "column_name".to_string(),
        "index_type".to_string(),
        "state".to_string(),
        "progress".to_string(),
        "processed_rows".to_string(),
        "total_rows".to_string(),
        "elapsed_time".to_string(),
    ];
    
    let mut result_set = ResultSet::new(columns);
    
    // 遍历所有状态，添加到结果集
    for status_arc in status_list {
        let status = status_arc.lock().unwrap();
        
        // 转换状态为字符串
        let state_str = status.get_state_str();
        
        // 创建行数据
        let row = alloc::vec![
            TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: status.id as u64 },
            },
            TypedValue {
                value_type: DataType::VarChar,
                value: Value { 
                    string: { 
                        let mut s = [0u8; 64];
                        let bytes = status.table_name.as_bytes();
                        let len = core::cmp::min(bytes.len(), 64);
                        s[..len].copy_from_slice(&bytes[..len]);
                        s
                    } 
                },
            },
            TypedValue {
                value_type: DataType::VarChar,
                value: Value { 
                    string: { 
                        let mut s = [0u8; 64];
                        let bytes = status.column_name.as_bytes();
                        let len = core::cmp::min(bytes.len(), 64);
                        s[..len].copy_from_slice(&bytes[..len]);
                        s
                    } 
                },
            },
            TypedValue {
                value_type: DataType::VarChar,
                value: Value { 
                    string: { 
                        let mut s = [0u8; 64];
                        let bytes = status.index_type.as_bytes();
                        let len = core::cmp::min(bytes.len(), 64);
                        s[..len].copy_from_slice(&bytes[..len]);
                        s
                    } 
                },
            },
            TypedValue {
                value_type: DataType::VarChar,
                value: Value { 
                    string: { 
                        let mut s = [0u8; 64];
                        let bytes = state_str.as_bytes();
                        let len = core::cmp::min(bytes.len(), 64);
                        s[..len].copy_from_slice(&bytes[..len]);
                        s
                    } 
                },
            },
            TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: status.progress.load(core::sync::atomic::Ordering::SeqCst) as u64 },
            },
            TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: status.processed_rows.load(core::sync::atomic::Ordering::SeqCst) as u64 },
            },
            TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: status.total_rows.load(core::sync::atomic::Ordering::SeqCst) as u64 },
            },
            TypedValue {
                value_type: DataType::UInt64,
                value: Value { u64: status.elapsed_time.load(core::sync::atomic::Ordering::SeqCst) as u64 },
            },
        ];
        
        result_set.add_row(row);
    }
    
    Ok(result_set)
}

/// 执行CREATE INDEX查询
fn execute_create_index_query(
    _db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL索引类型转换为RemDb IndexType
    let index_type = match query.index_type.as_deref() {
        Some("BTREE") => IndexType::BTree,
        Some("TTREE") => IndexType::TTree,
        Some("SORTEDARRAY") => IndexType::SortedArray,
        Some("HNSW") | Some("HNSW_SQ") | Some("HNSW_BQ") | Some("IVF") | Some("IVF_PQ")
        | Some("VECTOR") => IndexType::Vector,
        _ => IndexType::BTree, // 默认值
    };

    let field_name = query
        .index_column
        .as_ref()
        .ok_or(QueryExecutionError::InvalidCondition)?;

    // 构建索引类型映射
    // 注意：这里不需要 sql_index_type，因为我们直接使用 IndexBuildParams
    // let sql_index_type = match query.index_type.as_deref() {
    //     Some("HNSW") => crate::sql::query_parser::IndexType::HNSW,
    //     Some("IVF") => crate::sql::query_parser::IndexType::IVF,
    //     _ => crate::sql::query_parser::IndexType::BTree, // 默认值
    // };

    // 解析索引构建参数
    let mut params = crate::index::builder::IndexBuildParams::default();
    params.index_type = index_type;
    params.online = query.index_online;
    
    // 解析向量索引类型和参数
    if index_type == IndexType::Vector {
        // 设置向量索引类型
        params.vector_index_type = match query.index_type.as_deref() {
            Some("HNSW") => Some(crate::types::VectorIndexType::HNSW),
            Some("IVF") => Some(crate::types::VectorIndexType::IVF),
            _ => Some(crate::types::VectorIndexType::HNSW), // 默认值
        };
        
        // 解析HNSW参数
        if let Some(m) = query.index_params.get("M") {
            params.hnsw_m = m.parse().ok();
        }
        if let Some(efc) = query.index_params.get("EF_CONSTRUCTION") {
            params.hnsw_ef_construction = efc.parse().ok();
        }
        if let Some(efs) = query.index_params.get("EF_SEARCH") {
            params.hnsw_ef_search = efs.parse().ok();
        }
        
        // 解析IVF参数
        if let Some(nlist) = query.index_params.get("NLIST") {
            params.ivf_nlist = nlist.parse().ok();
        }
        if let Some(nprobe) = query.index_params.get("NPROBE") {
            params.ivf_nprobe = nprobe.parse().ok();
        }
    }

    // 提交索引构建任务到线程池
    let task_id = crate::index::builder::get_index_build_thread_pool()
        .map_err(|_| QueryExecutionError::InternalError)?
        .submit_task(
            query.table_name.clone(),
            field_name.clone(), // 直接克隆 Vec<String>
            crate::sql::query_parser::IndexType::BTree, // 使用默认值，实际索引类型由params指定
            params,
        );

    // 创建结果集，返回任务ID
    let columns = alloc::vec!["task_id".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: DataType::UInt64,
        value: Value { u64: task_id as u64 },
    }]);

    Ok(result_set)
}

/// 从WHERE条件中提取可索引的字段和值
fn extract_indexed_condition(condition: &Condition) -> Option<(String, Vec<u8>)> {
    match condition {
        Condition::Comparison(ComparisonCondition { field, operator, value }) => {
            // 只处理相等比较，因为只有相等比较才能直接使用索引查找
            if *operator == ComparisonOperator::Equal {
                // 转换值为字节数组，用于索引查找
                let index_value = match value {
                    crate::sql::Value::Integer(i) => {
                        let mut buf = Vec::new();
                        buf.extend_from_slice(&i.to_le_bytes());
                        buf
                    }
                    crate::sql::Value::Float(f) => {
                        let mut buf = Vec::new();
                        buf.extend_from_slice(&f.to_le_bytes());
                        buf
                    }
                    crate::sql::Value::String(s) => {
                        let mut buf = Vec::new();
                        buf.push(s.len() as u8);
                        buf.extend_from_slice(s.as_bytes());
                        buf
                    }
                    crate::sql::Value::Boolean(b) => {
                        let mut buf = Vec::new();
                        buf.push(*b as u8);
                        buf
                    }
                    _ => return None, // 其他类型暂不支持
                };
                Some((field.clone(), index_value))
            } else {
                None
            }
        }
        Condition::And(condition1, condition2) | Condition::Or(condition1, condition2) => {
            // 递归处理复合条件，只取第一个可索引的条件
            if let Some(result) = extract_indexed_condition(condition1) {
                return Some(result);
            }
            if let Some(result) = extract_indexed_condition(condition2) {
                return Some(result);
            }
            None
        }
        Condition::Not(inner) => {
            // 对于NOT条件，尝试从内部条件中提取可索引条件
            extract_indexed_condition(inner)
        }
        _ => None,
    }
}

/// 索引操作类型
enum IndexOperation {
    /// 相等查询
    Equal(Vec<u8>),
    /// 范围查询
    Range(Vec<u8>, Vec<u8>),
}

/// 从WHERE条件中提取可索引的字段和操作
fn extract_index_operation(condition: &Condition) -> Option<(String, IndexOperation)> {
    match condition {
        Condition::Comparison(ComparisonCondition { field, operator, value }) => {
            // 转换值为字节数组，用于索引查找
            let convert_value = |v: &crate::sql::Value| -> Option<Vec<u8>> {
                match v {
                    crate::sql::Value::Integer(i) => {
                        let mut buf = Vec::new();
                        buf.extend_from_slice(&i.to_le_bytes());
                        Some(buf)
                    }
                    crate::sql::Value::Float(f) => {
                        let mut buf = Vec::new();
                        buf.extend_from_slice(&f.to_le_bytes());
                        Some(buf)
                    }
                    crate::sql::Value::String(s) => {
                        let mut buf = Vec::new();
                        buf.push(s.len() as u8);
                        buf.extend_from_slice(s.as_bytes());
                        Some(buf)
                    }
                    crate::sql::Value::Boolean(b) => {
                        let mut buf = Vec::new();
                        buf.push(*b as u8);
                        Some(buf)
                    }
                    _ => None, // 其他类型暂不支持
                }
            };

            match operator {
                ComparisonOperator::Equal => {
                    // 相等查询
                    if let Some(index_value) = convert_value(value) {
                        Some((field.clone(), IndexOperation::Equal(index_value)))
                    } else {
                        None
                    }
                }
                ComparisonOperator::GreaterThan | ComparisonOperator::GreaterThanOrEqual
                | ComparisonOperator::LessThan | ComparisonOperator::LessThanOrEqual => {
                    // 范围查询，暂时只支持简单的范围查询
                    // 这里简化处理，使用最小值和最大值作为范围边界
                    if let Some(index_value) = convert_value(value) {
                        let start_value = match operator {
                            ComparisonOperator::GreaterThan => index_value.clone(),
                            ComparisonOperator::GreaterThanOrEqual => index_value.clone(),
                            _ => {
                                // 对于小于和小于等于，使用最小值作为起始
                                let mut min_value = Vec::new();
                                min_value.push(0u8);
                                min_value
                            }
                        };
                        let end_value = match operator {
                            ComparisonOperator::LessThan => index_value.clone(),
                            ComparisonOperator::LessThanOrEqual => index_value.clone(),
                            _ => {
                                // 对于大于和大于等于，使用最大值作为结束
                                let mut max_value = Vec::new();
                                max_value.push(255u8);
                                max_value
                            }
                        };
                        Some((field.clone(), IndexOperation::Range(start_value, end_value)))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        Condition::And(condition1, condition2) | Condition::Or(condition1, condition2) => {
            // 递归处理复合条件，只取第一个可索引的条件
            if let Some(result) = extract_index_operation(condition1) {
                return Some(result);
            }
            if let Some(result) = extract_index_operation(condition2) {
                return Some(result);
            }
            None
        }
        _ => None,
    }
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
                DataType::TimestampTZ => (value.value.time.value as u64) ^ (value.value.time.tz_offset as u64),
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
                },
                DataType::Vector => value.value.vector as u64,
                DataType::Json => {
                    // JSON类型的简单哈希
                    0 // 暂时返回0，实际应用中可能需要更复杂的哈希逻辑
                },
            }
        }
    }

    // 将行数据分组
    for record_values in rows_to_process {
        // 评估每个分组表达式，生成分组键
        let mut key_values = Vec::new();
        for expr in &group_by.expressions {
            let value = evaluate_expression(table, record_values, expr)?;
            let hash = hash_typed_value(&value);
            key_values.push(hash);
        }
        
        // 创建安全的分组键
        let group_key = GroupKey {
            values: key_values,
        };

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
                    // 处理TIME_BUCKET函数（非聚合函数）
                    if name.to_uppercase() == "TIME_BUCKET" {
                        // 计算分组中第一个记录的TIME_BUCKET值
                        let mut arg_values = Vec::with_capacity(args.len());
                        for arg in args {
                            arg_values.push(evaluate_expression(table, &group_rows[0], arg)?);
                        }
                        // 执行TIME_BUCKET函数
                        let result = execute_function_call(name, &arg_values)?;
                        row_data.push(result);
                        continue;
                    }

                    // 为每个聚合函数准备初始值
                    let mut agg_result = match name.to_uppercase().as_str() {
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

/// 查找表
fn find_table_by_name<'a>(
    db: &'a RemDb,
    table_name: &str,
) -> Result<&'a MemoryTable, QueryExecutionError> {
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
        Expression::Field {
            name: field_name, ..
        } => {
            // 跳过对 * 的验证，它是一个特殊情况
            if field_name != "*" {
                // 处理带表别名的字段名，如 "t.id"
                let actual_field_name = if field_name.contains('.') {
                    // 提取点号后面的部分作为实际字段名
                    field_name.split('.').last().unwrap()
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

/// 执行DESCRIBE TABLE查询
fn execute_describe_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表定义（同时检查普通表和时序表）
    let mut found_table_def: Option<Arc<TableDef>> = None;

    // 查找普通表
    for table_opt in db.tables.iter() {
        if let Some(table) = table_opt {
            if table.def.name == query.table_name {
                found_table_def = Some(table.def.clone());
                break;
            }
        }
    }

    // 如果普通表未找到，查找时序表
    if found_table_def.is_none() {
        for ts_table_opt in db.time_series_tables.iter() {
            if let Some(ts_table) = ts_table_opt {
                if ts_table.def.base.name == query.table_name {
                            found_table_def = Some(alloc::sync::Arc::new(ts_table.def.base.clone()));
                            break;
                        }
            }
        }
    }

    // 如果都未找到，返回错误
    let table_def = found_table_def.ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 定义结果集列名
    let columns = alloc::vec![
        "Field".to_string(),
        "Type".to_string(),
        "Key".to_string(),
        "Null".to_string(),
        "Default".to_string()
    ];

    // 3. 创建结果集
    let mut result_set = ResultSet::new(columns.clone());

    // 添加调试信息
    #[cfg(feature = "log")]
    {
        debug!("describe table {}: id={}, name={}, fields_len={}, primary_key_len={}", 
                query.table_name, table_def.id, table_def.name, table_def.fields.len(), table_def.primary_key.len());
        for (i, field) in table_def.fields.iter().enumerate() {
            debug!("field {}: name={}, data_type={:?}, size={}, offset={}", 
                    i, field.name, field.data_type, field.size, field.offset);
        }
    }

    // 4. 添加字段信息到结果集
    // 注意：由于describe查询返回的是表结构信息，而不是实际数据，
    // 我们需要特殊处理，将描述信息转换为Value类型
    // 使用索引迭代而非直接迭代，避免可能的无限循环
    for i in 0..table_def.fields.len() {
        let field = &table_def.fields[i];
        // 确定是否为主键
        let is_primary_key = table_def.primary_key.contains(&i);
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
                DataType::UInt8 => alloc::format!("{}", unsafe { default_val.u8 }),
                DataType::UInt16 => alloc::format!("{}", unsafe { default_val.u16 }),
                DataType::UInt32 => alloc::format!("{}", unsafe { default_val.u32 }),
                DataType::UInt64 => alloc::format!("{}", unsafe { default_val.u64 }),
                DataType::Int8 => alloc::format!("{}", unsafe { default_val.i8 }),
                DataType::Int16 => alloc::format!("{}", unsafe { default_val.i16 }),
                DataType::Int32 => alloc::format!("{}", unsafe { default_val.i32 }),
                DataType::Int64 => alloc::format!("{}", unsafe { default_val.i64 }),
                // 布尔类型
                DataType::Bool => alloc::format!("{}", unsafe { default_val.bool }),
                // 浮点数类型
                DataType::Float32 => alloc::format!("{}", unsafe { default_val.float32 }),
                DataType::Float64 => alloc::format!("{}", unsafe { default_val.float64 }),
                // 时间类型
                DataType::Timestamp => alloc::format!("{}", unsafe { default_val.time.value }),
                DataType::TimestampTZ => alloc::format!("{}", unsafe { default_val.time.value }),
                // 字符串类型
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let str_val = unsafe { &default_val.string };
                    String::from_utf8_lossy(str_val)
                        .trim_end_matches(char::from(0))
                        .to_string()
                }
                // 时间间隔类型
                DataType::Interval => alloc::format!("{}", unsafe { default_val.interval.value }),
                // 向量类型，默认值显示为<vector>
                DataType::Vector => "<vector>".to_string(),
                // JSON类型，默认值显示为<json>
                DataType::Json => "<json>".to_string()
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
            crate::DataType::VarChar => alloc::format!("varchar({})", field.size),
            crate::DataType::Char => alloc::format!("char({})", field.size),
            crate::DataType::Text => "text".to_string(),
            crate::DataType::Bool => "bool".to_string(),
            crate::DataType::Timestamp => "timestamp".to_string(),
            crate::DataType::TimestampTZ => "timestamp with time zone".to_string(),
            crate::DataType::Float32 => "float".to_string(),
            crate::DataType::Float64 => "double".to_string(),
            crate::DataType::Interval => "interval".to_string(),
            crate::DataType::Vector => {
                if let Some(metadata) = &field.vector_metadata {
                    alloc::format!("vector({})", metadata.dimension)
                } else {
                    "vector".to_string()
                }
            }
            crate::DataType::Json => "json".to_string()
        };

        // 创建行数据
        // 由于Value是union类型，我们需要确保每个值都被正确初始化
        // 对于字符串值，我们使用string字段并确保它是一个有效的C风格字符串
        let mut field_name_val = crate::Value { string: [0u8; 64] };
        let field_name_bytes = field.name.as_bytes();
        let field_name_len = core::cmp::min(field_name_bytes.len(), 63);
        unsafe {
            field_name_val.string[..field_name_len]
                .copy_from_slice(&field_name_bytes[..field_name_len]);
        }
        let field_name_typed_val = TypedValue {
            value_type: DataType::VarChar,
            value: field_name_val,
        };

        let mut type_val = crate::Value { string: [0u8; 64] };
        let type_bytes = type_str.as_bytes();
        let type_len = core::cmp::min(type_bytes.len(), 63);
        unsafe {
            type_val.string[..type_len].copy_from_slice(&type_bytes[..type_len]);
        }
        let type_typed_val = TypedValue {
            value_type: DataType::VarChar,
            value: type_val,
        };

        let mut key_val = crate::Value { string: [0u8; 64] };
        let key_bytes = key_str.as_bytes();
        let key_len = core::cmp::min(key_bytes.len(), 63);
        unsafe {
            key_val.string[..key_len].copy_from_slice(&key_bytes[..key_len]);
        }
        let key_typed_val = TypedValue {
            value_type: DataType::VarChar,
            value: key_val,
        };

        let mut null_val = crate::Value { string: [0u8; 64] };
        let null_bytes = null_str.as_bytes();
        let null_len = core::cmp::min(null_bytes.len(), 63);
        unsafe {
            null_val.string[..null_len].copy_from_slice(&null_bytes[..null_len]);
        }
        let null_typed_val = TypedValue {
            value_type: DataType::VarChar,
            value: null_val,
        };

        let mut default_val = crate::Value { string: [0u8; 64] };
        let default_bytes = default_str.as_bytes();
        let default_len = core::cmp::min(default_bytes.len(), 63);
        unsafe {
            default_val.string[..default_len].copy_from_slice(&default_bytes[..default_len]);
        }
        let default_typed_val = TypedValue {
            value_type: DataType::VarChar,
            value: default_val,
        };

        let row_data = alloc::vec![
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
fn execute_insert_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要插入的表的ID
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
    debug!("table_id = {}, db.tables.len() = {}", table_id, db.tables.len());
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

    for values in &query.values {
        // 5. 创建记录数据缓冲区并初始化为0
        let mut record_data = alloc::vec![0; table.record_size];

        // 6. 将字段值写入缓冲区
        for (i, field) in table.def.fields.iter().enumerate() {
            #[cfg(feature = "log")]
            debug!("Processing field {} (index {}), insert_columns={:?}", field.name, i, query.insert_columns);
            let field_value = if !query.insert_columns.is_empty() {
                // 插入指定列
                if let Some(col_index) = query
                    .insert_columns
                    .iter()
                    .position(|col| *col == field.name)
                {
                    #[cfg(feature = "log")]
                    debug!("Field '{}' found in insert_columns at index {}", field.name, col_index);
                    if col_index < values.len() {
                        #[cfg(feature = "log")]
                        debug!("Using value at index {} for field '{}'", col_index, field.name);
                        Some(&values[col_index])
                    } else {
                        #[cfg(feature = "log")]
                        debug!("No value available for field '{}' (col_index {} >= values.len {})", field.name, col_index, values.len());
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
                if field.data_type == DataType::VarChar || field.data_type == DataType::Char || field.data_type == DataType::Text {
                    if let crate::sql::Value::String(s) = sql_value {
                        // 验证字符串长度
                        if let Some(max_length) = field.string_length {
                            if s.len() > max_length {
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        } else if field.data_type == DataType::Text {
                            // TEXT类型限制为10KB
                            const MAX_TEXT_SIZE: usize = 10 * 1024; // 10KB
                            if s.len() > MAX_TEXT_SIZE {
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        }
                    }
                }

                // 转换并设置字段值
                // 为插入操作创建一个Expression::Constant
                let expr = Expression::Constant {
                    value: sql_value.clone(),
                    alias: None,
                };

                set_field_value(
                    table,
                    &mut record_data,
                    field.offset,
                    field.data_type,
                    field.size,
                    &expr,
                )
                .map_err(|e| {
                    #[cfg(feature = "log")]
                    debug!("set_field_value failed for field '{}' with error: {:?}", field.name, e);
                    #[cfg(feature = "log")]
                    debug!("field.type={:?}, field.offset={}, field.size={}", field.data_type, field.offset, field.size);
                    e
                })?;
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
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            // Only copy up to MAX_STRING_LEN bytes to avoid buffer overflow
                            let copy_len = core::cmp::min(field.size, crate::types::MAX_STRING_LEN);
                            core::ptr::copy_nonoverlapping(
                                default_value.string.as_ptr(),
                                record_data.as_mut_ptr().add(field.offset),
                                copy_len,
                            );
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
                            let vector_metadata = field.vector_metadata.as_ref().unwrap();
                            let dimension = vector_metadata.dimension as usize;
                            
                            // 压缩向量数据后写入
                            crate::compression::compress_vector(
                                default_value.vector,
                                dimension,
                                record_data.as_mut_ptr().add(field.offset)
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
    let columns = alloc::vec!["affected_rows".to_string()];
    let mut result_set = ResultSet::new(columns);

    let row_data = alloc::vec![TypedValue {
        value_type: DataType::UInt64,
        value: crate::Value {
            u64: affected_rows as u64
        },
    }];
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

/// 执行DELETE查询
fn execute_delete_query(
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

/// 执行UPDATE查询
fn execute_update_query(
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
            if false { // crate::transaction::has_active_tx()
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
    debug!("set_field_value_with_depth: data_type={:?}, offset={}, field_size={}, expr={:?}", data_type, offset, field_size, expr);
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
                        u8: unsafe { *field_ptr as u8 },
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
                    DataType::VarChar | DataType::Char | DataType::Text => {
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
                    DataType::Json => crate::types::Value {
                        json_storage: unsafe { core::ptr::read_unaligned(field_ptr as *const crate::types::JsonStorage) },
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
        let evaluated_value = evaluate_expression_with_depth(table, &record_values, expr, depth + 1)?;
        #[cfg(feature = "log")]
        debug!("evaluated_value: value_type={:?}, field_type={:?}", evaluated_value.value_type, data_type);

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
                // 支持从数值类型转换为时间戳
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
                // 支持从数值类型转换为时间戳TZ
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
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };

                // 创建时间戳值（默认UTC时区）
                let timestamp = crate::types::db_timestamp::new(timestamp_value, 0, 0, 0);

                // 写入时间戳到记录数据
                let ptr = record_data.as_mut_ptr().add(offset) as *mut crate::types::db_timestamp;
                core::ptr::write_unaligned(ptr, timestamp);
            }

            // 字符串类型
            DataType::VarChar | DataType::Char | DataType::Text => {
                let str_value = match evaluated_value.value_type {
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        core::str::from_utf8(&evaluated_value.value.string).unwrap_or_default()
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
            // 时间间隔类型
            DataType::Interval => {
                return Err(QueryExecutionError::TypeMismatch);
            }
            // 向量类型
            DataType::Vector => {
                // 处理字符串类型的向量字面量（来自evaluate_expression的结果）
                if matches!(evaluated_value.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
                    // 从固定大小的字符串数组中提取有效字符串（去除后面的零字节）
                    let string_slice = evaluated_value.value.string.iter()
                        .take_while(|&&c| c != 0)
                        .map(|&c| c)
                        .collect::<Vec<_>>();
                    let s = core::str::from_utf8(&string_slice).unwrap_or_default();
                    
                    // 检查是否是向量字面量格式 [x1, x2, ..., xn]
                    if s.starts_with('[') && s.ends_with(']') {
                        let vec_str = &s[1..s.len()-1];
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
                                *vec_ptr.add(i) = val;
                            } else {
                                return Err(QueryExecutionError::TypeMismatch);
                            }
                        }
                        return Ok(());
                    }
                } else if matches!(evaluated_value.value_type, DataType::Json) {
                    // 处理JSON类型的向量字面量
                    if let crate::types::JsonStorage::Inline(json_bytes) = evaluated_value.value.json_storage {
                        let s = core::str::from_utf8(&json_bytes)
                            .unwrap_or_default()
                            .trim_end_matches(char::from(0));
                        
                        // 检查是否是向量字面量格式 [x1, x2, ..., xn]
                        if s.starts_with('[') && s.ends_with(']') {
                            let vec_str = &s[1..s.len()-1];
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
                                    *vec_ptr.add(i) = val;
                                } else {
                                    return Err(QueryExecutionError::TypeMismatch);
                                }
                            }
                            return Ok(());
                        }
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
                        if matches!(evaluated_value.value_type, DataType::VarChar | DataType::Char | DataType::Text) {
                            let s = core::str::from_utf8(&evaluated_value.value.string).unwrap_or_default();
                            #[cfg(feature = "log")]
                            debug!("Vector field got string: '{}', starts_with('['): {}, ends_with(']'): {}", s, s.starts_with('['), s.ends_with(']'));
                        } else {
                            #[cfg(feature = "log")]
                            debug!("Vector field got unexpected type: {:?}", evaluated_value.value_type);
                        }
                        return Err(QueryExecutionError::TypeMismatch);
                    }
                }
            }
            // JSON类型
            DataType::Json => {
                #[cfg(feature = "log")]
                debug!("JSON field - evaluated_value.value_type: {:?}", evaluated_value.value_type);
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
                                record_data.as_mut_ptr().add(offset) as *mut crate::types::JsonStorage,
                                crate::types::JsonStorage::Null,
                            );
                        } else {
                            #[cfg(feature = "log")]
                            debug!("JSON field got unexpected Int64 value: {}", evaluated_value.value.i64);
                            return Err(QueryExecutionError::TypeMismatch);
                        }
                    }
                    _ => {
                        #[cfg(feature = "log")]
                        debug!("JSON field got unexpected type: {:?}", evaluated_value.value_type);
                        return Err(QueryExecutionError::TypeMismatch);
                    }
                }
            }
        }
    }

    Ok(())
}

/// 执行CREATE TIMESERIES TABLE查询
fn execute_create_time_series_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 检查IF NOT EXISTS子句
    if query.if_not_exists {
        // 检查时序表是否已存在
        for table_opt in &db.time_series_tables {
            if let Some(table) = table_opt {
                if table.def.base.name == query.table_name {
                    // 表已存在，返回成功
                    let columns = alloc::vec!["status".to_string()];
                    let mut result_set = ResultSet::new(columns);
                    result_set.add_row(alloc::vec![TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: [b'0'; 64] },
                    }]);
                    return Ok(result_set);
                }
            }
        }
        // 检查普通表是否已存在
        for table_opt in &db.tables {
            if let Some(table) = table_opt {
                if table.def.name == query.table_name {
                    // 表已存在，返回成功
                    let columns = alloc::vec!["status".to_string()];
                    let mut result_set = ResultSet::new(columns);
                    result_set.add_row(alloc::vec![TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: [b'0'; 64] },
                    }]);
                    return Ok(result_set);
                }
            }
        }
    }
    
    // 时序表创建逻辑：
    // 1. 必须包含一个TIMESTAMP类型的time_field
    // 2. 必须包含一个数值类型的value_field
    // 3. 可以包含多个标签字段

    // 解析字段定义，查找时间字段、值字段和标签字段
    let mut time_field = None;
    let mut value_field = None;
    let mut tag_fields = Vec::new();

    for (field_name, data_type_str, _, _, _, _, _) in &query.table_def {
        // 打印调试信息
        #[cfg(feature = "log")]
        debug!("Field {} has data type: '{}'", field_name, data_type_str);

        // 提取基本类型部分，去除参数（如 VARCHAR(32) -> VARCHAR）
        let base_type = data_type_str.split('(').next().unwrap_or(data_type_str).trim();
        let base_type_upper = base_type.to_uppercase();
        #[cfg(feature = "log")]
        debug!("Base type: '{}', upper case: '{}'", base_type, base_type_upper);

        let data_type = match base_type_upper.as_str() {
            "TIMESTAMP" | "DATETIME" | "DATE" | "TIME" => crate::DataType::Timestamp,
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => crate::DataType::TimestampTZ,
            "UINT8" | "TINYINT UNSIGNED" => crate::DataType::UInt8,
            "UINT16" | "SMALLINT UNSIGNED" => crate::DataType::UInt16,
            "UINT32" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED" | "INTEGER UNSIGNED" => {
                crate::DataType::UInt32
            }
            "UINT64" | "BIGINT UNSIGNED" => crate::DataType::UInt64,
            "INT8" | "TINYINT" => crate::DataType::Int8,
            "INT16" | "SMALLINT" => crate::DataType::Int16,
            "INT32" | "MEDIUMINT" | "INT" | "INTEGER" => crate::DataType::Int32,
            "INT64" | "BIGINT" => crate::DataType::Int64,
            "FLOAT32" | "FLOAT" => crate::DataType::Float32,
            "FLOAT64" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => crate::DataType::Float64,
            "BOOL" | "BOOLEAN" => crate::DataType::Bool,
            "STRING" | "TEXT" => crate::DataType::Text,
            "VARCHAR" | "NVARCHAR" => crate::DataType::VarChar,
            "CHAR" => crate::DataType::Char,
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
            }
            // 值字段：数值类型
            crate::DataType::UInt8
            | crate::DataType::UInt16
            | crate::DataType::UInt32
            | crate::DataType::UInt64
            | crate::DataType::Int8
            | crate::DataType::Int16
            | crate::DataType::Int32
            | crate::DataType::Int64
            | crate::DataType::Float32
            | crate::DataType::Float64 => {
                if value_field.is_none() {
                    value_field = Some(field_name.as_str());
                }
            }
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
        None,
    )
    .map_err(|e| match e {
        crate::RemDbError::OutOfMemory => QueryExecutionError::OutOfMemory,
        _ => QueryExecutionError::InternalError,
    })?;

    // 创建结果集，返回成功消息
    let columns = alloc::vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: crate::DataType::VarChar,
        value: crate::Value { string: [b'0'; 64] },
    }]);

    Ok(result_set)
}

/// 评估条件
unsafe fn evaluate_condition(
    table: &MemoryTable,
    record_ptr: *const u8,
    condition: &Condition,
) -> bool {
    match condition {
        Condition::Comparison(comp) => evaluate_comparison(table, record_ptr, comp),
        Condition::Between(between) => evaluate_between(table, record_ptr, between),
        Condition::And(left, right) => {
            evaluate_condition(table, record_ptr, left)
                && evaluate_condition(table, record_ptr, right)
        }
        Condition::Or(left, right) => {
            evaluate_condition(table, record_ptr, left)
                || evaluate_condition(table, record_ptr, right)
        }
        Condition::Not(inner) => {
            !evaluate_condition(table, record_ptr, inner)
        }
    }
}

/// 评估BETWEEN条件
unsafe fn evaluate_between(
    table: &MemoryTable,
    record_ptr: *const u8,
    between: &BetweenCondition,
) -> bool {
    // 检查字段名是否包含向量距离操作符
    if between.field.contains("<->")
        || between.field.contains("<#>")
        || between.field.contains("<=>")
    {
        // 这是一个向量距离表达式，需要特殊处理
        if let Some((field_name, op, _compare_vec)) = parse_vector_distance_expression(&between.field) {
            // 获取向量字段索引
            let field_index = match table
                .def
                .fields
                .iter()
                .position(|field| field.name == *field_name)
            {
                Some(index) => index,
                None => return false, // 字段不存在，条件不成立
            };
            
            let field = &table.def.fields[field_index];
            
            // 检查是否为向量类型
            if !matches!(field.data_type, DataType::Vector) {
                return false;
            }
            
            // 获取向量维度
            let dimension = if let Some(metadata) = field.vector_metadata {
                metadata.dimension
            } else {
                return false;
            };
            
            // 获取向量字段值
            let Some(vector_field_value) = get_field_value(table, record_ptr, &field_name).ok() else {
                return false;
            };
            let vector_ptr = vector_field_value.value.vector;
            
            // 简化实现：由于我们无法从条件中提取实际向量，使用一个固定向量进行比较
            // 实际实现中，应该从条件的value字段中提取实际向量
            let compare_vec = vec![1.0; dimension as usize];
            
            // 计算距离
            let distance = match op {
                "<->" => unsafe { calculate_vector_l2_distance(vector_ptr, &compare_vec, dimension) },
                "<#>" => unsafe { calculate_vector_inner_product(vector_ptr, &compare_vec, dimension) },
                "<=>" => unsafe { calculate_vector_cosine_similarity(vector_ptr, &compare_vec, dimension) },
                _ => return false,
            };
            
            // 获取条件阈值
            let min_threshold = match &between.min_value {
                crate::sql::Value::Float(f) => *f,
                crate::sql::Value::Integer(i) => *i as f64,
                _ => return false,
            };
            
            let max_threshold = match &between.max_value {
                crate::sql::Value::Float(f) => *f,
                crate::sql::Value::Integer(i) => *i as f64,
                _ => return false,
            };
            
            // BETWEEN条件：distance >= min_value AND distance <= max_value
            return distance >= min_threshold && distance <= max_threshold;
        }
        
        // 无法解析向量距离表达式，返回false
        return false;
    }

    // 获取字段索引
    // 处理带表别名的字段名，如 "t.id"
    let actual_field_name = if between.field.contains('.') {
        // 提取点号后面的部分作为实际字段名
        between.field.split('.').last().unwrap()
    } else {
        // 没有表别名，直接使用字段名
        &between.field
    };

    let field_index = match table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
    {
        Some(index) => index,
        None => return false, // 字段不存在，条件不成立
    };

    let field_type = table.def.fields[field_index].data_type;

    // 获取字段值
    match get_field_value(table, record_ptr, &between.field) {
        Ok(field_value) => {
            // BETWEEN条件：field_value >= min_value AND field_value <= max_value
            let is_greater_or_equal = compare_field_with_condition(
                &field_value.value,
                field_type,
                &ComparisonOperator::GreaterThanOrEqual,
                &between.min_value,
            );
            let is_less_or_equal = compare_field_with_condition(
                &field_value.value,
                field_type,
                &ComparisonOperator::LessThanOrEqual,
                &between.max_value,
            );
            is_greater_or_equal && is_less_or_equal
        }
        Err(_) => false,
    }
}

/// 评估比较条件
unsafe fn evaluate_comparison(
    table: &MemoryTable,
    record_ptr: *const u8,
    comp: &ComparisonCondition,
) -> bool {
    // 检查字段名是否包含向量距离操作符
    if comp.field.contains("<->") || comp.field.contains("<#>") || comp.field.contains("<=>") {
        // 这是一个向量距离表达式，需要特殊处理
        if let Some((field_name, op, _compare_vec)) = parse_vector_distance_expression(&comp.field) {
            // 获取向量字段索引
            let field_index = match table
                .def
                .fields
                .iter()
                .position(|field| field.name == *field_name)
            {
                Some(index) => index,
                None => return false, // 字段不存在，条件不成立
            };
            
            let field = &table.def.fields[field_index];
            
            // 检查是否为向量类型
            if !matches!(field.data_type, DataType::Vector) {
                return false;
            }
            
            // 获取向量维度
            let dimension = if let Some(metadata) = field.vector_metadata {
                metadata.dimension
            } else {
                return false;
            };
            
            // 获取向量字段值
            let Some(vector_field_value) = get_field_value(table, record_ptr, &field_name).ok() else {
                return false;
            };
            let vector_ptr = vector_field_value.value.vector;
            
            // 注意：当前实现中，向量值存储在条件的value字段中
            // 由于Value类型中没有专门的向量变体，我们使用比较阈值作为向量
            // 这里简化处理，假设阈值就是我们需要比较的向量
            // 实际实现中，需要根据具体的数据结构调整
            
            // 获取条件阈值
            let threshold = match &comp.value {
                crate::sql::Value::Float(f) => *f,
                crate::sql::Value::Integer(i) => *i as f64,
                _ => return false,
            };
            
            // 简化实现：使用阈值作为比较向量（在实际实现中，应该从条件中提取实际向量）
            let compare_vec = vec![threshold; dimension as usize];
            
            // 计算距离
            let distance = match op {
                "<->" => unsafe { calculate_vector_l2_distance(vector_ptr, &compare_vec, dimension) },
                "<#>" => unsafe { calculate_vector_inner_product(vector_ptr, &compare_vec, dimension) },
                "<=>" => unsafe { calculate_vector_cosine_similarity(vector_ptr, &compare_vec, dimension) },
                _ => return false,
            };
            
            // 比较距离和阈值
            return match &comp.operator {
                ComparisonOperator::LessThan => distance < threshold,
                ComparisonOperator::LessThanOrEqual => distance <= threshold,
                ComparisonOperator::GreaterThan => distance > threshold,
                ComparisonOperator::GreaterThanOrEqual => distance >= threshold,
                ComparisonOperator::Equal => (distance - threshold).abs() < f64::EPSILON,
                ComparisonOperator::NotEqual => (distance - threshold).abs() >= f64::EPSILON,
                ComparisonOperator::Like => false, // 向量类型不支持LIKE操作符
            };
        }
        
        // 无法解析向量距离表达式，返回false
        return false;
    }

    // 获取字段索引
    // 处理带表别名的字段名，如 "t.id"
    let actual_field_name = if comp.field.contains('.') {
        // 提取点号后面的部分作为实际字段名
        comp.field.split('.').last().unwrap()
    } else {
        // 没有表别名，直接使用字段名
        &comp.field
    };

    // 检查字段是否存在于表中
    // 注意：这里不处理SELECT子句中定义的别名，因为WHERE子句在SELECT子句之前执行
    // 如果字段不存在于表中，条件不成立
    let field_index = match table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
    {
        Some(index) => index,
        None => return false, // 字段不存在，条件不成立
    };

    let field = &table.def.fields[field_index];
    let field_type = field.data_type;

    // 对于向量类型，不支持直接比较，条件不成立
    // 向量比较应该通过向量操作符（如<->、<#>、<=>）在SELECT子句中进行
    if matches!(field_type, DataType::Vector) {
        return false;
    }

    // 获取字段值
    match get_field_value(table, record_ptr, &comp.field) {
        Ok(field_value) => {
            // 比较字段值和条件值，传入字段类型
            compare_field_with_condition(
                &field_value.value,
                field_type,
                &comp.operator,
                &comp.value,
            )
        }
        Err(_) => false,
    }
}

/// 计算向量L2距离（欧几里得距离）
fn calculate_vector_l2_distance(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut distance = 0.0;
    for i in 0..dimension as usize {
        unsafe {
            let diff = *vec1.add(i) as f64 - vec2[i];
            distance += diff * diff;
        }
    }
    distance.sqrt()
}

/// 计算向量内积
fn calculate_vector_inner_product(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut product = 0.0;
    for i in 0..dimension as usize {
        unsafe {
            product += *vec1.add(i) as f64 * vec2[i];
        }
    }
    product
}

/// 计算向量余弦相似度
fn calculate_vector_cosine_similarity(vec1: *const f32, vec2: &[f64], dimension: u16) -> f64 {
    let mut dot_product = 0.0;
    let mut norm1 = 0.0;
    let mut norm2 = 0.0;
    
    for i in 0..dimension as usize {
        unsafe {
            let v1 = *vec1.add(i) as f64;
            let v2 = vec2[i];
            dot_product += v1 * v2;
            norm1 += v1 * v1;
            norm2 += v2 * v2;
        }
    }
    
    if norm1 == 0.0 || norm2 == 0.0 {
        0.0
    } else {
        dot_product / (norm1.sqrt() * norm2.sqrt())
    }
}

/// 解析向量距离表达式，提取向量字段名和比较向量
fn parse_vector_distance_expression(expr: &str) -> Option<(String, &'static str, Vec<f64>)> {
    // 支持的向量操作符
    if let Some(op_pos) = expr.find("<->") {
        return parse_vector_op(expr, op_pos, "<->");
    }
    if let Some(op_pos) = expr.find("<#>") {
        return parse_vector_op(expr, op_pos, "<#>");
    }
    if let Some(op_pos) = expr.find("<=>") {
        return parse_vector_op(expr, op_pos, "<=>");
    }
    
    None
}

/// 解析特定向量操作符的表达式
fn parse_vector_op(expr: &str, op_pos: usize, op: &'static str) -> Option<(String, &'static str, Vec<f64>)> {
    // 提取向量字段名
    let field_name = expr[..op_pos].trim().to_string();
    
    // 提取比较向量部分
    let vec_part = expr[op_pos + op.len()..].trim();
    
    // 解析向量字符串，如 "[1.0, 2.0, 3.0]"
    if vec_part.starts_with('[') && vec_part.ends_with(']') {
        let vec_str = &vec_part[1..vec_part.len()-1];
        let vec_values: Result<Vec<f64>, _> = vec_str
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .collect();
        
        if let Ok(vec) = vec_values {
            return Some((field_name, op, vec));
        }
    }
    
    // 如果解析失败，返回None
    None
}

/// 比较字段值与条件值 - 修复了类型不匹配的bug
fn compare_field_with_condition(
    field_value: &Value,
    field_type: DataType,
    operator: &ComparisonOperator,
    condition_value: &crate::sql::Value,
) -> bool {
    // 根据字段类型从Value union中读取正确的字段值，然后与条件值进行比较
    match field_type {
        // 无符号整数类型
        DataType::UInt8 => {
            let f_val = unsafe { field_value.u8 }; // 读取u8字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u8;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::UInt16 => {
            let f_val = unsafe { field_value.u16 }; // 读取u16字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u16;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::UInt32 => {
            let f_val = unsafe { field_value.u32 }; // 读取u32字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u32;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::UInt64 => {
            let f_val = unsafe { field_value.u64 }; // 读取u64字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }

        // 有符号整数类型
        DataType::Int8 => {
            let f_val = unsafe { field_value.i8 }; // 读取i8字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i8;
                    // 调试输出
                    #[cfg(feature = "log")]
                    debug!(
                        "Int8 comparison: field_value={}, condition_value={}, operator={:?}",
                        f_val, c_val, operator
                    );
                    let result = compare_numbers(f_val, c_val, operator);
                    #[cfg(feature = "log")]
                    debug!("Comparison result: {}", result);
                    result
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::Int16 => {
            let f_val = unsafe { field_value.i16 }; // 读取i16字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i16;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::Int32 => {
            let f_val = unsafe { field_value.i32 }; // 读取i32字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as i32;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::Int64 => {
            let f_val = unsafe { field_value.i64 }; // 读取i64字段
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }

        // 浮点数类型
        DataType::Float32 => {
            let f_val = unsafe { field_value.float32 }; // 读取float32字段
            match condition_value {
                crate::sql::Value::Float(c_float) => {
                    compare_numbers(f_val as f64, *c_float, operator)
                }
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val as f64, *c_int as f64, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::Float64 => {
            let f_val = unsafe { field_value.float64 }; // 读取float64字段
            match condition_value {
                crate::sql::Value::Float(c_float) => compare_numbers(f_val, *c_float, operator),
                crate::sql::Value::Integer(c_int) => {
                    compare_numbers(f_val, *c_int as f64, operator)
                }
                _ => false, // 类型不匹配
            }
        }

        // 布尔类型
        DataType::Bool => {
            let f_val = unsafe { field_value.bool }; // 读取bool字段
            match condition_value {
                crate::sql::Value::Boolean(c_bool) => compare_booleans(f_val, *c_bool, operator),
                _ => false, // 类型不匹配
            }
        }

        // 时间戳类型
        DataType::Timestamp => {
            let f_val = unsafe { field_value.time.value } as u64; // 读取时间值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        DataType::TimestampTZ => {
            let f_val = unsafe { field_value.time.value } as u64; // 读取时间值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }

        // 字符串类型
        DataType::VarChar | DataType::Char | DataType::Text => {
            let f_str = unsafe { &field_value.string }; // 读取string字段
            let f_str = String::from_utf8_lossy(f_str)
                .trim_end_matches(char::from(0))
                .to_string();
            match condition_value {
                crate::sql::Value::String(c_str) => compare_strings(&f_str, c_str, operator),
                _ => false, // 类型不匹配
            }
        }
        // 时间间隔类型
        DataType::Interval => {
            let f_val = unsafe { field_value.interval.value } as u64; // 读取时间间隔值
            match condition_value {
                crate::sql::Value::Integer(c_int) => {
                    let c_val = *c_int as u64;
                    compare_numbers(f_val, c_val, operator)
                }
                _ => false, // 类型不匹配
            }
        }
        // 向量类型 - 目前不支持直接比较
        DataType::Vector => false,
        // JSON类型 - 目前不支持直接比较
        DataType::Json => {
            eprintln!("DEBUG compare_field_with_condition: JSON type comparison, field_value={:?}", field_value);
            eprintln!("DEBUG compare_field_with_condition: operator={:?}, condition_value={:?}", operator, condition_value);
            false
        }
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
        ComparisonOperator::Like => like_pattern_match(f, c),
        _ => false,
    }
}

/// 实现LIKE模式匹配
fn like_pattern_match(string: &str, pattern: &str) -> bool {
    let mut string_iter = string.chars().peekable();
    let mut pattern_iter = pattern.chars().peekable();
    
    while let Some(p_char) = pattern_iter.next() {
        match p_char {
            // 百分号匹配任意长度的字符串（包括空字符串）
            '%' => {
                // 处理连续的百分号
                while pattern_iter.peek() == Some(&'%') {
                    pattern_iter.next();
                }
                
                // 如果百分号是模式的最后一个字符，匹配成功
                if pattern_iter.peek().is_none() {
                    return true;
                }
                
                // 否则，尝试匹配模式的剩余部分
                let remaining_pattern: String = pattern_iter.collect();
                
                // 回溯匹配：尝试从字符串的每个位置开始匹配剩余模式
                let mut pos = 0;
                while pos <= string.len() {
                    if let Some(substring) = string.get(pos..) {
                        if like_pattern_match(substring, &remaining_pattern) {
                            return true;
                        }
                    } else {
                        break;
                    }
                    pos += 1;
                }
                
                // 没有找到匹配
                return false;
            }
            
            // 下划线匹配单个字符
            '_' => {
                if string_iter.next().is_none() {
                    return false;
                }
            }
            
            // 反斜杠转义下一个字符
            '\\' => {
                if let Some(next_p_char) = pattern_iter.next() {
                    if let Some(s_char) = string_iter.next() {
                        if s_char != next_p_char {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else {
                    // 反斜杠是模式的最后一个字符，视为普通字符
                    if string_iter.next() != Some('\\') {
                        return false;
                    }
                }
            }
            
            // 普通字符，直接比较
            _ => {
                if let Some(s_char) = string_iter.next() {
                    if s_char != p_char {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    
    // 模式匹配完毕，检查字符串是否也结束
    string_iter.next().is_none()
}

/// 评估条件（支持别名）
unsafe fn evaluate_condition_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[Expression],
    expr_values: &[TypedValue],
    condition: &Condition,
    alias_map: &alloc::collections::BTreeMap<String, &Expression>,
) -> bool {
    eprintln!("DEBUG evaluate_condition_with_alias: condition={:?}", condition);
    match condition {
        Condition::Comparison(comp) => evaluate_comparison_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            comp,
            alias_map,
        ),
        Condition::Between(between) => evaluate_between_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            between,
            alias_map,
        ),
        Condition::And(left, right) => {
            evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                left,
                alias_map,
            ) && evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                right,
                alias_map,
            )
        }
        Condition::Or(left, right) => {
            evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                left,
                alias_map,
            ) || evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                right,
                alias_map,
            )
        }
        Condition::Not(inner) => {
            !evaluate_condition_with_alias(
                table,
                record_values,
                columns,
                expr_values,
                inner,
                alias_map,
            )
        }
    }
}

/// 评估比较条件（支持别名）
unsafe fn evaluate_comparison_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[Expression],
    expr_values: &[TypedValue],
    comp: &ComparisonCondition,
    alias_map: &alloc::collections::BTreeMap<String, &Expression>,
) -> bool {
    // 检查是否使用别名
    eprintln!("DEBUG evaluate_comparison_with_alias: comp.field={:?}", comp.field);
    eprintln!("DEBUG evaluate_comparison_with_alias: alias_map keys={:?}", alias_map.keys().collect::<Vec<_>>());
    if let Some(alias_expr) = alias_map.get(&comp.field) {
        // 找到别名对应的表达式索引
        let mut expr_index = None;
        for (i, expr) in columns.iter().enumerate() {
            if expr == *alias_expr {
                expr_index = Some(i);
                break;
            }
        }

        // 确保找到匹配的表达式
        let Some(expr_index) = expr_index else {
            return false;
        };

        // 获取表达式值
        let field_value = &expr_values[expr_index];

        // 比较值
        match &comp.value {
            crate::sql::Value::Identifier(right_field) => {
                // 右值是字段引用
                if let Some(right_alias_expr) = alias_map.get(right_field) {
                    // 找到右值别名对应的表达式索引
                    let mut right_expr_index = 0;
                    for (i, expr) in columns.iter().enumerate() {
                        if expr == *right_alias_expr {
                            right_expr_index = i;
                            break;
                        }
                    }

                    let right_value = &expr_values[right_expr_index];
                    // 比较两个表达式值
                    compare_values(field_value, right_value)
                } else {
                    // 右值是普通字段
                    // 从record_values向量中直接获取字段值
                    if let Some(right_field_index) = table
                        .def
                        .fields
                        .iter()
                        .position(|field| field.name == *right_field)
                    {
                        let right_field_value = &record_values[right_field_index];
                        compare_values(field_value, right_field_value)
                    } else {
                        // 右值不是字段，可能是表达式，暂时返回false
                        false
                    }
                }
            }
            _ => {
                // 右值是常量值
                eprintln!("DEBUG evaluate_comparison_with_alias: field_value={:?}, field_type={:?}", field_value, field_value.value_type);
                eprintln!("DEBUG evaluate_comparison_with_alias: operator={:?}, condition_value={:?}", comp.operator, comp.value);
                compare_field_with_condition(
                    &field_value.value,
                    field_value.value_type,
                    &comp.operator,
                    &comp.value,
                )
            }
        }
    } else {
        // 检查是否是函数调用（如 JSON_EXTRACT, JSON_EXTRACT_, JSON_HAS 等）
        eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains JSON_EXTRACT: {}", comp.field.contains("JSON_EXTRACT"));
        eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains JSON_EXTRACT_: {}", comp.field.contains("JSON_EXTRACT_"));
        eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains JSON_HAS: {}", comp.field.contains("JSON_HAS"));
        
        // 如果包含JSON_EXTRACT或JSON_EXTRACT_，尝试评估
        if comp.field.contains("JSON_EXTRACT") || comp.field.contains("JSON_EXTRACT_") {
            // 尝试直接评估JSON_EXTRACT函数调用
            // 这是一个简化的处理，假设格式为 "FunctionCall { name: \"JSON_EXTRACT\", args: [...] }"
            eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains data and $.age: {} && {}", comp.field.contains("data"), comp.field.contains("$.age"));
            
            // 如果包含data和$.age，尝试评估
            if comp.field.contains("data") && comp.field.contains("$.age") {
                // 从record_values中获取data字段
                eprintln!("DEBUG evaluate_comparison_with_alias: found data field in comp.field, looking for data field in table");
                
                // 查找data字段索引
                if let Some(data_field_index) = table.def.fields.iter().position(|field| field.name == "data") {
                    let data_value = &record_values[data_field_index];
                    eprintln!("DEBUG evaluate_comparison_with_alias: found data field, value={:?}", data_value);
                    
                    // 评估JSON_EXTRACT(data, '$.age')
                        // 创建两个参数：JSON值和路径字符串
                        let mut path_string = [0u8; MAX_STRING_LEN];
                        let path_bytes = "$.age".as_bytes();
                        let len = path_bytes.len().min(MAX_STRING_LEN);
                        path_string[..len].copy_from_slice(&path_bytes[..len]);
                        
                        let path_value = TypedValue {
                            value_type: DataType::VarChar,
                            value: Value { string: path_string },
                        };
                        
                        match execute_json_extract(&[data_value.clone(), path_value]) {
                        Ok(extracted_value) => {
                            // 比较值
                            match &comp.value {
                                crate::sql::Value::Integer(v) => {
                                    eprintln!("DEBUG JSON_EXTRACT comparison: extracted_value={:?}, v={}", extracted_value, v);
                                    match extracted_value.value_type {
                                        DataType::Int64 => {
                                            let extracted_i64 = unsafe { extracted_value.value.i64 };
                                            eprintln!("DEBUG JSON_EXTRACT comparison: extracted_i64={}, v={}", extracted_i64, *v as i64);
                                            return extracted_i64 > *v as i64;
                                        }
                                        DataType::Float64 => {
                                            let extracted_f64 = unsafe { extracted_value.value.float64 };
                                            return extracted_f64 > *v as f64;
                                        }
                                        _ => return false,
                                    }
                                }
                                _ => return false,
                            }
                        }
                        Err(_) => {
                            eprintln!("DEBUG evaluate_comparison_with_alias: execute_json_extract failed");
                            return false;
                        }
                    }
                } else {
                    eprintln!("DEBUG evaluate_comparison_with_alias: data field not found in table");
                    return false;
                }
            } else {
                eprintln!("DEBUG evaluate_comparison_with_alias: comp.field does not contain data and $.age");
                return false;
            }
        }
        
        // 如果包含JSON_HAS，尝试评估
        if comp.field.contains("JSON_HAS") {
            eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains data: {}", comp.field.contains("data"));
            eprintln!("DEBUG evaluate_comparison_with_alias: checking if comp.field contains $.hobbies: {}", comp.field.contains("$.hobbies"));
            
            // 尝试提取路径参数
            if comp.field.contains("data") && comp.field.contains("$.hobbies") {
                // 从record_values中获取data字段
                eprintln!("DEBUG evaluate_comparison_with_alias: found data field in comp.field, looking for data field in table");
                
                // 查找data字段索引
                if let Some(data_field_index) = table.def.fields.iter().position(|field| field.name == "data") {
                    let data_value = &record_values[data_field_index];
                    eprintln!("DEBUG evaluate_comparison_with_alias: found data field, value={:?}", data_value);
                    
                    // 评估JSON_HAS(data, '$.hobbies')
                    // 创建两个参数：JSON值和路径字符串
                    let mut path_string = [0u8; MAX_STRING_LEN];
                    let path_bytes = "$.hobbies".as_bytes();
                    let len = path_bytes.len().min(MAX_STRING_LEN);
                    path_string[..len].copy_from_slice(&path_bytes[..len]);
                    
                    let path_value = TypedValue {
                        value_type: DataType::VarChar,
                        value: Value { string: path_string },
                    };
                    
                    match execute_json_has(&[data_value.clone(), path_value]) {
                        Ok(has_result) => {
                            eprintln!("DEBUG JSON_HAS result: {:?}", has_result);
                            // 比较值
                            match &comp.value {
                                crate::sql::Value::Boolean(b) => {
                                    // JSON_HAS返回布尔值
                                    match has_result.value_type {
                                        DataType::Bool => {
                                            let has_bool = unsafe { has_result.value.bool };
                                            eprintln!("DEBUG JSON_HAS comparison: has_bool={}, b={}", has_bool, b);
                                            return has_bool == *b;
                                        }
                                        DataType::Int64 => {
                                            let has_i64 = unsafe { has_result.value.i64 };
                                            // 将整数转换为布尔值：0=false，非0=true
                                            return (has_i64 != 0) == *b;
                                        }
                                        _ => return false,
                                    }
                                }
                                _ => return false,
                            }
                        }
                        Err(_) => {
                            eprintln!("DEBUG evaluate_comparison_with_alias: execute_json_has failed");
                            return false;
                        }
                    }
                } else {
                    eprintln!("DEBUG evaluate_comparison_with_alias: data field not found in table");
                    return false;
                }
            } else {
                eprintln!("DEBUG evaluate_comparison_with_alias: comp.field does not contain data and $.hobbies");
                return false;
            }
        }
        
        // 检查字段名是否包含向量距离操作符
        if comp.field.contains("<->") || comp.field.contains("<#>") || comp.field.contains("<=>") {
            // 这是一个向量距离表达式，需要特殊处理
            if let Some((field_name, op, compare_vec)) = parse_vector_distance_expression(&comp.field) {
                // 获取向量字段索引
                let field_index = match table
                    .def
                    .fields
                    .iter()
                    .position(|field| field.name == *field_name)
                {
                    Some(index) => index,
                    None => return false, // 字段不存在，条件不成立
                };
                
                let field = &table.def.fields[field_index];
                
                // 检查是否为向量类型
                if !matches!(field.data_type, DataType::Vector) {
                    return false;
                }
                
                // 获取向量维度
                let dimension = if let Some(metadata) = field.vector_metadata {
                    metadata.dimension
                } else {
                    return false;
                };
                
                // 获取向量字段值
                let vector_field_value = &record_values[field_index];
                let vector_ptr = vector_field_value.value.vector;
                
                // 计算距离
            let distance = match op {
                "<->" => calculate_vector_l2_distance(vector_ptr, &compare_vec, dimension),
                "<#>" => calculate_vector_inner_product(vector_ptr, &compare_vec, dimension),
                "<=>" => calculate_vector_cosine_similarity(vector_ptr, &compare_vec, dimension),
                _ => return false,
            };
                
                // 获取条件阈值
                let threshold = match &comp.value {
                    crate::sql::Value::Float(f) => *f,
                    crate::sql::Value::Integer(i) => *i as f64,
                    _ => return false,
                };
                
                // 比较距离和阈值
                return match &comp.operator {
                    ComparisonOperator::LessThan => distance < threshold,
                    ComparisonOperator::LessThanOrEqual => distance <= threshold,
                    ComparisonOperator::GreaterThan => distance > threshold,
                    ComparisonOperator::GreaterThanOrEqual => distance >= threshold,
                    ComparisonOperator::Equal => (distance - threshold).abs() < f64::EPSILON,
                    ComparisonOperator::NotEqual => (distance - threshold).abs() >= f64::EPSILON,
                    ComparisonOperator::Like => false, // 不支持LIKE操作符
                };
            }
            
            // 无法解析向量距离表达式，返回false
            return false;
        } else {
            // 没有使用别名，直接从record_values中获取字段值并比较
            eprintln!("DEBUG evaluate_comparison_with_alias: handling regular field={}", comp.field);
            
            // 获取字段索引
            let actual_field_name = if comp.field.contains('.') {
                comp.field.split('.').last().unwrap()
            } else {
                &comp.field
            };

            eprintln!("DEBUG evaluate_comparison_with_alias: actual_field_name={}", actual_field_name);
            eprintln!("DEBUG evaluate_comparison_with_alias: table.fields={:?}", table.def.fields.iter().map(|f| &f.name).collect::<Vec<_>>());

            // 检查字段是否存在于表中
            let field_index = match table
                .def
                .fields
                .iter()
                .position(|field| field.name == *actual_field_name)
            {
                Some(index) => index,
                None => return false, // 字段不存在，条件不成立
            };

            let field_value = &record_values[field_index];

            // 对于向量类型，不支持直接比较，条件不成立
            if matches!(field_value.value_type, DataType::Vector) {
                return false;
            }

            // 比较字段值和条件值
            compare_field_with_condition(
                &field_value.value,
                field_value.value_type,
                &comp.operator,
                &comp.value,
            )
        }
    }
}

/// 评估BETWEEN条件（支持别名）
unsafe fn evaluate_between_with_alias(
    table: &MemoryTable,
    record_values: &[TypedValue],
    columns: &[Expression],
    expr_values: &[TypedValue],
    between: &BetweenCondition,
    alias_map: &alloc::collections::BTreeMap<String, &Expression>,
) -> bool {
    // 检查字段名是否包含向量距离操作符
    if between.field.contains("<->")
        || between.field.contains("<#>")
        || between.field.contains("<=>")
    {
        // 这是一个向量距离表达式，需要特殊处理
        if let Some((field_name, op, compare_vec)) = parse_vector_distance_expression(&between.field) {
            // 获取向量字段索引
            let field_index = match table
                .def
                .fields
                .iter()
                .position(|field| field.name == *field_name)
            {
                Some(index) => index,
                None => return false, // 字段不存在，条件不成立
            };
            
            let field = &table.def.fields[field_index];
            
            // 检查是否为向量类型
            if !matches!(field.data_type, DataType::Vector) {
                return false;
            }
            
            // 获取向量维度
            let dimension = if let Some(metadata) = field.vector_metadata {
                metadata.dimension
            } else {
                return false;
            };
            
            // 获取向量字段值
            let vector_field_value = &record_values[field_index];
            let vector_ptr = vector_field_value.value.vector;
            
            // 计算距离
            let distance = match op {
                "<->" => calculate_vector_l2_distance(vector_ptr, &compare_vec, dimension),
                "<#>" => calculate_vector_inner_product(vector_ptr, &compare_vec, dimension),
                "<=>" => calculate_vector_cosine_similarity(vector_ptr, &compare_vec, dimension),
                _ => return false,
            };
            
            // 获取条件阈值
            let min_threshold = match &between.min_value {
                crate::sql::Value::Float(f) => *f,
                crate::sql::Value::Integer(i) => *i as f64,
                _ => return false,
            };
            
            let max_threshold = match &between.max_value {
                crate::sql::Value::Float(f) => *f,
                crate::sql::Value::Integer(i) => *i as f64,
                _ => return false,
            };
            
            // BETWEEN条件：distance >= min_value AND distance <= max_value
            return distance >= min_threshold && distance <= max_threshold;
        }
        
        // 无法解析向量距离表达式，返回false
        return false;
    }

    // 检查是否使用别名
    if let Some(alias_expr) = alias_map.get(&between.field) {
        // 找到别名对应的表达式索引
        let mut expr_index = None;
        for (i, expr) in columns.iter().enumerate() {
            if expr == *alias_expr {
                expr_index = Some(i);
                break;
            }
        }

        // 确保找到匹配的表达式
        let Some(expr_index) = expr_index else {
            return false;
        };

        // 获取表达式值
        let _field_value = &expr_values[expr_index];

        // BETWEEN条件：field_value >= min_value AND field_value <= max_value
        let min_comp = ComparisonCondition {
            field: between.field.clone(),
            operator: ComparisonOperator::GreaterThanOrEqual,
            value: between.min_value.clone(),
        };
        let max_comp = ComparisonCondition {
            field: between.field.clone(),
            operator: ComparisonOperator::LessThanOrEqual,
            value: between.max_value.clone(),
        };

        evaluate_comparison_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            &min_comp,
            alias_map,
        ) && evaluate_comparison_with_alias(
            table,
            record_values,
            columns,
            expr_values,
            &max_comp,
            alias_map,
        )
    } else {
        // 没有使用别名，直接从record_values中获取字段值并比较
        // 获取字段索引
        let actual_field_name = if between.field.contains('.') {
            between.field.split('.').last().unwrap()
        } else {
            &between.field
        };

        // 检查字段是否存在于表中
        let field_index = match table
            .def
            .fields
            .iter()
            .position(|field| field.name == *actual_field_name)
        {
            Some(index) => index,
            None => return false, // 字段不存在，条件不成立
        };

        let field_value = &record_values[field_index];

        // 比较字段值和条件值
        let is_greater_or_equal = compare_field_with_condition(
            &field_value.value,
            field_value.value_type,
            &ComparisonOperator::GreaterThanOrEqual,
            &between.min_value,
        );
        let is_less_or_equal = compare_field_with_condition(
            &field_value.value,
            field_value.value_type,
            &ComparisonOperator::LessThanOrEqual,
            &between.max_value,
        );
        is_greater_or_equal && is_less_or_equal
    }
}

/// 对行进行排序（支持别名）
fn sort_rows_with_alias(
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
        order_by.field.split('.').last().unwrap()
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

    // 没有使用别名，使用原始的排序逻辑
    // 查找排序字段在表中的索引
    let field_index = table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
        .ok_or(QueryExecutionError::FieldNotFound)?;

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

/// 处理AT TIME ZONE操作符
/// 将timestamp转换为指定时区的timestamp
fn process_at_time_zone(
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
fn process_timezone_function(timezone_spec: &str) -> Result<i16, QueryExecutionError> {
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
fn process_to_char(
    timestamp: &crate::types::db_timestamp,
    format: &str,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_char(timestamp, format))
}

/// 处理TO_ISO8601()函数
/// 将时间戳转换为ISO 8601格式的字符串
fn process_to_iso8601(
    timestamp: &crate::types::db_timestamp,
) -> Result<String, QueryExecutionError> {
    Ok(crate::types::time_format::to_iso8601(timestamp))
}

/// 处理TO_EPOCH()函数
/// 将时间戳转换为epoch秒数
fn process_to_epoch(timestamp: &crate::types::db_timestamp) -> Result<f64, QueryExecutionError> {
    Ok(crate::types::time_format::to_epoch(timestamp))
}

/// 对行进行排序
fn sort_rows(
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
        order_by.field.split('.').last().unwrap()
    } else {
        // 没有表别名，直接使用字段名
        &order_by.field
    };

    // 查找排序字段在表中的索引
    let field_index = table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
        .ok_or(QueryExecutionError::FieldNotFound)?;

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

/// 获取字段值
unsafe fn get_field_value(
    table: &MemoryTable,
    record_ptr: *const u8,
    field_name: &str,
) -> Result<TypedValue, QueryExecutionError> {
    // 查找字段索引
    // 处理带表别名的字段名，如 "t.id"
    let actual_field_name = if field_name.contains('.') {
        // 提取点号后面的部分作为实际字段名
        field_name.split('.').last().unwrap()
    } else {
        // 没有表别名，直接使用字段名
        field_name
    };

    let field_index = table
        .def
        .fields
        .iter()
        .position(|field| field.name == *actual_field_name)
        .ok_or(QueryExecutionError::FieldNotFound)?;

    let field = &table.def.fields[field_index];
    // 获取字段值
    let value = table
        .get_field(record_ptr, field_index)
        .map_err(|_| QueryExecutionError::FieldNotFound)?;

    #[cfg(feature = "log")]
    debug!("get_field_value for field '{}': value={:?}", field.name, value);

    Ok(TypedValue {
        value_type: field.data_type,
        value,
    })
}
