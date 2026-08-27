//! SQL DDL (Data Definition Language) Operations
//!
//! This module contains DDL operations like CREATE/DROP TABLE, DATABASE, INDEX, etc.

use crate::sql::{QueryExecutionError, ResultSet, SqlQuery};
use crate::sql::parse_data_type_with_precision;
use crate::types::{DataType, TypedValue};
use crate::{DdlExecutor, IndexType, RemDb, RemDbError, TableDef, Value, MAX_STRING_LEN};
use crate::try_lock;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "log")]
use crate::log::debug;

/// 执行DROP TABLE查询
pub fn execute_drop_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
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
pub fn execute_create_database_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的create_database方法
    db.create_database(&database_name)
        .map_err(|err| match err {
            crate::RemDbError::DatabaseExists => QueryExecutionError::DatabaseExists,
            _ => QueryExecutionError::InternalError,
        })?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行USE DATABASE查询
pub fn execute_use_database_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的use_database方法
    db.use_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行CLOSE DATABASE查询
pub fn execute_close_database_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的close_database方法
    db.close_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}

/// 执行DROP DATABASE查询
pub fn execute_drop_database_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 提取数据库名称
    let database_name = query.table_name.clone();

    // 调用RemDb的drop_database方法
    db.drop_database(&database_name)
        .map_err(|_| QueryExecutionError::InternalError)?;

    // 返回空结果集
    Ok(ResultSet::new(Vec::new()))
}
/// 执行CREATE CHECKPOINT查询
pub fn execute_create_checkpoint_query(db: &mut RemDb) -> Result<ResultSet, QueryExecutionError> {
    unsafe {
        db.checkpoint()
            .map_err(|_| QueryExecutionError::InternalError)?;
    }

    // 创建结果集，返回成功消息
    let columns = alloc::vec!["status".to_string()];
    let mut result_set = ResultSet::new(columns);

    // 创建一个表示成功的消息
    let mut success_msg = [b'0'; 64];
    let msg = "Checkpoint created successfully";
    let msg_bytes = msg.as_bytes();
    let copy_len = core::cmp::min(msg_bytes.len(), 63);
    success_msg[..copy_len].copy_from_slice(&msg_bytes[..copy_len]);
    success_msg[copy_len] = 0;

    result_set.add_row(alloc::vec![TypedValue {
        value_type: crate::DataType::VarChar,
        value: crate::Value {
            string: success_msg
        },
    }]);

    Ok(result_set)
}

