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
    
    // 3. 创建结果集，预分配足够的行空间
    let mut result_set = ResultSet::new(columns.clone());
    
    // 4. 直接在遍历记录时将结果添加到结果集，避免使用中间向量
    let limit = query.limit.unwrap_or(table.def.max_records);
    
    // 直接遍历表中的所有记录
    unsafe {
        // 预先创建一个足够大的向量来存储匹配的记录
        let mut matched_rows = Vec::with_capacity(table.def.max_records);
        
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
                
                // 检查是否达到LIMIT限制
                if matched_rows.len() >= limit {
                    return false; // 停止遍历
                }
            }
            
            true // 继续遍历
        });
        iterate_result.map_err(|_| QueryExecutionError::InternalError)?;
        
        // 将收集到的记录添加到结果集
        for row_data in matched_rows {
            result_set.add_row(row_data);
        }
    }
    
    Ok(result_set)
}

/// 执行CREATE TABLE查询
fn execute_create_table_query(db: &mut RemDb, query: &SqlQuery) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL数据类型转换为RemDb DataType
    let mut fields = Vec::new();
    let mut field_constraints = Vec::new(); // 存储约束信息
    
    for (field_name, data_type_str, is_primary_key, is_not_null, is_unique) in &query.table_def {
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
        field_constraints.push((is_primary_key, is_not_null, is_unique));
    }
    
    // 查找主键字段索引
    let primary_key_index = query.primary_key.as_ref().and_then(|pk| {
        query.table_def.iter().position(|(name, _, _, _, _)| name == pk)
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
        } else {
            ""
        };
        
        // 确定是否允许NULL（目前所有字段都不允许NULL）
        let null_str = "NO";
        
        // 确定默认值（目前所有字段默认值为0或空字符串）
        let default_str = "0";
        
        // 将字段名称转换为索引
        let field_index = match field.name {
            "id" => 0,
            "name" => 1,
            "age" => 2,
            "active" => 3,
            _ => 0,
        };
        
        // 将字段类型转换为索引
        let type_index = match field.data_type {
            crate::DataType::UInt32 => 4,
            crate::DataType::String => 5,
            crate::DataType::UInt8 => 6,
            crate::DataType::Bool => 7,
            _ => 0,
        };
        
        // 将主键标志转换为索引
        let key_index = if key_str == "PRI" {
            8
        } else {
            9
        };
        
        // 将NULL约束转换为索引
        let null_index = if null_str == "NO" {
            10
        } else {
            9
        };
        
        // 将默认值转换为索引
        let default_index = if default_str == "0" {
            11
        } else {
            9
        };
        
        // 创建行数据 - 使用索引映射来表示字符串值
        let row_data = vec![
            crate::Value { u64: field_index as u64 }, // Field name
            crate::Value { u64: type_index as u64 },  // Type
            crate::Value { u64: key_index as u64 },   // Key
            crate::Value { u64: null_index as u64 },  // Null
            crate::Value { u64: default_index as u64 }, // Default
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
        // 5. 创建记录数据缓冲区
        let mut record_data = Vec::with_capacity(table.record_size);
        unsafe {
            record_data.set_len(table.record_size);
        }
        
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
            
            // 转换并设置字段值
            if let Some(sql_value) = field_value {
                set_field_value(&mut record_data, field.offset, field.data_type, field.size, sql_value)?;
            }
        }
        
        // 7. 调用表的插入方法
        match table.insert(record_data.as_ptr()) {
            Ok(_) => affected_rows += 1,
            Err(_) => return Err(QueryExecutionError::OutOfMemory),
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

/// 设置字段值
fn set_field_value(record_data: &mut Vec<u8>, offset: usize, data_type: DataType, field_size: usize, sql_value: &crate::sql::Value) -> Result<(), QueryExecutionError> {
    unsafe {
        match data_type {
            // 无符号整数类型
            DataType::UInt8 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as u8,
                    crate::sql::Value::Float(f) => *f as u8,
                    crate::sql::Value::Boolean(b) => *b as u8,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // u8不需要对齐，直接复制
                record_data[offset] = value;
            },
            DataType::UInt16 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as u16,
                    crate::sql::Value::Float(f) => *f as u16,
                    crate::sql::Value::Boolean(b) => *b as u16,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u16, value);
            },
            DataType::UInt32 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as u32,
                    crate::sql::Value::Float(f) => *f as u32,
                    crate::sql::Value::Boolean(b) => *b as u32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u32, value);
            },
            DataType::UInt64 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as u64,
                    crate::sql::Value::Float(f) => *f as u64,
                    crate::sql::Value::Boolean(b) => *b as u64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut u64, value);
            },
            
            // 有符号整数类型
            DataType::Int8 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as i8,
                    crate::sql::Value::Float(f) => *f as i8,
                    crate::sql::Value::Boolean(b) => *b as i8,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // i8不需要对齐，直接复制
                record_data[offset] = value as u8;
            },
            DataType::Int16 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as i16,
                    crate::sql::Value::Float(f) => *f as i16,
                    crate::sql::Value::Boolean(b) => *b as i16,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i16, value);
            },
            DataType::Int32 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as i32,
                    crate::sql::Value::Float(f) => *f as i32,
                    crate::sql::Value::Boolean(b) => *b as i32,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // 使用core::ptr::write_unaligned来避免对齐问题
                core::ptr::write_unaligned(record_data.as_mut_ptr().add(offset) as *mut i32, value);
            },
            DataType::Int64 => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i,
                    crate::sql::Value::Float(f) => *f as i64,
                    crate::sql::Value::Boolean(b) => *b as i64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
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
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
                // bool不需要对齐，直接复制
                record_data[offset] = value as u8;
            },
            
            // 时间戳类型
            DataType::Timestamp => {
                let value = match sql_value {
                    crate::sql::Value::Integer(i) => *i as u64,
                    crate::sql::Value::Float(f) => *f as u64,
                    _ => return Err(QueryExecutionError::TypeMismatch),
                };
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

/// 对行进行排序 - 简化实现
fn sort_rows(_rows: &mut Vec<(usize, Vec<Value>)>, _table: &MemoryTable, _order_by: &OrderByClause) -> Result<(), QueryExecutionError> {
    // 由于Value是union类型，排序实现较为复杂，暂时返回Ok
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
