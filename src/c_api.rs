#![allow(non_snake_case)]

use crate::types::{DataType, FieldDef, TableDef, Value};
use crate::config::DbConfig;
use crate::transaction::{TransactionType, IsolationLevel};

/// C API: 数据类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbDataType {
    Int8 = 0,
    Int16 = 1,
    Int32 = 2,
    Int64 = 3,
    Float32 = 4,
    Float64 = 5,
    Bool = 6,
    Timestamp = 7,
    String = 8,
}

impl From<RemDbDataType> for DataType {
    fn from(c_type: RemDbDataType) -> Self {
        match c_type {
            RemDbDataType::Int8 => DataType::Int8,
            RemDbDataType::Int16 => DataType::Int16,
            RemDbDataType::Int32 => DataType::Int32,
            RemDbDataType::Int64 => DataType::Int64,
            RemDbDataType::Float32 => DataType::Float32,
            RemDbDataType::Float64 => DataType::Float64,
            RemDbDataType::Bool => DataType::Bool,
            RemDbDataType::Timestamp => DataType::Timestamp,
            RemDbDataType::String => DataType::String,
        }
    }
}

/// C API: 最大字符串长度
pub const REMDB_MAX_STRING_LEN: usize = 64;

/// C API: 通用值类型
#[repr(C)]
pub union RemDbValue {
    pub int8: i8,
    pub int16: i16,
    pub int32: i32,
    pub int64: i64,
    pub float32: f32,
    pub float64: f64,
    pub bool: u8,
    pub timestamp: u64,
    pub string: [u8; REMDB_MAX_STRING_LEN],
}

impl From<Value> for RemDbValue {
    fn from(rust_value: Value) -> Self {
        unsafe {
            match rust_value {
                Value { int8: v } => RemDbValue { int8: v },
                Value { int16: v } => RemDbValue { int16: v },
                Value { int32: v } => RemDbValue { int32: v },
                Value { int64: v } => RemDbValue { int64: v },
                Value { float32: v } => RemDbValue { float32: v },
                Value { float64: v } => RemDbValue { float64: v },
                Value { bool: v } => RemDbValue { bool: v as u8 },
                Value { timestamp: v } => RemDbValue { timestamp: v },
                Value { string: v } => {
                    let mut c_str = [0u8; REMDB_MAX_STRING_LEN];
                    c_str.copy_from_slice(&v);
                    RemDbValue { string: c_str }
                },
            }
        }
    }
}

impl From<RemDbValue> for Value {
    fn from(c_value: RemDbValue) -> Self {
        // 注意：这个转换需要知道具体的数据类型才能安全进行
        // 在实际使用中，应该根据字段的数据类型来选择合适的变体
        // 这里提供一个默认实现，实际使用时需要根据上下文调整
        unsafe {
            Value { int32: c_value.int32 }
        }
    }
}

/// C API: 字段定义
#[repr(C)]
pub struct RemDbFieldDef {
    pub name: *const u8,
    pub data_type: RemDbDataType,
    pub size: usize,
    pub offset: usize,
}

impl From<&FieldDef> for RemDbFieldDef {
    fn from(rust_field: &FieldDef) -> Self {
        RemDbFieldDef {
            name: rust_field.name.as_ptr(),
            data_type: match rust_field.data_type {
                DataType::Int8 => RemDbDataType::Int8,
                DataType::Int16 => RemDbDataType::Int16,
                DataType::Int32 => RemDbDataType::Int32,
                DataType::Int64 => RemDbDataType::Int64,
                DataType::Float32 => RemDbDataType::Float32,
                DataType::Float64 => RemDbDataType::Float64,
                DataType::Bool => RemDbDataType::Bool,
                DataType::Timestamp => RemDbDataType::Timestamp,
                DataType::String => RemDbDataType::String,
            },
            size: rust_field.size,
            offset: rust_field.offset,
        }
    }
}

