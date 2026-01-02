//! SQL查询执行器
//! 
//! 该模块负责执行SQL查询并返回结果集。

use alloc::string::String;
use alloc::vec::Vec;

use crate::{RemDb, MemoryTable, Value, RemDbError, types::DataType, IndexType, DdlExecutor};
use crate::sql::{SqlQuery, ResultSet, Condition, ComparisonCondition, ComparisonOperator, OrderByClause};

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
        }
    }
}

impl core::error::Error for QueryExecutionError {} 

/// 执行SQL查询
pub fn execute_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    match query.query_type {
        crate::sql::QueryType::Select => execute_select_query(db, query),
        crate::sql::QueryType::Insert => execute_insert_query(db, query),
        crate::sql::QueryType::Update => execute_update_query(db, query),
        crate::sql::QueryType::Delete => execute_delete_query(db, query),
        crate::sql::QueryType::Describe => execute_describe_query(db, query),
        crate::sql::QueryType::CreateTable => execute_create_table_query(db, query),
        crate::sql::QueryType::CreateIndex => execute_create_index_query(db, query),
        _ => Err(QueryExecutionError::InternalError),
    }
}

/// 执行SELECT查询
fn execute_select_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表
    let table = find_table_by_name(db, &query.table_name)?;
    
    // 2. 确定要返回的列
    let columns = if query.select_all {
        // 返回所有列
        table.def.fields
            .iter()
            .map(|field| field.name.to_string())
            .collect()
    } else {
        // 返回指定列
        validate_columns(table, &query.columns)?;
        query.columns.clone()
    };
    
    // 3. 创建结果集
    let mut result_set = ResultSet::new(columns.clone());
    
    // 4. 遍历表中的所有记录，收集匹配的记录
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
                let mut row_data = Vec::with_capacity(columns.len());
                for column_name in &columns {
                    match get_field_value(table, record_ptr, column_name) {
                        Ok(value) => row_data.push(value),
                        Err(_) => return true, // 跳过错误记录，继续遍历
                    }
                }
                
                // 将匹配的记录添加到向量中
                matched_rows.push(row_data);
            }
            
            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
    }
    
    // 5. 如果有ORDER BY子句，对记录进行排序
    if let Some(order_by) = &query.order_by {
        sort_rows(&mut matched_rows, table, order_by)?;
    }
    
    // 6. 应用LIMIT限制
    let limit = query.limit.unwrap_or(matched_rows.len());
    let rows_to_add = &matched_rows[..core::cmp::min(matched_rows.len(), limit)];
    
    // 7. 将处理后的记录添加到结果集
    for row_data in rows_to_add {
        result_set.add_row(row_data.clone());
    }
    
    Ok(result_set)
}