/// 执行CREATE TIMESERIES TABLE查询
pub fn execute_create_time_series_table_query(
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
        let base_type = data_type_str
            .split('(')
            .next()
            .unwrap_or(data_type_str)
            .trim();
        let base_type_upper = base_type.to_uppercase();
        #[cfg(feature = "log")]
        debug!(
            "Base type: '{}', upper case: '{}'",
            base_type, base_type_upper
        );

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

/// 执行DESCRIBE TABLE查询
pub fn execute_describe_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 1. 查找要查询的表定义（同时检查普通表和时序表）
    let mut found_table_def: Option<Arc<TableDef>> = None;

    // 查找普通表
    for table in db.tables.iter().flatten() {
        if table.def.name == query.table_name {
            found_table_def = Some(table.def.clone());
            break;
        }
    }

    // 如果普通表未找到，查找时序表
    if found_table_def.is_none() {
        for ts_table in db.time_series_tables.iter().flatten() {
            if ts_table.def.base.name == query.table_name {
                found_table_def = Some(alloc::sync::Arc::new(ts_table.def.base.clone()));
                break;
            }
        }
    }

    // 如果都未找到，返回错误
    let table_def = found_table_def.ok_or(QueryExecutionError::TableNotFound)?;

    // 2. 定义结果集列名
    let columns = alloc::vec![
        "column_name".to_string(),
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
        debug!(
            "describe table {}: id={}, name={}, fields_len={}, primary_key_len={}",
            query.table_name,
            table_def.id,
            table_def.name,
            table_def.fields.len(),
            table_def.primary_key.len()
        );
        for (i, field) in table_def.fields.iter().enumerate() {
            debug!(
                "field {}: name={}, data_type={:?}, size={}, offset={}",
                i, field.name, field.data_type, field.size, field.offset
            );
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
                DataType::Json => "<json>".to_string(),
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
            crate::DataType::Json => "json".to_string(),
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

/// 执行CREATE INDEX查询
pub fn execute_create_index_query(
    _db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // 将SQL索引类型转换为RemDb IndexType
    let index_type = match query.index_type.as_deref() {
        Some("BTREE") => IndexType::BTree,
        Some("TTREE") => IndexType::TTree,
        Some("SORTEDARRAY") => IndexType::SortedArray,
        Some("HNSW") | Some("HNSW_SQ") | Some("HNSW_BQ") | Some("IVF") | Some("IVF_PQ")
        | Some("IVF_FLAT") | Some("VECTOR") => IndexType::Vector,
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
            Some("HNSW_SQ") => Some(crate::types::VectorIndexType::HNSW_SQ),
            Some("HNSW_BQ") => Some(crate::types::VectorIndexType::HNSW_BQ),
            Some("IVF") | Some("IVF_FLAT") => Some(crate::types::VectorIndexType::IVF),
            Some("IVF_PQ") => Some(crate::types::VectorIndexType::IVF_PQ),
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

    // 获取索引构建线程池，如果不可用则返回默认结果集
    let task_id = match crate::index::builder::get_index_build_thread_pool() {
        Ok(thread_pool) => {
            thread_pool.submit_task(
                query.table_name.clone(),
                field_name.clone(),                         // 直接克隆 Vec<String>
                crate::sql::query_parser::IndexType::BTree, // 使用默认值，实际索引类型由params指定
                params,
            )
        }
        Err(_) => {
            // 线程池不可用，返回 task_id = 0
            0
        }
    };

    // 创建结果集，返回任务ID
    let columns = alloc::vec!["task_id".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: DataType::UInt64,
        value: Value {
            u64: task_id as u64
        },
    }]);

    Ok(result_set)
}

/// 执行SHOW TABLES查询
pub fn execute_show_tables_query(db: &mut RemDb) -> Result<ResultSet, QueryExecutionError> {
    let columns = alloc::vec!["table_name".to_string(),];

    let mut result_set = ResultSet::new(columns);

    for table in db.tables.iter().flatten() {
        let row = alloc::vec![TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: {
                    let mut s = [0u8; 64];
                    let bytes = table.def.name.as_bytes();
                    let len = core::cmp::min(bytes.len(), 64);
                    s[..len].copy_from_slice(&bytes[..len]);
                    s
                }
            },
        },];
        result_set.add_row(row);
    }

    for ts_table in db.time_series_tables.iter().flatten() {
        let row = alloc::vec![TypedValue {
            value_type: DataType::VarChar,
            value: Value {
                string: {
                    let mut s = [0u8; 64];
                    let bytes = ts_table.def.base.name.as_bytes();
                    let len = core::cmp::min(bytes.len(), 64);
                    s[..len].copy_from_slice(&bytes[..len]);
                    s
                }
            },
        },];
        result_set.add_row(row);
    }

    Ok(result_set)
}

/// 执行REINDEX查询
pub fn execute_reindex_query(
    _db: &mut RemDb,
    _query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    // REINDEX is currently a no-op - just return success
    let columns = alloc::vec!["result".to_string()];
    let mut result_set = ResultSet::new(columns);
    result_set.add_row(alloc::vec![TypedValue {
        value_type: DataType::VarChar,
        value: Value {
            string: {
                let mut s = [0u8; 64];
                let bytes = b"OK";
                s[..2].copy_from_slice(bytes);
                s
            }
        },
    }]);
    Ok(result_set)
}