/// C API: 表定义
#[repr(C)]
pub struct RemDbTableDef {
    pub id: u8,
    pub name: *const u8,
    pub fields: *const RemDbFieldDef,
    pub fields_count: usize,
    pub primary_key: usize,
    pub secondary_index: i32,
    pub record_size: usize,
    pub max_records: usize,
}

/// C API: 数据库配置
#[repr(C)]
pub struct RemDbConfig {
    pub tables: *const RemDbTableDef,
    pub tables_count: usize,
    pub total_memory: usize,
    pub low_power_mode_supported: u8,
    pub low_power_max_records: i32,
}

/// C API: 数据库句柄类型别名
pub type RemDbHandle = *mut crate::RemDb;

/// C API: 事务类型
#[repr(u8)]
pub enum RemDbTransactionType {
    ReadOnly = 0,
    ReadWrite = 1,
}

impl From<RemDbTransactionType> for TransactionType {
    fn from(c_type: RemDbTransactionType) -> Self {
        match c_type {
            RemDbTransactionType::ReadOnly => TransactionType::ReadOnly,
            RemDbTransactionType::ReadWrite => TransactionType::ReadWrite,
        }
    }
}

/// C API: 隔离级别
#[repr(u8)]
pub enum RemDbIsolationLevel {
    ReadUncommitted = 0,
    ReadCommitted = 1,
    RepeatableRead = 2,
    Serializable = 3,
}

impl From<RemDbIsolationLevel> for IsolationLevel {
    fn from(c_level: RemDbIsolationLevel) -> Self {
        match c_level {
            RemDbIsolationLevel::ReadUncommitted => IsolationLevel::ReadUncommitted,
            RemDbIsolationLevel::ReadCommitted => IsolationLevel::ReadCommitted,
            RemDbIsolationLevel::RepeatableRead => IsolationLevel::RepeatableRead,
            RemDbIsolationLevel::Serializable => IsolationLevel::Serializable,
        }
    }
}

/// C API: 数据库指标快照
#[repr(C)]
pub struct RemDbMetricsSnapshot {
    pub total_memory: usize,
    pub used_memory: usize,
    pub read_ops: u64,
    pub write_ops: u64,
    pub delete_ops: u64,
    pub update_ops: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub index_lookups: u64,
    pub index_inserts: u64,
    pub index_deletes: u64,
    pub transactions: u64,
    pub committed_transactions: u64,
    pub rolled_back_transactions: u64,
    pub start_time: u64,
}

impl From<crate::DbMetricsSnapshot> for RemDbMetricsSnapshot {
    fn from(rust_snapshot: crate::DbMetricsSnapshot) -> Self {
        RemDbMetricsSnapshot {
            total_memory: rust_snapshot.total_memory,
            used_memory: rust_snapshot.used_memory,
            read_ops: rust_snapshot.read_ops,
            write_ops: rust_snapshot.write_ops,
            delete_ops: rust_snapshot.delete_ops,
            update_ops: rust_snapshot.update_ops,
            cache_hits: rust_snapshot.cache_hits,
            cache_misses: rust_snapshot.cache_misses,
            index_lookups: rust_snapshot.index_lookups,
            index_inserts: rust_snapshot.index_inserts,
            index_deletes: rust_snapshot.index_deletes,
            transactions: rust_snapshot.transactions,
            committed_transactions: rust_snapshot.committed_transactions,
            rolled_back_transactions: rust_snapshot.rolled_back_transactions,
            start_time: 0, // 占位符，实际值需要根据DbMetricsSnapshot结构体调整
        }
    }
}

/// C API: 健康状态
#[repr(u8)]
pub enum RemDbHealthStatus {
    Healthy = 0,
    Warning = 1,
    Unhealthy = 2,
}