/// 执行CREATE TABLE查询
fn execute_create_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL数据类型转换为RemDb DataType
    let mut fields = Vec::new();
    let mut field_constraints = Vec::new(); // 存储约束信息
    
    for (field_name, data_type_str, is_primary_key, is_not_null, is_unique, is_auto_increment) in &query.table_def {
        let data_type = match data_type_str.to_uppercase().as_str() {
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
            
            // 字符串类型
            "STRING" | "TEXT" | "VARCHAR" | "NVARCHAR" | "CHAR" | "CLOB" => DataType::String,
            
            _ => return Err(QueryExecutionError::TypeMismatch),
        };
        
        // 保存字段和约束信息
        fields.push((field_name.as_str(), data_type));
        field_constraints.push((is_primary_key, is_not_null, is_unique, is_auto_increment));
    }
    
    // 查找主键字段索引
    let primary_key_index = query.primary_key.as_ref().and_then(|pk| {
        query.table_def.iter().position(|(name, _, _, _, _, _)| name == pk)
    });
    
    // 调用DdlExecutor的create_table方法
    // 注意：这里暂时只传递字段名和类型，约束信息将在表创建后更新
    db.create_table(
        &query.table_name,
        &fields,
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
    
    // 查找创建的表
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
    
    // 更新字段约束信息
    if let Some(table) = &mut db.tables[table_id] {
        // 注意：这里我们无法直接修改field_defs，因为它们是静态的
        // 所以我们需要修改RemDb的create_table实现，使其支持从SQL解析约束信息
        // 目前暂时不支持直接从SQL更新约束，只支持通过DDL宏定义约束
    }
    
    // 创建结果集，返回成功消息
    let columns = vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(vec![Value { string: [b'0'; 64] }]);
    
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
    result_set.add_row(vec![Value { string: [b'0'; 64] }]);
    
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

/// 验证列名是否有效
fn validate_columns(table: &MemoryTable, columns: &[String]) -> Result<(), QueryExecutionError> {
    for column in columns {
        if !table.def.fields.iter().any(|field| field.name == column) {
            return Err(QueryExecutionError::FieldNotFound);
        }
    }
    
    Ok(())
}



/// 执行DESCRIBE TABLE查询
fn execute_describe_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表
    let table = find_table_by_name(db, &query.table_name)?;
    
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
    for field in table.def.fields {
        // 确定是否为主键
        let is_primary_key = table.def.primary_key < table.def.fields.len() && 
                             table.def.fields[table.def.primary_key].name == field.name;
        let key_str = if is_primary_key {
            "PRI"
        } else if field.unique {
            "UNI"
        } else {
            ""
        };
        
        // 确定是否允许NULL
        let null_str = "NO"; // 目前所有字段都不允许NULL
        
        // 确定默认值
        let default_str = "0";
        
        // 确定字段类型字符串表示
        let type_str = match field.data_type {
            crate::DataType::UInt32 => "int".to_string(),
            crate::DataType::Int32 => "int".to_string(),
            crate::DataType::String => format!("varchar({})", field.size),
            crate::DataType::UInt8 => "tinyint".to_string(),
            crate::DataType::Int8 => "tinyint".to_string(),
            crate::DataType::Bool => "bool".to_string(),
            crate::DataType::Timestamp => "timestamp".to_string(),
            crate::DataType::Float32 => "float".to_string(),
            crate::DataType::Float64 => "double".to_string(),
            _ => "unknown".to_string(),
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
        
        let mut type_val = crate::Value { string: [0u8; 64] };
        let type_bytes = type_str.as_bytes();
        let type_len = core::cmp::min(type_bytes.len(), 63);
        unsafe {
            type_val.string[..type_len].copy_from_slice(&type_bytes[..type_len]);
        }
        
        let mut key_val = crate::Value { string: [0u8; 64] };
        let key_bytes = key_str.as_bytes();
        let key_len = core::cmp::min(key_bytes.len(), 63);
        unsafe {
            key_val.string[..key_len].copy_from_slice(&key_bytes[..key_len]);
        }
        
        let mut null_val = crate::Value { string: [0u8; 64] };
        let null_bytes = null_str.as_bytes();
        let null_len = core::cmp::min(null_bytes.len(), 63);
        unsafe {
            null_val.string[..null_len].copy_from_slice(&null_bytes[..null_len]);
        }
        
        let mut default_val = crate::Value { string: [0u8; 64] };
        let default_bytes = default_str.as_bytes();
        let default_len = core::cmp::min(default_bytes.len(), 63);
        unsafe {
            default_val.string[..default_len].copy_from_slice(&default_bytes[..default_len]);
        }
        
        let row_data = vec![
            field_name_val, // Field name
            type_val,       // Type
            key_val,        // Key
            null_val,       // Null
            default_val,    // Default
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
            }
        }
        
        // 7. 调用表的插入方法
        match table.insert(record_data.as_ptr()) {
            Ok(_) => affected_rows += 1,
            Err(e) => {
                match e {
                    RemDbError::InvalidRecordSize | RemDbError::DuplicateKey | RemDbError::TypeMismatch => {
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
    
    let row_data = vec![crate::Value { u64: affected_rows as u64 }];
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
    
    let row_data = vec![crate::Value { u64: affected_rows as u64 }];
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
    
    let row_data = vec![crate::Value { u64: affected_rows as u64 }];
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
                let value = to_integer(sql_value)? as u64;
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u64, value);
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
        }
    }
    
    Ok(())
}

/// 评估条件
unsafe fn evaluate_condition(table: &MemoryTable, record_ptr: *const u8, condition: &Condition) -> bool {
    match condition {
        Condition::Comparison(comp) => evaluate_comparison(table, record_ptr, comp),
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
            compare_values(&field_value, field_type, &comp.operator, &comp.value)
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
            let f_val = unsafe { field_value.timestamp }; // 读取timestamp字段
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

/// 对行进行排序
fn sort_rows(rows: &mut Vec<Vec<Value>>, table: &MemoryTable, order_by: &OrderByClause) -> Result<(), QueryExecutionError> {
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
                let a_val = unsafe { val_a.u8 };
                let b_val = unsafe { val_b.u8 };
                a_val.cmp(&b_val)
            },
            DataType::UInt16 => {
                let a_val = unsafe { val_a.u16 };
                let b_val = unsafe { val_b.u16 };
                a_val.cmp(&b_val)
            },
            DataType::UInt32 => {
                let a_val = unsafe { val_a.u32 };
                let b_val = unsafe { val_b.u32 };
                a_val.cmp(&b_val)
            },
            DataType::UInt64 => {
                let a_val = unsafe { val_a.u64 };
                let b_val = unsafe { val_b.u64 };
                a_val.cmp(&b_val)
            },
            
            // 有符号整数类型
            DataType::Int8 => {
                let a_val = unsafe { val_a.i8 };
                let b_val = unsafe { val_b.i8 };
                a_val.cmp(&b_val)
            },
            DataType::Int16 => {
                let a_val = unsafe { val_a.i16 };
                let b_val = unsafe { val_b.i16 };
                a_val.cmp(&b_val)
            },
            DataType::Int32 => {
                let a_val = unsafe { val_a.i32 };
                let b_val = unsafe { val_b.i32 };
                a_val.cmp(&b_val)
            },
            DataType::Int64 => {
                let a_val = unsafe { val_a.i64 };
                let b_val = unsafe { val_b.i64 };
                a_val.cmp(&b_val)
            },
            
            // 浮点数类型
            DataType::Float32 => {
                let a_val = unsafe { val_a.float32 };
                let b_val = unsafe { val_b.float32 };
                a_val.partial_cmp(&b_val).unwrap_or(core::cmp::Ordering::Equal)
            },
            DataType::Float64 => {
                let a_val = unsafe { val_a.float64 };
                let b_val = unsafe { val_b.float64 };
                a_val.partial_cmp(&b_val).unwrap_or(core::cmp::Ordering::Equal)
            },
            
            // 布尔类型
            DataType::Bool => {
                let a_val = unsafe { val_a.bool };
                let b_val = unsafe { val_b.bool };
                a_val.cmp(&b_val)
            },
            
            // 时间戳类型
            DataType::Timestamp => {
                let a_val = unsafe { val_a.timestamp };
                let b_val = unsafe { val_b.timestamp };
                a_val.cmp(&b_val)
            },
            
            // 字符串类型
            DataType::String => {
                let a_str = unsafe { &val_a.string };
                let b_str = unsafe { &val_b.string };
                
                let a_str = String::from_utf8_lossy(a_str).trim_end_matches(char::from(0)).to_string();
                let b_str = String::from_utf8_lossy(b_str).trim_end_matches(char::from(0)).to_string();
                
                a_str.cmp(&b_str)
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
unsafe fn get_field_value(table: &MemoryTable, record_ptr: *const u8, field_name: &str) -> Result<Value, QueryExecutionError> {
    // 查找字段索引
    let field_index = table.def.fields
        .iter()
        .position(|field| field.name == field_name)
        .ok_or(QueryExecutionError::FieldNotFound)?;
    
    // 获取字段值
    table.get_field(record_ptr, field_index)
        .map_err(|_| QueryExecutionError::FieldNotFound)
}