/// 执行CREATE INDEX查询
/// 执行SHOW INDEX BUILD STATUS查询
pub fn execute_show_index_build_status_query(
    _db: &mut RemDb,
    _query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
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

    // 获取索引构建线程池，如果不可用则返回空结果集
    if let Ok(thread_pool) = crate::index::builder::get_index_build_thread_pool() {
        // 获取所有索引构建状态
        let status_list = thread_pool.get_build_status(None);

        // 遍历所有状态，添加到结果集
        for status_arc in status_list {
            let status = try_lock!(status_arc);

            // 转换状态为字符串
            let state_str = status.get_state_str();

            // 创建行数据
            let row = alloc::vec![
                TypedValue {
                    value_type: DataType::UInt64,
                    value: Value { u64: status.id },
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
                    value: Value {
                        u64: status.progress.load(core::sync::atomic::Ordering::SeqCst) as u64
                    },
                },
                TypedValue {
                    value_type: DataType::UInt64,
                    value: Value {
                        u64: status
                            .processed_rows
                            .load(core::sync::atomic::Ordering::SeqCst)
                            as u64
                    },
                },
                TypedValue {
                    value_type: DataType::UInt64,
                    value: Value {
                        u64: status.total_rows.load(core::sync::atomic::Ordering::SeqCst) as u64
                    },
                },
                TypedValue {
                    value_type: DataType::UInt64,
                    value: Value {
                        u64: status
                            .elapsed_time
                            .load(core::sync::atomic::Ordering::SeqCst)
                            as u64
                    },
                },
            ];

            result_set.add_row(row);
        }
    }

    Ok(result_set)
}