impl From<crate::HealthStatus> for RemDbHealthStatus {
    fn from(rust_status: crate::HealthStatus) -> Self {
        match rust_status {
            crate::HealthStatus::Healthy => RemDbHealthStatus::Healthy,
            crate::HealthStatus::Warning => RemDbHealthStatus::Warning,
            crate::HealthStatus::Unhealthy => RemDbHealthStatus::Unhealthy,
        }
    }
}

/// C API: 健康检查结果
#[repr(C)]
pub struct RemDbHealthCheckResult {
    pub status: RemDbHealthStatus,
    pub metrics: RemDbMetricsSnapshot,
    pub details: *const u8,
}

impl From<crate::HealthCheckResult> for RemDbHealthCheckResult {
    fn from(rust_result: crate::HealthCheckResult) -> Self {
        RemDbHealthCheckResult {
            status: rust_result.status.into(),
            metrics: rust_result.metrics.into(),
            details: rust_result.details.as_ptr(),
        }
    }
}

/// C API: 错误码
#[repr(u32)]
pub enum RemDbError {
    Success = 0,
    OutOfMemory = 1,
    RecordNotFound = 2,
    DuplicateKey = 3,
    FieldNotFound = 4,
    TypeMismatch = 5,
    TransactionError = 6,
    ConfigError = 7,
    UnsupportedOperation = 8,
    FileIoError = 9,
    SnapshotFormatError = 10,
    Crc32Error = 11,
    LogFormatError = 12,
    LogRecordNotFound = 13,
    LogChecksumError = 14,
    LockConflict = 15,
    LockTimeout = 16,
    TableNotFound = 17,
    InvalidRecordSize = 18,
}

impl From<crate::RemDbError> for RemDbError {
    fn from(rust_error: crate::RemDbError) -> Self {
        match rust_error {
            crate::RemDbError::OutOfMemory => RemDbError::OutOfMemory,
            crate::RemDbError::RecordNotFound => RemDbError::RecordNotFound,
            crate::RemDbError::DuplicateKey => RemDbError::DuplicateKey,
            crate::RemDbError::FieldNotFound => RemDbError::FieldNotFound,
            crate::RemDbError::TypeMismatch => RemDbError::TypeMismatch,
            crate::RemDbError::TransactionError => RemDbError::TransactionError,
            crate::RemDbError::ConfigError => RemDbError::ConfigError,
            crate::RemDbError::UnsupportedOperation => RemDbError::UnsupportedOperation,
            crate::RemDbError::FileIoError => RemDbError::FileIoError,
            crate::RemDbError::SnapshotFormatError => RemDbError::SnapshotFormatError,
            crate::RemDbError::Crc32Error => RemDbError::Crc32Error,
            crate::RemDbError::LogFormatError => RemDbError::LogFormatError,
            crate::RemDbError::LogRecordNotFound => RemDbError::LogRecordNotFound,
            crate::RemDbError::LogChecksumError => RemDbError::LogChecksumError,
            crate::RemDbError::LockConflict => RemDbError::LockConflict,
            crate::RemDbError::LockTimeout => RemDbError::LockTimeout,
            crate::RemDbError::TableNotFound => RemDbError::TableNotFound,
            crate::RemDbError::InvalidRecordSize => RemDbError::InvalidRecordSize,
        }
    }
}

/// C API: 从C字符串创建Rust字符串
unsafe fn c_str_to_rust(c_str: *const u8) -> alloc::string::String {
    let mut len = 0;
    while *c_str.offset(len) != 0 {
        len += 1;
    }
    let slice = core::slice::from_raw_parts(c_str, len as usize);
    alloc::string::String::from_utf8_lossy(slice).into_owned()
}

/// C API: 获取C字符串长度
unsafe fn _c_strlen(s: *const u8) -> usize {
    let mut len = 0;
    while *s.offset(len) != 0 {
        len += 1;
    }
    len as usize
}

/// C API: 初始化全局数据库实例
#[no_mangle]
pub unsafe extern "C" fn remdb_init_global(
    config: *const RemDbConfig,
    handle: *mut RemDbHandle,
) -> RemDbError {
    if config.is_null() || handle.is_null() {
        return RemDbError::ConfigError;
    }

    // 将C配置转换为Rust配置
    let c_config = &*config;
    
    // 转换表定义
    let mut rust_tables = Vec::new();
    for i in 0..c_config.tables_count {
        let c_table = &*c_config.tables.offset(i as isize);
        
        // 转换字段定义
        let mut rust_fields = Vec::new();
        for j in 0..c_table.fields_count {
            let c_field = &*c_table.fields.offset(j as isize);
            rust_fields.push(FieldDef {
                name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    c_field.name,
                    _c_strlen(c_field.name),
                )),
                data_type: c_field.data_type.into(),
                size: c_field.size,
                offset: c_field.offset,
            });
        }
        
        rust_tables.push(TableDef {
            id: c_table.id,
            name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                c_table.name,
                _c_strlen(c_table.name),
            )),
            fields: rust_fields.leak(),
            primary_key: c_table.primary_key,
            secondary_index: if c_table.secondary_index == -1 {
                None
            } else {
                Some(c_table.secondary_index as usize)
            },
            record_size: c_table.record_size,
            max_records: c_table.max_records,
        });
    }
    
    let rust_config = DbConfig {
        tables: rust_tables.leak(),
        total_memory: c_config.total_memory,
        low_power_mode_supported: c_config.low_power_mode_supported != 0,
        low_power_max_records: if c_config.low_power_max_records == -1 {
            None
        } else {
            Some(c_config.low_power_max_records as usize)
        },
    };
    
    // 初始化全局数据库
    // 注意：这里需要根据实际情况调整，不能直接使用固定大小的数组
    // 实际使用中应该根据配置动态创建表、主键索引和辅助索引
    match crate::init_global_db(
        core::mem::transmute(&rust_config),
        // 使用Vec::leak()创建动态数组，避免Copy trait问题
        Vec::with_capacity(c_config.tables_count).leak(),
        Vec::with_capacity(c_config.tables_count).leak(),
        Vec::with_capacity(c_config.tables_count).leak(),
    ) {
        Ok(db) => {
            *handle = db as *mut _;
            RemDbError::Success
        },
        Err(e) => e.into(),
    }
}

/// C API: 获取全局数据库实例
#[no_mangle]
pub unsafe extern "C" fn remdb_get_global(
    handle: *mut RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    match crate::get_global_db() {
        Some(db) => {
            *handle = db as *mut _;
            RemDbError::Success
        },
        None => RemDbError::ConfigError,
    }
}