/// 执行CREATE TABLE查询
pub fn execute_create_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
    #[cfg(feature = "log")]
    debug!(
        "execute_create_table_query called for table: {}",
        query.table_name
    );
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
        let (base_type, precision, _distance_type) = parse_data_type_with_precision(data_type_str)?;

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
                            *i
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
                                    precision as u8,
                                    0,
                                ),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(
                                    actual_value,
                                    0,
                                    precision as u8,
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
                                    precision as u8,
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
                            }
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
                            time: crate::types::db_timestamp::new(*f as i64, 0, precision as u8, 0),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(*f as i64, 0, precision as u8, 0),
                        },
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut s = [0; MAX_STRING_LEN];
                            let str_val = f.to_string();
                            let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                            s[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                            Value { string: s }
                        }
                        DataType::Interval => Value {
                            interval: crate::types::db_interval::new(*f as i64, precision as u8, 0),
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
                        }
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
                            time: crate::types::db_timestamp::new(*b as i64, 0, precision as u8, 0),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(*b as i64, 0, precision as u8, 0),
                        },
                        DataType::VarChar | DataType::Char | DataType::Text => {
                            let mut s = [0; MAX_STRING_LEN];
                            let str_val = b.to_string();
                            let len = core::cmp::min(str_val.len(), MAX_STRING_LEN);
                            s[..len].copy_from_slice(&str_val.as_bytes()[..len]);
                            Value { string: s }
                        }
                        DataType::Interval => Value {
                            interval: crate::types::db_interval::new(*b as i64, precision as u8, 0),
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
                                precision as u8,
                                0,
                            ),
                        },
                        DataType::TimestampTZ => Value {
                            time: crate::types::db_timestamp::new(
                                s.parse().unwrap_or(0) as i64,
                                0,
                                precision as u8,
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
                                precision as u8,
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
                                    precision as u8,
                                    0,
                                ),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(
                                    s.parse().unwrap_or(0) as i64,
                                    0,
                                    precision as u8,
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
                                    precision as u8,
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
                                time: crate::types::db_timestamp::new(0, 0, precision as u8, 0),
                            },
                            DataType::TimestampTZ => Value {
                                time: crate::types::db_timestamp::new(0, 0, precision as u8, 0),
                            },
                            DataType::VarChar | DataType::Char | DataType::Text => Value {
                                string: [0; MAX_STRING_LEN],
                            },
                            DataType::Interval => Value {
                                interval: crate::types::db_interval::new(0, precision as u8, 0),
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
                    },
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

    // 从WITH CONFIGURATION子句中提取max_records配置
    let max_records = query
        .table_config
        .get("MAX_RECORDS")
        .and_then(|v| v.parse::<usize>().ok());

    // 调用DdlExecutor::create_table方法，支持约束和复合主键
    #[cfg(feature = "log")]
    debug!("Before create_table, db.tables.len() = {}", db.tables.len());
    // 使用 create_table_with_constraints 方法传递约束信息
    db.create_table_with_constraints(
        &query.table_name,
        &fields,
        Some(&field_constraints),
        primary_key_indices,
        max_records,
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


/// 执行ALTER TABLE查询
pub fn execute_alter_table_query(
    db: &mut RemDb,
    query: &SqlQuery,
) -> Result<ResultSet, QueryExecutionError> {
            // 处理ALTER TABLE语句
            for (field1, field2, pk, not_null, unique, auto_inc, default_val) in &query.table_def {
                if field2 == "DROP" {
                    // 执行DROP COLUMN操作
                    db.alter_table(
                        &query.table_name,
                        crate::AlterTableOperation::DropColumn {
                            name: field1.clone(),
                        },
                    )
                    .map_err(|_| QueryExecutionError::InternalError)?;
                } else if !field2.is_empty() && field2 != "DROP" {
                    // 检查是否是RENAME COLUMN操作
                    // 通过检查field2是否是有效的数据类型来区分
                    match parse_data_type_with_precision(field2) {
                        Ok(_) => {
                            // field2是有效的数据类型，执行ADD COLUMN或MODIFY COLUMN操作
                            // 解析数据类型
                            let (base_type, size, distance_type) =
                                parse_data_type_with_precision(field2)?;
                            let data_type = match base_type.as_str() {
                                "INT" | "INTEGER" | "BIGINT" | "TINYINT" | "SMALLINT" | "INT16"
                                | "INT32" | "INT64" => crate::types::DataType::Int64,
                                "UINT" | "UINTEGER" | "UBIGINT" | "UTINYINT" | "USMALLINT"
                                | "UINT16" | "UINT32" | "UINT64" => crate::types::DataType::UInt64,
                                "FLOAT" | "DOUBLE" | "REAL" | "FLOAT32" | "FLOAT64" => {
                                    crate::types::DataType::Float32
                                }
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

                            let field_exists = existing_table
                                .map(|table_opt| {
                                    if let Some(table) = table_opt {
                                        table.def.fields.iter().any(|f| f.name == *field1)
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false);

                            // 转换默认值类型：query_parser::Value -> types::Value
                            let types_default_value =
                                default_val.as_ref().map(|qp_val| match qp_val {
                                    crate::sql::query_parser::Value::Integer(i) => {
                                        crate::types::Value { i64: *i }
                                    }
                                    crate::sql::query_parser::Value::Float(f) => {
                                        crate::types::Value { float32: *f as f32 }
                                    }
                                    crate::sql::query_parser::Value::String(s) => {
                                        let mut string_val =
                                            crate::types::Value { string: [0u8; 64] };
                                        unsafe {
                                            let s_bytes = s.as_bytes();
                                            let dest = &mut string_val.string as *mut u8;
                                            let src = s_bytes.as_ptr();
                                            let copy_size = core::cmp::min(s_bytes.len(), 64);
                                            core::ptr::copy_nonoverlapping(src, dest, copy_size);
                                        }
                                        string_val
                                    }
                                    crate::sql::query_parser::Value::Boolean(b) => {
                                        crate::types::Value { bool: *b }
                                    }
                                    _ => crate::types::Value { i64: 0 },
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
                                )
                                .map_err(|_| QueryExecutionError::InternalError)?;
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
                                )
                                .map_err(|_| QueryExecutionError::InternalError)?;
                            }
                        }
                        Err(_) => {
                            // field2不是有效的数据类型，执行RENAME COLUMN操作
                            db.alter_table(
                                &query.table_name,
                                crate::AlterTableOperation::RenameColumn {
                                    old_name: field1.clone(),
                                    new_name: field2.clone(),
                                },
                            )
                            .map_err(|_| QueryExecutionError::InternalError)?;
                        }
                    }
                }
            }
    Ok(ResultSet::new(Vec::new()))
}