/// C API: 进入低功耗模式
#[no_mangle]
pub unsafe extern "C" fn remdb_enter_low_power_mode(
    handle: RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    match db.enter_low_power_mode() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 退出低功耗模式
#[no_mangle]
pub unsafe extern "C" fn remdb_exit_low_power_mode(
    handle: RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    match db.exit_low_power_mode() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 检查是否处于低功耗模式
#[no_mangle]
pub unsafe extern "C" fn remdb_is_low_power_mode(
    handle: RemDbHandle,
    is_enabled: *mut u8,
) -> RemDbError {
    if handle.is_null() || is_enabled.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    *is_enabled = db.is_low_power_mode() as u8;
    RemDbError::Success
}

/// C API: 开始事务
#[no_mangle]
pub unsafe extern "C" fn remdb_begin_transaction(
    handle: RemDbHandle,
    tx_type: RemDbTransactionType,
    isolation_level: RemDbIsolationLevel,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    let tx_buffer = core::ptr::null_mut();
    let log_buffer = core::ptr::null_mut();
    
    match db.begin_transaction(
        tx_type.into(),
        isolation_level.into(),
        tx_buffer,
        log_buffer,
        0,
    ) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 提交事务
#[no_mangle]
pub unsafe extern "C" fn remdb_commit_transaction(
    handle: RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    match db.commit_transaction() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 回滚事务
#[no_mangle]
pub unsafe extern "C" fn remdb_rollback_transaction(
    handle: RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    match db.rollback_transaction() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 保存快照到文件
#[no_mangle]
pub unsafe extern "C" fn remdb_save_snapshot(
    handle: RemDbHandle,
    path: *const u8,
) -> RemDbError {
    if handle.is_null() || path.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    let rust_path = c_str_to_rust(path);
    match db.save_snapshot(&rust_path) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 从文件恢复快照
#[no_mangle]
pub unsafe extern "C" fn remdb_restore_snapshot(
    handle: RemDbHandle,
    path: *const u8,
) -> RemDbError {
    if handle.is_null() || path.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    let rust_path = c_str_to_rust(path);
    match db.restore_snapshot(&rust_path) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 保存增量快照到文件
#[no_mangle]
pub unsafe extern "C" fn remdb_save_incremental_snapshot(
    handle: RemDbHandle,
    path: *const u8,
) -> RemDbError {
    if handle.is_null() || path.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    let rust_path = c_str_to_rust(path);
    match db.save_incremental_snapshot(&rust_path) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 获取指标快照
#[no_mangle]
pub unsafe extern "C" fn remdb_get_metrics_snapshot(
    handle: RemDbHandle,
    snapshot: *mut RemDbMetricsSnapshot,
) -> RemDbError {
    if handle.is_null() || snapshot.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    let rust_snapshot = db.metrics_snapshot();
    *snapshot = rust_snapshot.into();
    RemDbError::Success
}

/// C API: 重置所有指标
#[no_mangle]
pub unsafe extern "C" fn remdb_reset_metrics(
    handle: RemDbHandle,
) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    db.reset_metrics();
    RemDbError::Success
}

/// C API: 执行健康检查
#[no_mangle]
pub unsafe extern "C" fn remdb_health_check(
    handle: RemDbHandle,
    result: *mut RemDbHealthCheckResult,
) -> RemDbError {
    if handle.is_null() || result.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    let rust_result = db.health_check();
    *result = rust_result.into();
    RemDbError::Success
}

/// C API: 将指标输出到字符串
#[no_mangle]
pub unsafe extern "C" fn remdb_dump_metrics(
    handle: RemDbHandle,
    buffer: *mut u8,
    buffer_size: usize,
    written: *mut usize,
) -> RemDbError {
    if handle.is_null() || buffer.is_null() || written.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    let metrics_str = db.dump_metrics();
    let metrics_bytes = metrics_str.as_bytes();
    
    let copy_len = core::cmp::min(metrics_bytes.len(), buffer_size - 1);
    core::ptr::copy_nonoverlapping(metrics_bytes.as_ptr(), buffer, copy_len);
    *buffer.offset(copy_len as isize) = 0;
    *written = copy_len;
    
    RemDbError::Success
}

/// C API: 获取快照版本
#[no_mangle]
pub unsafe extern "C" fn remdb_get_snapshot_version(
    handle: RemDbHandle,
    version: *mut u32,
) -> RemDbError {
    if handle.is_null() || version.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    *version = db.snapshot_version;
    RemDbError::Success
}

/// C API: 向表中插入记录
#[no_mangle]
pub unsafe extern "C" fn remdb_table_insert(
    handle: RemDbHandle,
    table_id: usize,
    record: *const u8,
) -> RemDbError {
    if handle.is_null() || record.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    match db.get_table_mut(table_id) {
        Ok(table) => {
            match table.insert(record) {
                Ok(_) => RemDbError::Success,
                Err(e) => e.into(),
            }
        },
        Err(e) => e.into(),
    }
}

/// C API: 从表中获取记录
#[no_mangle]
pub unsafe extern "C" fn remdb_table_get(
    handle: RemDbHandle,
    table_id: usize,
    key: *const RemDbValue,
    record: *mut u8,
) -> RemDbError {
    if handle.is_null() || key.is_null() || record.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    
    // 1. 获取主键索引
    let primary_index = match db.get_primary_index_mut(table_id) {
        Ok(index) => index,
        Err(e) => return e.into(),
    };
    
    // 2. 使用主键索引查找记录ID
    let key_ptr = key as *const u8;
    let key_size = core::mem::size_of::<RemDbValue>();
    let record_id = match primary_index.find(key_ptr, key_size) {
        Ok(id) => id as usize,
        Err(e) => return e.into(),
    };
    
    // 3. 获取表并读取记录
    let table = match db.get_table(table_id) {
        Ok(table) => table,
        Err(e) => return e.into(),
    };
    
    match table.get_by_id(record_id, record) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 更新表中的记录
#[no_mangle]
pub unsafe extern "C" fn remdb_table_update(
    handle: RemDbHandle,
    table_id: usize,
    key: *const RemDbValue,
    record: *const u8,
) -> RemDbError {
    if handle.is_null() || key.is_null() || record.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    
    // 1. 获取主键索引
    let primary_index = match db.get_primary_index_mut(table_id) {
        Ok(index) => index,
        Err(e) => return e.into(),
    };
    
    // 2. 使用主键索引查找记录ID
    let key_ptr = key as *const u8;
    let key_size = core::mem::size_of::<RemDbValue>();
    let record_id = match primary_index.find(key_ptr, key_size) {
        Ok(id) => id as usize,
        Err(e) => return e.into(),
    };
    
    // 3. 获取表并更新记录
    let table = match db.get_table_mut(table_id) {
        Ok(table) => table,
        Err(e) => return e.into(),
    };
    
    match table.update(record_id, record) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 从表中删除记录
#[no_mangle]
pub unsafe extern "C" fn remdb_table_delete(
    handle: RemDbHandle,
    table_id: usize,
    key: *const RemDbValue,
) -> RemDbError {
    if handle.is_null() || key.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &mut *handle;
    
    // 1. 获取主键索引
    let primary_index = match db.get_primary_index_mut(table_id) {
        Ok(index) => index,
        Err(e) => return e.into(),
    };
    
    // 2. 使用主键索引查找记录ID
    let key_ptr = key as *const u8;
    let key_size = core::mem::size_of::<RemDbValue>();
    let record_id = match primary_index.find(key_ptr, key_size) {
        Ok(id) => id as usize,
        Err(e) => return e.into(),
    };
    
    // 3. 获取表并删除记录
    let table = match db.get_table_mut(table_id) {
        Ok(table) => table,
        Err(e) => return e.into(),
    };
    
    match table.delete(record_id) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 获取表的记录数
#[no_mangle]
pub unsafe extern "C" fn remdb_table_get_record_count(
    handle: RemDbHandle,
    table_id: usize,
    count: *mut usize,
) -> RemDbError {
    if handle.is_null() || count.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    match db.get_table(table_id) {
        Ok(table) => {
            *count = table.record_count;
            RemDbError::Success
        },
        Err(e) => e.into(),
    }
}

/// C API: 通过名称获取表
#[no_mangle]
pub unsafe extern "C" fn remdb_table_get_by_name(
    handle: RemDbHandle,
    name: *const u8,
    table_id: *mut usize,
) -> RemDbError {
    if handle.is_null() || name.is_null() || table_id.is_null() {
        return RemDbError::ConfigError;
    }
    
    let db = &*handle;
    let rust_name = c_str_to_rust(name);
    
    for (i, table_opt) in db.tables.iter().enumerate() {
        if let Some(table) = table_opt {
            if table.def.name == rust_name {
                *table_id = i;
                return RemDbError::Success;
            }
        }
    }
    
    RemDbError::TableNotFound
}
