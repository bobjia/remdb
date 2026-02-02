#![allow(non_snake_case)]

use crate::config::DbConfig;
use crate::transaction::{IsolationLevel, TransactionType};
use crate::types::{DataType, FieldDef, TableDef, Value};

/// C API: 数据类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbDataType {
    UInt8 = 0,
    UInt16 = 1,
    UInt32 = 2,
    UInt64 = 3,
    Float32 = 4,
    Float64 = 5,
    Bool = 6,
    Timestamp = 7,
    String = 8,
    Vector = 9,
}

impl From<RemDbDataType> for DataType {
    fn from(c_type: RemDbDataType) -> Self {
        match c_type {
            RemDbDataType::UInt8 => DataType::UInt8,
            RemDbDataType::UInt16 => DataType::UInt16,
            RemDbDataType::UInt32 => DataType::UInt32,
            RemDbDataType::UInt64 => DataType::UInt64,
            RemDbDataType::Float32 => DataType::Float32,
            RemDbDataType::Float64 => DataType::Float64,
            RemDbDataType::Bool => DataType::Bool,
            RemDbDataType::Timestamp => DataType::Timestamp,
            RemDbDataType::String => DataType::String,
            RemDbDataType::Vector => DataType::Vector,
        }
    }
}

/// C API: 最大字符串长度
pub const REMDB_MAX_STRING_LEN: usize = 64;

/// C API: 通用值类型
#[repr(C)]
pub union RemDbValue {
    pub u8: u8,
    pub u16: u16,
    pub u32: u32,
    pub u64: u64,
    pub float32: f32,
    pub float64: f64,
    pub boolean: u8,
    pub timestamp: u64,
    pub string: [u8; REMDB_MAX_STRING_LEN],
    pub vector: *const f32,
}

impl From<Value> for RemDbValue {
    fn from(rust_value: Value) -> Self {
        unsafe {
            // 注意：Value是union，直接访问第一个字段作为默认值
            // 实际使用中，应该根据字段的数据类型来访问正确的union字段
            RemDbValue {
                u32: rust_value.u32,
            }
        }
    }
}

impl From<RemDbValue> for Value {
    fn from(c_value: RemDbValue) -> Self {
        // 注意：这个转换需要知道具体的数据类型才能安全进行
        // 在实际使用中，应该根据字段的数据类型来选择合适的变体
        // 这里提供一个默认实现，实际使用时需要根据上下文调整
        unsafe { Value { u32: c_value.u32 } }
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
                DataType::UInt8 => RemDbDataType::UInt8,
                DataType::UInt16 => RemDbDataType::UInt16,
                DataType::UInt32 => RemDbDataType::UInt32,
                DataType::UInt64 => RemDbDataType::UInt64,
                DataType::Int8 => RemDbDataType::UInt8, // 映射为无符号类型
                DataType::Int16 => RemDbDataType::UInt16, // 映射为无符号类型
                DataType::Int32 => RemDbDataType::UInt32, // 映射为无符号类型
                DataType::Int64 => RemDbDataType::UInt64, // 映射为无符号类型
                DataType::Float32 => RemDbDataType::Float32,
                DataType::Float64 => RemDbDataType::Float64,
                DataType::Bool => RemDbDataType::Bool,
                DataType::Timestamp => RemDbDataType::Timestamp,
                DataType::TimestampTZ => RemDbDataType::Timestamp, // 映射为Timestamp
                DataType::String => RemDbDataType::String,
                DataType::Interval => RemDbDataType::UInt64, // 映射为UInt64
                DataType::Vector => RemDbDataType::Vector,   // 映射为Vector类型
                DataType::Json => RemDbDataType::Vector,     // 暂时映射为Vector类型
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

/// C API: HA角色枚举
#[cfg(feature = "ha")]
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbHARole {
    Master = 0,
    Slave = 1,
    Auto = 2,
}

#[cfg(feature = "ha")]
impl From<RemDbHARole> for crate::ha::HARole {
    fn from(c_role: RemDbHARole) -> Self {
        match c_role {
            RemDbHARole::Master => crate::ha::HARole::Master,
            RemDbHARole::Slave => crate::ha::HARole::Slave,
            RemDbHARole::Auto => crate::ha::HARole::Auto,
        }
    }
}

/// C API: 复制模式枚举
#[cfg(feature = "ha")]
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbReplicationMode {
    Async = 0,
    Sync = 1,
}

#[cfg(feature = "ha")]
impl From<RemDbReplicationMode> for crate::ha::ReplicationMode {
    fn from(c_mode: RemDbReplicationMode) -> Self {
        match c_mode {
            RemDbReplicationMode::Async => crate::ha::ReplicationMode::Async,
            RemDbReplicationMode::Sync => crate::ha::ReplicationMode::Sync,
        }
    }
}

/// C API: HA配置
#[cfg(feature = "ha")]
#[repr(C)]
pub struct RemDbHAConfig {
    pub ha_role: RemDbHARole,
    pub replication_mode: RemDbReplicationMode,
    pub heartbeat_interval_ms: u32,
    pub failure_detection_ms: u32,
    pub sync_timeout_ms: u32,
    pub master_address: *const u8, // 字符串形式的IP地址
    pub master_port: u16,
    pub replication_port: u16,
    pub node_id: u32,
}

/// C API: 数据库配置
#[repr(C)]
pub struct RemDbConfig {
    pub tables: *const RemDbTableDef,
    pub tables_count: usize,
    pub time_series_tables: *const RemDbTimeSeriesTableDef,
    pub time_series_tables_count: usize,
    pub total_memory: usize,
    pub low_power_mode_supported: u8,
    pub low_power_max_records: i32,
    pub ha_config: *const core::ffi::c_void, // 可选的HA配置，使用void*以避免依赖特定特性
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
            read_ops: rust_snapshot.read_ops as u64,
            write_ops: rust_snapshot.write_ops as u64,
            delete_ops: rust_snapshot.delete_ops as u64,
            update_ops: rust_snapshot.update_ops as u64,
            cache_hits: rust_snapshot.cache_hits as u64,
            cache_misses: rust_snapshot.cache_misses as u64,
            index_lookups: rust_snapshot.index_lookups as u64,
            index_inserts: rust_snapshot.index_inserts as u64,
            index_deletes: rust_snapshot.index_deletes as u64,
            transactions: rust_snapshot.transactions as u64,
            committed_transactions: rust_snapshot.committed_transactions as u64,
            rolled_back_transactions: rust_snapshot.rolled_back_transactions as u64,
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

/// C API: 数据库状态枚举
#[repr(u8)]
pub enum RemDbDatabaseStatus {
    Created = 0,
    Open = 1,
    Closed = 2,
    Dropped = 3,
}

impl From<crate::DatabaseStatus> for RemDbDatabaseStatus {
    fn from(rust_status: crate::DatabaseStatus) -> Self {
        match rust_status {
            crate::DatabaseStatus::Created => RemDbDatabaseStatus::Created,
            crate::DatabaseStatus::Open => RemDbDatabaseStatus::Open,
            crate::DatabaseStatus::Closed => RemDbDatabaseStatus::Closed,
            crate::DatabaseStatus::Dropped => RemDbDatabaseStatus::Dropped,
        }
    }
}

/// C API: 数据库信息结构体
#[repr(C)]
pub struct RemDbDatabaseInfo {
    pub name: *const u8,
    pub database_type: *const u8,
    pub status: RemDbDatabaseStatus,
    pub table_count: usize,
    pub memory_usage: usize,
}

impl From<crate::DatabaseInfo> for RemDbDatabaseInfo {
    fn from(rust_info: crate::DatabaseInfo) -> Self {
        RemDbDatabaseInfo {
            name: rust_info.name.as_ptr(),
            database_type: rust_info.database_type.as_ptr(),
            status: rust_info.status.into(),
            table_count: rust_info.table_count,
            memory_usage: rust_info.memory_usage,
        }
    }
}

/// C API: 数据库配置结构体
#[repr(C)]
pub struct RemDbDatabaseConfig {
    pub name: *const u8,
    pub memory_limit: *const usize,
    pub max_tables: *const usize,
    pub wal_mode: *const u8,
    pub default_index_type: *const u8,
    pub temp_store: *const u8,
}

/// C API: 类型化值
#[repr(C)]
pub struct RemDbTypedValue {
    pub data_type: RemDbDataType,
    pub value: RemDbValue,
}

impl From<crate::types::TypedValue> for RemDbTypedValue {
    fn from(rust_value: crate::types::TypedValue) -> Self {
        RemDbTypedValue {
            data_type: match rust_value.value_type {
                crate::DataType::UInt8 => RemDbDataType::UInt8,
                crate::DataType::UInt16 => RemDbDataType::UInt16,
                crate::DataType::UInt32 => RemDbDataType::UInt32,
                crate::DataType::UInt64 => RemDbDataType::UInt64,
                crate::DataType::Int8 => RemDbDataType::UInt8, // 映射为无符号类型
                crate::DataType::Int16 => RemDbDataType::UInt16, // 映射为无符号类型
                crate::DataType::Int32 => RemDbDataType::UInt32, // 映射为无符号类型
                crate::DataType::Int64 => RemDbDataType::UInt64, // 映射为无符号类型
                crate::DataType::Float32 => RemDbDataType::Float32,
                crate::DataType::Float64 => RemDbDataType::Float64,
                crate::DataType::Bool => RemDbDataType::Bool,
                crate::DataType::Timestamp => RemDbDataType::Timestamp,
                crate::DataType::TimestampTZ => RemDbDataType::Timestamp, // 映射为Timestamp
                crate::DataType::String => RemDbDataType::String,
                crate::DataType::Interval => RemDbDataType::UInt64, // 映射为UInt64
                crate::DataType::Vector => RemDbDataType::Vector,   // 映射为Vector类型
                crate::DataType::Json => RemDbDataType::Vector,     // 暂时映射为Vector类型
            },
            value: rust_value.value.into(),
        }
    }
}

/// C API: 结果行
#[repr(C)]
pub struct RemDbResultRow {
    pub values: *const RemDbTypedValue,
    pub values_count: usize,
}

/// C API: 结果集
#[repr(C)]
pub struct RemDbResultSet {
    pub columns: *const *const u8,
    pub columns_count: usize,
    pub rows: *const RemDbResultRow,
    pub rows_count: usize,
}

/// C API: UDP模式枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbUdpMode {
    Unicast = 0,
    Broadcast = 1,
    Multicast = 2,
}

impl From<RemDbUdpMode> for crate::pubsub::UdpMode {
    fn from(c_mode: RemDbUdpMode) -> Self {
        match c_mode {
            RemDbUdpMode::Unicast => crate::pubsub::UdpMode::Unicast,
            RemDbUdpMode::Broadcast => crate::pubsub::UdpMode::Broadcast,
            RemDbUdpMode::Multicast => crate::pubsub::UdpMode::Multicast,
        }
    }
}

/// C API: 压缩类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbCompressionType {
    None = 0,
    DeltaRunLength = 1,
    Snappy = 2,
}

impl From<RemDbCompressionType> for crate::time_series::CompressionType {
    fn from(c_type: RemDbCompressionType) -> Self {
        match c_type {
            RemDbCompressionType::None => crate::time_series::CompressionType::None,
            RemDbCompressionType::DeltaRunLength => {
                crate::time_series::CompressionType::DeltaRunLength
            }
            RemDbCompressionType::Snappy => crate::time_series::CompressionType::DeltaRunLength, // 不支持Snappy，使用DeltaRunLength替代
        }
    }
}

/// C API: 时序数据记录
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RemDbTimeSeriesRecord {
    pub timestamp: u64,
    pub value: f64,
    pub tag_count: u8,
    pub tags: [u64; 8],
}

impl From<RemDbTimeSeriesRecord> for crate::time_series::TimeSeriesRecord {
    fn from(c_record: RemDbTimeSeriesRecord) -> Self {
        crate::time_series::TimeSeriesRecord {
            timestamp: c_record.timestamp,
            value: c_record.value,
            tag_count: c_record.tag_count,
            tags: c_record.tags,
        }
    }
}

impl From<crate::time_series::TimeSeriesRecord> for RemDbTimeSeriesRecord {
    fn from(rust_record: crate::time_series::TimeSeriesRecord) -> Self {
        RemDbTimeSeriesRecord {
            timestamp: rust_record.timestamp,
            value: rust_record.value,
            tag_count: rust_record.tag_count,
            tags: rust_record.tags,
        }
    }
}

/// C API: 时序数据配置
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RemDbTimeSeriesConfig {
    pub partition_duration_secs: u64,
    pub retention_period_secs: u64,
    pub compression: RemDbCompressionType,
    pub max_partitions: usize,
}

impl From<RemDbTimeSeriesConfig> for crate::time_series::TimeSeriesConfig {
    fn from(c_config: RemDbTimeSeriesConfig) -> Self {
        crate::time_series::TimeSeriesConfig {
            partition_duration_secs: c_config.partition_duration_secs,
            retention_period_secs: c_config.retention_period_secs,
            compression: c_config.compression.into(),
            max_partitions: c_config.max_partitions,
        }
    }
}

/// C API: 时序表定义
#[repr(C)]
pub struct RemDbTimeSeriesTableDef {
    pub id: u8,
    pub name: *const u8,
    pub fields: *const RemDbFieldDef,
    pub fields_count: usize,
    pub primary_key: usize,
    pub secondary_index: i32,
    pub record_size: usize,
    pub max_records: usize,
    pub time_field: usize,
    pub value_field: usize,
    pub tag_fields: *const usize,
    pub tag_fields_count: usize,
    pub config: RemDbTimeSeriesConfig,
}

/// C API: 发布/订阅配置
#[repr(C)]
pub struct RemDbPubSubConfig {
    pub udp_mode: RemDbUdpMode,
    pub multicast_addr: *const u8, // 字符串形式的IP地址
    pub port: u16,
    pub max_topics: usize,
    pub max_subscribers_per_topic: usize,
    pub buffer_size: usize,
    pub enable_nack: u8,
    pub retransmit_timeout_ms: u32,
    pub max_retransmits: usize,
    pub heartbeat_interval_secs: u32,
    pub frame_pool_size: usize,
}

/// C API: 订阅回调函数类型
type RemDbPubSubCallback = extern "C" fn(topic_id: u16, data: *const u8, data_len: usize) -> u8;

/// 内部: 保存C回调的全局存储
static mut C_CALLBACK_STORAGE: Option<RemDbPubSubCallback> = None;

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
    // PubSub相关错误
    PubSubInitFailed = 19,
    PubSubNetworkError = 20,
    PubSubInvalidParameter = 21,
    PubSubResourceExhausted = 22,
    PubSubInvalidFrameFormat = 23,
    PubSubCrcCheckFailed = 24,
    PubSubTopicNotFound = 25,
    PubSubSubscriptionNotFound = 26,
}

impl From<crate::RemDbError> for RemDbError {
    fn from(rust_error: crate::RemDbError) -> Self {
        match rust_error {
            crate::RemDbError::OutOfMemory => RemDbError::OutOfMemory,
            crate::RemDbError::RecordNotFound => RemDbError::RecordNotFound,
            crate::RemDbError::DuplicateKey => RemDbError::DuplicateKey,
            crate::RemDbError::FieldNotFound => RemDbError::FieldNotFound,
            crate::RemDbError::TypeMismatch => RemDbError::TypeMismatch,
            crate::RemDbError::NotNullViolation => RemDbError::TypeMismatch, // 映射为TypeMismatch
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
            crate::RemDbError::InvalidSqlQuery => RemDbError::UnsupportedOperation,
            crate::RemDbError::InternalError => RemDbError::UnsupportedOperation,
            crate::RemDbError::NoRecordsToOverwrite => RemDbError::RecordNotFound,
            crate::RemDbError::TwoMoreIndexNotSupported => RemDbError::ConfigError, // 映射为ConfigError
            crate::RemDbError::DatabaseNotFound => RemDbError::ConfigError, // 映射为ConfigError
            crate::RemDbError::DatabaseExists => RemDbError::DuplicateKey, // 映射为DuplicateKey
            crate::RemDbError::DatabaseClosed => RemDbError::ConfigError, // 映射为ConfigError
            crate::RemDbError::MaxDatabasesReached => RemDbError::ConfigError, // 映射为ConfigError
        }
    }
}

impl From<crate::pubsub::PubSubError> for RemDbError {
    fn from(rust_error: crate::pubsub::PubSubError) -> Self {
        match rust_error {
            crate::pubsub::PubSubError::InitFailed => RemDbError::PubSubInitFailed,
            crate::pubsub::PubSubError::NetworkError => RemDbError::PubSubNetworkError,
            crate::pubsub::PubSubError::InvalidParameter => RemDbError::PubSubInvalidParameter,
            crate::pubsub::PubSubError::ResourceExhausted => RemDbError::PubSubResourceExhausted,
            crate::pubsub::PubSubError::InvalidFrameFormat => RemDbError::PubSubInvalidFrameFormat,
            crate::pubsub::PubSubError::CrcCheckFailed => RemDbError::PubSubCrcCheckFailed,
            crate::pubsub::PubSubError::TopicNotFound => RemDbError::PubSubTopicNotFound,
            crate::pubsub::PubSubError::SubscriptionNotFound => {
                RemDbError::PubSubSubscriptionNotFound
            }
            crate::pubsub::PubSubError::UnsupportedOperation => RemDbError::UnsupportedOperation,
        }
    }
}

/// 从HA错误转换为C API错误
#[cfg(feature = "ha")]
impl From<crate::ha::HAError> for RemDbError {
    fn from(ha_error: crate::ha::HAError) -> Self {
        match ha_error {
            crate::ha::HAError::InitFailed => RemDbError::PubSubInitFailed,
            crate::ha::HAError::NetworkError => RemDbError::PubSubNetworkError,
            crate::ha::HAError::InvalidParameter => RemDbError::ConfigError,
            crate::ha::HAError::RoleConflict => RemDbError::UnsupportedOperation,
            crate::ha::HAError::SyncFailed => RemDbError::UnsupportedOperation,
            crate::ha::HAError::HeartbeatTimeout => RemDbError::UnsupportedOperation,
            crate::ha::HAError::ReplicationError => RemDbError::UnsupportedOperation,
            crate::ha::HAError::UnsupportedOperation => RemDbError::UnsupportedOperation,
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

    // 初始化内存分配器
    // 分配内存用于内存池
    let total_memory = c_config.total_memory;
    
    // 检查内存大小是否足够
    if total_memory < 1024 * 1024 { // 最小1MB
        return RemDbError::OutOfMemory;
    }
    
    // 分配内存缓冲区
    let mut memory_buffer = alloc::vec::Vec::with_capacity(total_memory);
    
    // 尝试调整内存缓冲区大小
    if let Err(_) = memory_buffer.try_reserve(total_memory) {
        return RemDbError::OutOfMemory;
    }
    
    // 调整大小并初始化
    memory_buffer.resize(total_memory, 0);
    let memory_ptr = memory_buffer.as_mut_ptr();
    
    // 泄漏内存，使其成为静态内存
    core::mem::forget(memory_buffer);
    
    // 初始化全局内存分配器
    if let Err(e) = crate::memory::allocator::init_global_allocator(memory_ptr, total_memory) {
        return e.into();
    }
    
    // 检查内存分配器是否初始化成功
    let stats = crate::memory::allocator::get_memory_stats();
    if stats.total < total_memory / 2 { // 至少应该有一半的内存可用
        return RemDbError::OutOfMemory;
    }

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
                )).to_string(),
                data_type: c_field.data_type.into(),
                size: c_field.size,
                offset: c_field.offset,
                primary_key: j == c_table.primary_key,
                not_null: j == c_table.primary_key, // 主键默认非空
                unique: j == c_table.primary_key,   // 主键默认唯一
                auto_increment: false,              // 默认不自增
                default_value: None,                // 默认无默认值
                vector_metadata: None,              // 默认无向量元数据
                json_metadata: None,                // 默认无JSON元数据
            });
        }

        rust_tables.push(TableDef {
            id: c_table.id,
            name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                c_table.name,
                _c_strlen(c_table.name),
            )).to_string(),
            fields: rust_fields,
            primary_key: vec![c_table.primary_key],
            secondary_index: if c_table.secondary_index == -1 {
                None
            } else {
                Some(vec![c_table.secondary_index as usize])
            },
            secondary_index_type: crate::types::IndexType::Hash,
            record_size: c_table.record_size,
            max_records: c_table.max_records,
            version: 1,
            created_at: 0,
            updated_at: 0,
        });
    }

    // 解析HA配置
    let ha_config = if !c_config.ha_config.is_null() {
        #[cfg(feature = "ha")]
        {
            let c_ha_config = &*(c_config.ha_config as *const RemDbHAConfig);

            // 解析主节点地址
            let master_address = if !c_ha_config.master_address.is_null() {
                Some(c_str_to_rust(c_ha_config.master_address))
            } else {
                None
            };

            Some(crate::ha::HAConfig {
                node_id: c_ha_config.node_id,
                ha_role: c_ha_config.ha_role.into(),
                replication_mode: c_ha_config.replication_mode.into(),
                heartbeat_interval_ms: c_ha_config.heartbeat_interval_ms as u64,
                failure_detection_ms: c_ha_config.failure_detection_ms as u64,
                sync_timeout_ms: c_ha_config.sync_timeout_ms as u64,
                master_address: master_address
                    .map(|s| Box::<str>::leak(s.into_boxed_str()) as &'static str),
                master_port: if c_ha_config.master_port == 0 {
                    None
                } else {
                    Some(c_ha_config.master_port)
                },
                replication_port: c_ha_config.replication_port,
            })
        }
        #[cfg(not(feature = "ha"))]
        {
            // HA特性未启用，忽略HA配置
            None
        }
    } else {
        None
    };

    // 创建Rust配置
    let rust_config = DbConfig {
        tables: rust_tables,
        total_memory: c_config.total_memory,
        low_power_mode_supported: c_config.low_power_mode_supported != 0,
        low_power_max_records: if c_config.low_power_max_records == -1 {
            None
        } else {
            Some(c_config.low_power_max_records as usize)
        },
        default_max_records: 1000, // 默认值
        memory_allocator: &crate::config::DefaultMemoryAllocator,
        wal_config: crate::config::WALConfig {
            log_path: "remdb.wal", // 默认日志文件路径
            log_mode: crate::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,         // 默认60秒
            log_file_size_limit: 16 * 1024 * 1024, // 默认16MB
            log_prealloc_size: 16 * 1024 * 1024,   // 默认16MB
            log_segment_size: 16 * 1024 * 1024,    // 默认16MB
            retained_checkpoints: 2,               // 默认保留2个检查点
        },
        time_series_defaults: crate::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        ha_config,
    };

    // 初始化全局数据库
    // 注意：这里需要根据实际情况调整，不能直接使用固定大小的数组
    // 实际使用中应该根据配置动态创建表、主键索引和辅助索引
    match crate::init_global_db(core::mem::transmute(&rust_config)) {
        Ok(db) => {
            *handle = db as *mut _;

            // 如果有时序表定义，需要初始化时序表
            let db_mut = &mut *(*handle);

            for i in 0..c_config.time_series_tables_count {
                let c_time_series_table = &*c_config.time_series_tables.offset(i as isize);

                // 转换字段定义
                let mut rust_fields = Vec::new();
                for j in 0..c_time_series_table.fields_count {
                    let c_field = &*c_time_series_table.fields.offset(j as isize);
                    rust_fields.push(FieldDef {
                        name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                            c_field.name,
                            _c_strlen(c_field.name),
                        )).to_string(),
                        data_type: c_field.data_type.into(),
                        size: c_field.size,
                        offset: c_field.offset,
                        primary_key: j == c_time_series_table.primary_key,
                        not_null: j == c_time_series_table.primary_key, // 主键默认非空
                        unique: j == c_time_series_table.primary_key,   // 主键默认唯一
                        auto_increment: false,                          // 默认不自增
                        default_value: None,                            // 默认无默认值
                        vector_metadata: None,                          // 默认无向量元数据
                        json_metadata: None,                            // 默认无JSON元数据
                    });
                }

                // 转换标签字段索引
                let mut rust_tag_fields = Vec::new();
                for j in 0..c_time_series_table.tag_fields_count {
                    let tag_field = *c_time_series_table.tag_fields.offset(j as isize);
                    rust_tag_fields.push(tag_field);
                }

                // 创建基础表定义
                let base_table_def = TableDef {
                    id: c_time_series_table.id,
                    name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                        c_time_series_table.name,
                        _c_strlen(c_time_series_table.name),
                    )).to_string(),
                    fields: rust_fields,
                    primary_key: vec![c_time_series_table.primary_key],
                    secondary_index: if c_time_series_table.secondary_index == -1 {
                        None
                    } else {
                        Some(vec![c_time_series_table.secondary_index as usize])
                    },
                    secondary_index_type: crate::types::IndexType::Hash,
                    record_size: c_time_series_table.record_size,
                    max_records: c_time_series_table.max_records,
                    version: 1,
                    created_at: 0,
                    updated_at: 0,
                };

                // 创建时序表定义
                let time_series_table_def = crate::time_series::TimeSeriesTableDef {
                    base: base_table_def,
                    time_field: c_time_series_table.time_field,
                    value_field: c_time_series_table.value_field,
                    tag_fields: rust_tag_fields.into_boxed_slice(),
                    config: c_time_series_table.config.into(),
                };

                // 创建时序索引
                let time_series_index = crate::time_series::TimeSeriesIndex::new();

                // 创建时序表
                match crate::time_series::TimeSeriesTable::new(
                    alloc::sync::Arc::new(time_series_table_def),
                    time_series_index,
                ) {
                    Ok(time_series_table) => {
                        // 将时序表添加到数据库
                        while db_mut.time_series_tables.len() <= i {
                            db_mut.time_series_tables.push(None);
                        }
                        db_mut.time_series_tables[i] = Some(time_series_table);
                    }
                    Err(e) => {
                        return e.into();
                    }
                }
            }

            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 获取全局数据库实例
#[no_mangle]
pub unsafe extern "C" fn remdb_get_global(handle: *mut RemDbHandle) -> RemDbError {
    if handle.is_null() {
        return RemDbError::ConfigError;
    }

    // 初始化内存分配器
    // 分配内存用于内存池
    let total_memory = 1024 * 1024 * 1024; // 默认1GB
    
    // 检查内存大小是否足够
    if total_memory < 1024 * 1024 { // 最小1MB
        return RemDbError::OutOfMemory;
    }
    
    // 分配内存缓冲区
    let mut memory_buffer = alloc::vec::Vec::with_capacity(total_memory);
    
    // 尝试调整内存缓冲区大小
    if let Err(_) = memory_buffer.try_reserve(total_memory) {
        return RemDbError::OutOfMemory;
    }
    
    // 调整大小并初始化
    memory_buffer.resize(total_memory, 0);
    let memory_ptr = memory_buffer.as_mut_ptr();
    
    // 泄漏内存，使其成为静态内存
    core::mem::forget(memory_buffer);
    
    // 初始化全局内存分配器
    if let Err(e) = crate::memory::allocator::init_global_allocator(memory_ptr, total_memory) {
        return e.into();
    }
    
    // 检查内存分配器是否初始化成功
    let stats = crate::memory::allocator::get_memory_stats();
    if stats.total < total_memory / 2 { // 至少应该有一半的内存可用
        return RemDbError::OutOfMemory;
    }
    
    // 创建默认数据库配置
    let default_config = crate::config::DbConfig {
        tables: vec![],
        total_memory: total_memory,
        default_max_records: 100000,
        low_power_mode_supported: true,
        low_power_max_records: Some(10000),
        memory_allocator: &crate::config::DefaultMemoryAllocator,
        wal_config: crate::config::WALConfig {
            log_path: Box::leak(format!("./data/default").into_boxed_str()),
            log_mode: crate::config::LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
        },
        time_series_defaults: crate::time_series::TimeSeriesConfig {
            max_partitions: 100,
            partition_duration_secs: 3600,
            retention_period_secs: 86400 * 30,
            compression: crate::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        ha_config: None,
    };

    // 创建数据库实例
    let mut db = crate::RemDb::new_with_name("default", Box::leak(Box::new(default_config)));
    
    // 初始化数据库
    match db.init() {
        Ok(_) => {
            *handle = Box::leak(Box::new(db)) as *mut _;
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 进入低功耗模式
#[no_mangle]
pub unsafe extern "C" fn remdb_enter_low_power_mode(handle: RemDbHandle) -> RemDbError {
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
pub unsafe extern "C" fn remdb_exit_low_power_mode(handle: RemDbHandle) -> RemDbError {
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
    
    // 创建本地的事务缓冲区和日志缓冲区
    let mut tx_buffer = crate::transaction::Transaction::default();
    let mut log_buffer = [crate::transaction::LogItem::default(); 1024];

    match db.begin_transaction(
        tx_type.into(),
        isolation_level.into(),
        &mut tx_buffer as *mut crate::transaction::Transaction,
        log_buffer.as_mut_ptr(),
        1024,
    ) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 提交事务
#[no_mangle]
pub unsafe extern "C" fn remdb_commit_transaction(handle: RemDbHandle) -> RemDbError {
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
pub unsafe extern "C" fn remdb_rollback_transaction(handle: RemDbHandle) -> RemDbError {
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
pub unsafe extern "C" fn remdb_save_snapshot(handle: RemDbHandle, path: *const u8) -> RemDbError {
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
pub unsafe extern "C" fn remdb_reset_metrics(handle: RemDbHandle) -> RemDbError {
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
        Ok(table) => match table.insert(record) {
            Ok(_) => RemDbError::Success,
            Err(e) => e.into(),
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
        }
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

/// C API: 初始化发布/订阅系统
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_init(config: *const RemDbPubSubConfig) -> RemDbError {
    if config.is_null() {
        return RemDbError::ConfigError;
    }

    let c_config = &*config;

    // 解析组播地址
    let multicast_addr = if !c_config.multicast_addr.is_null() {
        let addr_str = c_str_to_rust(c_config.multicast_addr);
        match addr_str.parse() {
            Ok(addr) => Some(addr),
            Err(_) => None,
        }
    } else {
        None
    };

    // 创建Rust配置
    let rust_config = crate::pubsub::PubSubConfig {
        udp_mode: c_config.udp_mode.into(),
        multicast_addr,
        port: c_config.port,
        max_topics: c_config.max_topics,
        max_subscribers_per_topic: c_config.max_subscribers_per_topic,
        buffer_size: c_config.buffer_size,
        enable_nack: c_config.enable_nack != 0,
        retransmit_timeout: core::time::Duration::from_millis(
            c_config.retransmit_timeout_ms as u64,
        ),
        max_retransmits: c_config.max_retransmits,
        heartbeat_interval: core::time::Duration::from_secs(
            c_config.heartbeat_interval_secs as u64,
        ),
        frame_pool_size: c_config.frame_pool_size,
    };

    // 初始化pubsub
    match crate::pubsub::init(rust_config) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// 内部: 静态回调函数，调用保存的C回调
fn static_pubsub_callback(topic_id: u16, data: &[u8]) -> bool {
    unsafe {
        if let Some(callback) = C_CALLBACK_STORAGE {
            callback(topic_id, data.as_ptr(), data.len()) != 0
        } else {
            false
        }
    }
}

/// C API: 订阅主题
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_subscribe(
    topic_id: u16,
    callback: RemDbPubSubCallback,
    subscription_id: *mut usize,
) -> RemDbError {
    if subscription_id.is_null() {
        return RemDbError::ConfigError;
    }

    // 保存C回调到全局存储
    C_CALLBACK_STORAGE = Some(callback);

    // 订阅主题使用静态回调
    match crate::pubsub::subscribe(topic_id, static_pubsub_callback) {
        Ok(id) => {
            *subscription_id = id;
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 取消订阅
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_unsubscribe(subscription_id: usize) -> RemDbError {
    match crate::pubsub::unsubscribe(subscription_id) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 发布数据
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_publish(
    topic_id: u16,
    data: *const u8,
    data_len: usize,
) -> RemDbError {
    if data.is_null() || data_len == 0 {
        return RemDbError::ConfigError;
    }

    let data_slice = core::slice::from_raw_parts(data, data_len);

    match crate::pubsub::publish(topic_id, data_slice) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 启动接收线程
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_start_receiver() -> RemDbError {
    match crate::pubsub::start_receiver() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 停止发布/订阅系统
#[no_mangle]
pub unsafe extern "C" fn remdb_pubsub_shutdown() -> RemDbError {
    match crate::pubsub::shutdown() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 批量写入时序数据
#[no_mangle]
pub unsafe extern "C" fn remdb_time_series_batch_write(
    handle: RemDbHandle,
    table_id: usize,
    records: *const RemDbTimeSeriesRecord,
    count: usize,
    written: *mut usize,
) -> RemDbError {
    if handle.is_null() || records.is_null() || written.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;

    // 1. 获取时序表
    match db.get_time_series_table_mut(table_id) {
        Ok(time_series_table) => {
            // 2. 将C记录转换为Rust记录
            let mut rust_records = Vec::with_capacity(count);
            for i in 0..count {
                let c_record = unsafe { *records.offset(i as isize) };
                rust_records.push(c_record.into());
            }

            // 3. 执行批量写入
            match time_series_table.write_timeseries_batch(&rust_records) {
                Ok(inserted) => {
                    *written = inserted;
                    RemDbError::Success
                }
                Err(e) => e.into(),
            }
        }
        Err(e) => e.into(),
    }
}

/// C API: 根据时间范围查询时序数据
#[no_mangle]
pub unsafe extern "C" fn remdb_time_series_query(
    handle: RemDbHandle,
    table_id: usize,
    start_time: u64,
    end_time: u64,
    buffer: *mut RemDbTimeSeriesRecord,
    buffer_size: usize,
    result_count: *mut usize,
) -> RemDbError {
    if handle.is_null() || buffer.is_null() || result_count.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &*handle;

    // 1. 获取时序表
    match db.get_time_series_table(table_id) {
        Ok(time_series_table) => {
            // 2. 执行查询
            match time_series_table.query_time_range(start_time, end_time) {
                Ok(results) => {
                    // 3. 将结果转换为C格式并复制到输出缓冲区
                    let actual_count = core::cmp::min(results.len(), buffer_size);
                    for i in 0..actual_count {
                        *buffer.add(i) = results[i].into();
                    }

                    *result_count = actual_count;
                    RemDbError::Success
                }
                Err(e) => e.into(),
            }
        }
        Err(e) => e.into(),
    }
}

/// C API: 根据名称获取时序表
#[no_mangle]
pub unsafe extern "C" fn remdb_time_series_table_get_by_name(
    handle: RemDbHandle,
    name: *const u8,
    table_id: *mut usize,
) -> RemDbError {
    if handle.is_null() || name.is_null() || table_id.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &*handle;
    let rust_name = c_str_to_rust(name);

    // 查找时序表
    for (i, table_opt) in db.time_series_tables.iter().enumerate() {
        if let Some(table) = table_opt {
            if table.def.base.name == rust_name {
                *table_id = i;
                return RemDbError::Success;
            }
        }
    }

    RemDbError::TableNotFound
}

/// C API: 获取可变时序表引用（内部使用）
pub unsafe fn get_time_series_table_mut(
    db: &mut crate::RemDb,
    table_id: usize,
) -> Result<&mut crate::time_series::TimeSeriesTable, crate::RemDbError> {
    if table_id >= db.time_series_tables.len() {
        return Err(crate::RemDbError::TableNotFound);
    }

    match &mut db.time_series_tables[table_id] {
        Some(table) => Ok(table),
        None => Err(crate::RemDbError::TableNotFound),
    }
}

/// C API: 将Rust结果集转换为C结果集
/// 注意：返回的结果集需要通过remdb_free_result_set释放内存
#[no_mangle]
pub unsafe extern "C" fn remdb_sql_query(
    handle: RemDbHandle,
    sql: *const u8,
    result_set: *mut *mut RemDbResultSet,
) -> RemDbError {
    if handle.is_null() || sql.is_null() || result_set.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_sql = c_str_to_rust(sql);

    match db.sql_query(&rust_sql) {
        Ok(rust_result_set) => {
            // 分配内存存储结果集
            let c_result_set = alloc::alloc::alloc(alloc::alloc::Layout::new::<RemDbResultSet>())
                as *mut RemDbResultSet;
            if c_result_set.is_null() {
                return RemDbError::OutOfMemory;
            }

            // 分配内存存储列名
            let columns = alloc::alloc::alloc(
                alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len()).unwrap(),
            ) as *mut *const u8;
            if columns.is_null() {
                alloc::alloc::dealloc(
                    c_result_set as *mut u8,
                    alloc::alloc::Layout::new::<RemDbResultSet>(),
                );
                return RemDbError::OutOfMemory;
            }

            // 转换列名
            for (i, column) in rust_result_set.columns.iter().enumerate() {
                let column_str = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<u8>(column.len() + 1).unwrap(),
                ) as *mut u8;
                if column_str.is_null() {
                    // 释放已分配的内存
                    for j in 0..i {
                        let col = *columns.offset(j as isize);
                        alloc::alloc::dealloc(
                            col as *mut u8,
                            alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                        );
                    }
                    alloc::alloc::dealloc(
                        columns as *mut u8,
                        alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                            .unwrap(),
                    );
                    alloc::alloc::dealloc(
                        c_result_set as *mut u8,
                        alloc::alloc::Layout::new::<RemDbResultSet>(),
                    );
                    return RemDbError::OutOfMemory;
                }

                // 复制列名字符串
                core::ptr::copy_nonoverlapping(column.as_ptr(), column_str, column.len());
                *column_str.offset(column.len() as isize) = 0; // 添加终止符
                *columns.offset(i as isize) = column_str as *const u8;
            }

            // 分配内存存储行
            let rows = alloc::alloc::alloc(
                alloc::alloc::Layout::array::<RemDbResultRow>(rust_result_set.rows.len()).unwrap(),
            ) as *mut RemDbResultRow;
            if rows.is_null() {
                // 释放已分配的内存
                for i in 0..rust_result_set.columns.len() {
                    let col = *columns.offset(i as isize);
                    alloc::alloc::dealloc(
                        col as *mut u8,
                        alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                    );
                }
                alloc::alloc::dealloc(
                    columns as *mut u8,
                    alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                        .unwrap(),
                );
                alloc::alloc::dealloc(
                    c_result_set as *mut u8,
                    alloc::alloc::Layout::new::<RemDbResultSet>(),
                );
                return RemDbError::OutOfMemory;
            }

            // 转换行数据
            for (i, row) in rust_result_set.rows.iter().enumerate() {
                // 分配内存存储值
                let values = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<RemDbTypedValue>(row.values.len()).unwrap(),
                ) as *mut RemDbTypedValue;
                if values.is_null() {
                    // 释放已分配的内存
                    for j in 0..i {
                        let r = &*rows.offset(j as isize);
                        alloc::alloc::dealloc(
                            r.values as *mut u8,
                            alloc::alloc::Layout::array::<RemDbTypedValue>(r.values_count).unwrap(),
                        );
                    }
                    alloc::alloc::dealloc(
                        rows as *mut u8,
                        alloc::alloc::Layout::array::<RemDbResultRow>(rust_result_set.rows.len())
                            .unwrap(),
                    );
                    for j in 0..rust_result_set.columns.len() {
                        let col = *columns.offset(j as isize);
                        alloc::alloc::dealloc(
                            col as *mut u8,
                            alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                        );
                    }
                    alloc::alloc::dealloc(
                        columns as *mut u8,
                        alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                            .unwrap(),
                    );
                    alloc::alloc::dealloc(
                        c_result_set as *mut u8,
                        alloc::alloc::Layout::new::<RemDbResultSet>(),
                    );
                    return RemDbError::OutOfMemory;
                }

                // 转换值
                for (j, value) in row.values.iter().enumerate() {
                    *values.offset(j as isize) = value.clone().into();
                }

                // 设置行数据
                let row_ptr = rows.offset(i as isize);
                (*row_ptr).values = values;
                (*row_ptr).values_count = row.values.len();
            }

            // 设置结果集数据
            (*c_result_set).columns = columns;
            (*c_result_set).columns_count = rust_result_set.columns.len();
            (*c_result_set).rows = rows;
            (*c_result_set).rows_count = rust_result_set.rows.len();

            *result_set = c_result_set;
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 释放结果集内存
#[no_mangle]
pub unsafe extern "C" fn remdb_free_result_set(result_set: *mut RemDbResultSet) -> RemDbError {
    if result_set.is_null() {
        return RemDbError::Success;
    }

    let rs = &mut *result_set;

    // 释放列名
    for i in 0..rs.columns_count {
        let col = *rs.columns.offset(i as isize);
        if !col.is_null() {
            alloc::alloc::dealloc(
                col as *mut u8,
                alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
            );
        }
    }
    alloc::alloc::dealloc(
        rs.columns as *mut u8,
        alloc::alloc::Layout::array::<*const u8>(rs.columns_count).unwrap(),
    );

    // 释放行数据
    for i in 0..rs.rows_count {
        let row = &*rs.rows.offset(i as isize);
        if !row.values.is_null() {
            alloc::alloc::dealloc(
                row.values as *mut u8,
                alloc::alloc::Layout::array::<RemDbTypedValue>(row.values_count).unwrap(),
            );
        }
    }
    alloc::alloc::dealloc(
        rs.rows as *mut u8,
        alloc::alloc::Layout::array::<RemDbResultRow>(rs.rows_count).unwrap(),
    );

    // 释放结果集本身
    alloc::alloc::dealloc(
        result_set as *mut u8,
        alloc::alloc::Layout::new::<RemDbResultSet>(),
    );

    RemDbError::Success
}

/// C API: 执行查询操作
#[no_mangle]
pub unsafe extern "C" fn remdb_execute_query(
    handle: RemDbHandle,
    table_name: *const u8,
    columns: *const *const u8,
    columns_count: usize,
    where_clause: *const u8,
    limit: i32,
    result_set: *mut *mut RemDbResultSet,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || result_set.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_table_name = c_str_to_rust(table_name);

    // 转换列名
    let mut rust_columns = Vec::with_capacity(columns_count);
    for i in 0..columns_count {
        let col = *columns.offset(i as isize);
        if !col.is_null() {
            rust_columns.push(c_str_to_rust(col));
        }
    }

    // 转换where子句
    let rust_where_clause = if !where_clause.is_null() {
        Some(c_str_to_rust(where_clause))
    } else {
        None
    };

    // 转换limit
    let rust_limit = if limit > 0 {
        Some(limit as usize)
    } else {
        None
    };

    match db.execute_query(
        &rust_table_name,
        &rust_columns
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
        rust_where_clause.as_deref(),
        rust_limit,
    ) {
        Ok(rust_result_set) => {
            // 分配内存存储结果集
            let c_result_set = alloc::alloc::alloc(alloc::alloc::Layout::new::<RemDbResultSet>())
                as *mut RemDbResultSet;
            if c_result_set.is_null() {
                return RemDbError::OutOfMemory;
            }

            // 分配内存存储列名
            let columns_ptr = alloc::alloc::alloc(
                alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len()).unwrap(),
            ) as *mut *const u8;
            if columns_ptr.is_null() {
                alloc::alloc::dealloc(
                    c_result_set as *mut u8,
                    alloc::alloc::Layout::new::<RemDbResultSet>(),
                );
                return RemDbError::OutOfMemory;
            }

            // 转换列名
            for (i, column) in rust_result_set.columns.iter().enumerate() {
                let column_str = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<u8>(column.len() + 1).unwrap(),
                ) as *mut u8;
                if column_str.is_null() {
                    // 释放已分配的内存
                    for j in 0..i {
                        let col = *columns_ptr.offset(j as isize);
                        alloc::alloc::dealloc(
                            col as *mut u8,
                            alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                        );
                    }
                    alloc::alloc::dealloc(
                        columns_ptr as *mut u8,
                        alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                            .unwrap(),
                    );
                    alloc::alloc::dealloc(
                        c_result_set as *mut u8,
                        alloc::alloc::Layout::new::<RemDbResultSet>(),
                    );
                    return RemDbError::OutOfMemory;
                }

                // 复制列名字符串
                core::ptr::copy_nonoverlapping(column.as_ptr(), column_str, column.len());
                *column_str.offset(column.len() as isize) = 0; // 添加终止符
                *columns_ptr.offset(i as isize) = column_str as *const u8;
            }

            // 分配内存存储行
            let rows_ptr = alloc::alloc::alloc(
                alloc::alloc::Layout::array::<RemDbResultRow>(rust_result_set.rows.len()).unwrap(),
            ) as *mut RemDbResultRow;
            if rows_ptr.is_null() {
                // 释放已分配的内存
                for i in 0..rust_result_set.columns.len() {
                    let col = *columns_ptr.offset(i as isize);
                    alloc::alloc::dealloc(
                        col as *mut u8,
                        alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                    );
                }
                alloc::alloc::dealloc(
                    columns_ptr as *mut u8,
                    alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                        .unwrap(),
                );
                alloc::alloc::dealloc(
                    c_result_set as *mut u8,
                    alloc::alloc::Layout::new::<RemDbResultSet>(),
                );
                return RemDbError::OutOfMemory;
            }

            // 转换行数据
            for (i, row) in rust_result_set.rows.iter().enumerate() {
                // 分配内存存储值
                let values_ptr = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<RemDbTypedValue>(row.values.len()).unwrap(),
                ) as *mut RemDbTypedValue;
                if values_ptr.is_null() {
                    // 释放已分配的内存
                    for j in 0..i {
                        let r = &*rows_ptr.offset(j as isize);
                        if !r.values.is_null() {
                            alloc::alloc::dealloc(
                                r.values as *mut u8,
                                alloc::alloc::Layout::array::<RemDbTypedValue>(r.values_count)
                                    .unwrap(),
                            );
                        }
                    }
                    alloc::alloc::dealloc(
                        rows_ptr as *mut u8,
                        alloc::alloc::Layout::array::<RemDbResultRow>(rust_result_set.rows.len())
                            .unwrap(),
                    );
                    for j in 0..rust_result_set.columns.len() {
                        let col = *columns_ptr.offset(j as isize);
                        alloc::alloc::dealloc(
                            col as *mut u8,
                            alloc::alloc::Layout::array::<u8>(_c_strlen(col) + 1).unwrap(),
                        );
                    }
                    alloc::alloc::dealloc(
                        columns_ptr as *mut u8,
                        alloc::alloc::Layout::array::<*const u8>(rust_result_set.columns.len())
                            .unwrap(),
                    );
                    alloc::alloc::dealloc(
                        c_result_set as *mut u8,
                        alloc::alloc::Layout::new::<RemDbResultSet>(),
                    );
                    return RemDbError::OutOfMemory;
                }

                // 转换值
                for (j, value) in row.values.iter().enumerate() {
                    *values_ptr.offset(j as isize) = value.clone().into();
                }

                // 设置行数据
                let row_ptr = rows_ptr.offset(i as isize);
                (*row_ptr).values = values_ptr;
                (*row_ptr).values_count = row.values.len();
            }

            // 设置结果集数据
            (*c_result_set).columns = columns_ptr;
            (*c_result_set).columns_count = rust_result_set.columns.len();
            (*c_result_set).rows = rows_ptr;
            (*c_result_set).rows_count = rust_result_set.rows.len();

            *result_set = c_result_set;
    RemDbError::Success
}
Err(e) => e.into(),
}
}

// 向量索引相关C API函数声明

/// C API: 向量索引类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbVectorIndexType {
    HNSW = 0,
    HNSW_SQ = 1,
    HNSW_BQ = 2,
    IVF = 3,          // IVF_FLAT
    IVF_PQ = 4,
}

/// C API: 向量距离度量类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum RemDbDistanceType {
    L2 = 0,
    InnerProduct = 1,
    Cosine = 2,
}

/// C API: 向量元数据配置
#[repr(C)]
pub struct RemDbVectorMetadata {
    pub dimension: u16,
    pub distance_type: RemDbDistanceType,
    pub index_type: RemDbVectorIndexType,
    pub compression_enabled: u8,
    pub compression_scheme: u8,
    pub compression_level: u8,
    pub hnsw_m: u8,
    pub hnsw_ef_construction: u32,
    pub hnsw_ef_search: u32,
    pub ivf_nlist: u32,
    pub ivf_nprobe: u32,
}

/// C API: 初始化索引构建线程池
#[no_mangle]
pub unsafe extern "C" fn remdb_init_index_build_thread_pool(
    thread_count: u32,
) -> RemDbError {
    crate::index::builder::init_index_build_thread_pool(thread_count as usize);
    RemDbError::Success
}

/// C API: 创建向量索引
#[no_mangle]
pub unsafe extern "C" fn remdb_create_vector_index(
    handle: RemDbHandle,
    table_name: *const u8,
    field_name: *const u8,
    metadata: *const RemDbVectorMetadata,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || field_name.is_null() || metadata.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let table_name_str = c_str_to_rust(table_name);
    let field_name_str = c_str_to_rust(field_name);
    let c_meta = &*metadata;

    // 转换为Rust向量元数据
    let rust_meta = crate::types::VectorMetadata {
        dimension: c_meta.dimension,
        distance_type: match c_meta.distance_type {
            RemDbDistanceType::L2 => crate::types::DistanceType::L2,
            RemDbDistanceType::InnerProduct => crate::types::DistanceType::InnerProduct,
            RemDbDistanceType::Cosine => crate::types::DistanceType::Cosine,
        },
        index_type: match c_meta.index_type {
            RemDbVectorIndexType::HNSW => crate::types::VectorIndexType::HNSW,
            RemDbVectorIndexType::HNSW_SQ => crate::types::VectorIndexType::HNSW_SQ,
            RemDbVectorIndexType::HNSW_BQ => crate::types::VectorIndexType::HNSW_BQ,
            RemDbVectorIndexType::IVF => crate::types::VectorIndexType::IVF, // IVF_FLAT
            RemDbVectorIndexType::IVF_PQ => crate::types::VectorIndexType::IVF_PQ,
        },
        compression_enabled: c_meta.compression_enabled != 0,
        compression_scheme: c_meta.compression_scheme,
        compression_level: c_meta.compression_level,
        hnsw_m: c_meta.hnsw_m,
        hnsw_ef_construction: c_meta.hnsw_ef_construction,
        hnsw_ef_search: c_meta.hnsw_ef_search,
        ivf_nlist: c_meta.ivf_nlist,
        ivf_nprobe: c_meta.ivf_nprobe,
    };

    // 使用SQL API创建向量索引
    let sql = alloc::format!(
        "CREATE INDEX ON {} ({}) WITH DIMENSION={}, DISTANCE={:?}, INDEX_TYPE={:?}",
        table_name_str,
        field_name_str,
        rust_meta.dimension,
        rust_meta.distance_type,
        rust_meta.index_type
    );

    match db.sql_query(&sql) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 向量相似度搜索
#[no_mangle]
pub unsafe extern "C" fn remdb_vector_search(
    handle: RemDbHandle,
    table_name: *const u8,
    field_name: *const u8,
    query_vector: *const f32,
    _vector_dim: u16,
    k: u32,
    results: *mut *mut u32, // 返回匹配的记录ID数组
    distances: *mut *mut f32, // 返回距离数组
    result_count: *mut u32, // 实际返回的结果数量
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || field_name.is_null() || query_vector.is_null() || 
       results.is_null() || distances.is_null() || result_count.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let table_name_str = c_str_to_rust(table_name);
    let field_name_str = c_str_to_rust(field_name);

    // 使用SQL API执行向量搜索
    let sql = alloc::format!(
        "SELECT id, VECTOR_DISTANCE({}, ?) as distance FROM {} ORDER BY distance LIMIT {}",
        field_name_str,
        table_name_str,
        k
    );

    // 注意：此处简化实现，实际应该支持参数化查询
    match db.sql_query(&sql) {
        Ok(rust_result_set) => {
            let actual_count = rust_result_set.rows.len();
            *result_count = actual_count as u32;

            if actual_count > 0 {
                // 分配内存存储结果
                let result_ids = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<u32>(actual_count).unwrap(),
                ) as *mut u32;
                let result_distances = alloc::alloc::alloc(
                    alloc::alloc::Layout::array::<f32>(actual_count).unwrap(),
                ) as *mut f32;

                if result_ids.is_null() || result_distances.is_null() {
                    if !result_ids.is_null() {
                        alloc::alloc::dealloc(
                            result_ids as *mut u8,
                            alloc::alloc::Layout::array::<u32>(actual_count).unwrap(),
                        );
                    }
                    if !result_distances.is_null() {
                        alloc::alloc::dealloc(
                            result_distances as *mut u8,
                            alloc::alloc::Layout::array::<f32>(actual_count).unwrap(),
                        );
                    }
                    return RemDbError::OutOfMemory;
                }

                // 提取结果
                for i in 0..actual_count {
                    let row = &rust_result_set.rows[i];
                    if row.values.len() >= 2 {
                        // 假设第一列为id，第二列为distance
                        let id_value = &row.values[0];
                        let distance_value = &row.values[1];

                        // 提取id值
                        if let crate::types::DataType::UInt32 | 
                           crate::types::DataType::Int32 | 
                           crate::types::DataType::UInt64 | 
                           crate::types::DataType::Int64 = id_value.value_type {
                            *result_ids.offset(i as isize) = unsafe { id_value.value.u32 };
                        } else {
                            *result_ids.offset(i as isize) = 0;
                        }

                        // 提取distance值
                        if let crate::types::DataType::Float32 | 
                           crate::types::DataType::Float64 = distance_value.value_type {
                            *result_distances.offset(i as isize) = unsafe { distance_value.value.float32 };
                        } else {
                            *result_distances.offset(i as isize) = 0.0;
                        }
                    }
                }

                *results = result_ids;
                *distances = result_distances;
            }

            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 释放向量搜索结果内存
#[no_mangle]
pub unsafe extern "C" fn remdb_free_vector_search_results(
    results: *mut u32,
    distances: *mut f32,
    count: u32,
) -> RemDbError {
    if !results.is_null() {
        alloc::alloc::dealloc(
            results as *mut u8,
            alloc::alloc::Layout::array::<u32>(count as usize).unwrap(),
        );
    }

    if !distances.is_null() {
        alloc::alloc::dealloc(
            distances as *mut u8,
            alloc::alloc::Layout::array::<f32>(count as usize).unwrap(),
        );
    }

    RemDbError::Success
}

/// C API: 获取索引构建状态
#[no_mangle]
pub unsafe extern "C" fn remdb_get_index_build_status(
    handle: RemDbHandle,
    table_name: *const u8,
    field_name: *const u8,
    is_building: *mut u8,
    progress: *mut u32, // 0-100
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || field_name.is_null() || 
       is_building.is_null() || progress.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let table_name_str = c_str_to_rust(table_name);
    let field_name_str = c_str_to_rust(field_name);

    // 使用SQL API查询索引构建状态
    let sql = alloc::format!(
        "SHOW INDEX BUILD STATUS ON {} FOR {}",
        table_name_str,
        field_name_str
    );

    match db.sql_query(&sql) {
        Ok(rust_result_set) => {
            if rust_result_set.rows.len() > 0 {
                let row = &rust_result_set.rows[0];
                if row.values.len() >= 2 {
                    // 假设第一列为is_building，第二列为progress
                    let building_value = &row.values[0];
                    let progress_value = &row.values[1];

                    // 提取is_building值
                    if let crate::types::DataType::Bool = building_value.value_type {
                        *is_building = unsafe { building_value.value.bool as u8 };
                    } else {
                        *is_building = 0;
                    }

                    // 提取progress值
                    if let crate::types::DataType::UInt32 | 
                       crate::types::DataType::Int32 = progress_value.value_type {
                        *progress = unsafe { progress_value.value.u32 };
                    } else {
                        *progress = 0;
                    }
                }
            }

            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 创建表
#[no_mangle]
pub unsafe extern "C" fn remdb_create_table(
    handle: RemDbHandle,
    table_name: *const u8,
    fields: *const RemDbFieldDef,
    fields_count: usize,
    primary_key: i32,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_table_name = c_str_to_rust(table_name);

    // 转换字段定义
    let mut field_name_strings = Vec::with_capacity(fields_count);
    let mut rust_fields = Vec::with_capacity(fields_count);

    for i in 0..fields_count {
        let c_field = &*fields.offset(i as isize);
        let field_name = c_str_to_rust(c_field.name);
        field_name_strings.push(field_name);
    }

    // 现在创建字段定义向量
    for (i, field_name) in field_name_strings.iter().enumerate() {
        let c_field = &*fields.offset(i as isize);
        rust_fields.push((
            field_name.as_str(),
            c_field.data_type.into(),
            c_field.size as u16,
            None, // 不支持向量距离类型
            None, // 不支持默认值
        ));
    }

    // 转换主键索引
    let rust_primary_key = if primary_key >= 0 {
        Some(vec![primary_key as usize])
    } else {
        None
    };

    match db.create_table(&rust_table_name, &rust_fields, rust_primary_key) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 批量插入记录
#[no_mangle]
pub unsafe extern "C" fn remdb_batch_insert_record(
    handle: RemDbHandle,
    table_name: *const u8,
    column_names: *const *const u8,
    column_names_count: usize,
    records: *const *const *const u8,
    records_count: usize,
    values_per_record: usize,
    affected_rows: *mut usize,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || records.is_null() || affected_rows.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_table_name = c_str_to_rust(table_name);

    // 转换列名
    let mut col_name_vec = Vec::with_capacity(column_names_count);
    for i in 0..column_names_count {
        let col = *column_names.offset(i as isize);
        if !col.is_null() {
            let col_str = c_str_to_rust(col);
            col_name_vec.push(col_str);
        }
    }

    // 转换列名向量为&str切片
    let col_names: Vec<&str> = col_name_vec.iter().map(|s| s.as_str()).collect();

    // 转换并插入每条记录
    let mut total_inserted = 0;

    for i in 0..records_count {
        let record = *records.offset(i as isize);

        // 转换单条记录的字段值
        let mut field_value_vec = Vec::with_capacity(values_per_record);
        for j in 0..values_per_record {
            let value = *record.offset(j as isize);
            if !value.is_null() {
                let val_str = c_str_to_rust(value);
                field_value_vec.push(val_str);
            } else {
                field_value_vec.push("".to_string());
            }
        }

        // 转换字段值向量为&str切片
        let field_values: Vec<&str> = field_value_vec.iter().map(|s| s.as_str()).collect();

        // 单条插入记录
        match db.insert_record(&rust_table_name, &col_names, &field_values) {
            Ok(inserted) => {
                total_inserted += inserted;
            }
            Err(e) => {
                return e.into();
            }
        }
    }

    *affected_rows = total_inserted;
    RemDbError::Success
}

/// C API: 更新记录
#[no_mangle]
pub unsafe extern "C" fn remdb_update_record(
    handle: RemDbHandle,
    table_name: *const u8,
    set_clause: *const u8,
    where_clause: *const u8,
    affected_rows: *mut usize,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || set_clause.is_null() || affected_rows.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_table_name = c_str_to_rust(table_name);
    let rust_set_clause = c_str_to_rust(set_clause);

    // 转换where子句
    let where_clause_str = if !where_clause.is_null() {
        Some(c_str_to_rust(where_clause))
    } else {
        None
    };
    let rust_where_clause = where_clause_str.as_deref();

    match db.update_record(&rust_table_name, &rust_set_clause, rust_where_clause) {
        Ok(updated) => {
            *affected_rows = updated;
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 删除记录
#[no_mangle]
pub unsafe extern "C" fn remdb_delete_record(
    handle: RemDbHandle,
    table_name: *const u8,
    where_clause: *const u8,
    affected_rows: *mut usize,
) -> RemDbError {
    if handle.is_null() || table_name.is_null() || affected_rows.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_table_name = c_str_to_rust(table_name);

    // 转换where子句
    let where_clause_str = if !where_clause.is_null() {
        Some(c_str_to_rust(where_clause))
    } else {
        None
    };
    let rust_where_clause = where_clause_str.as_deref();

    match db.delete_record(&rust_table_name, rust_where_clause) {
        Ok(deleted) => {
            *affected_rows = deleted;
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 导出DDL
#[no_mangle]
pub unsafe extern "C" fn remdb_export_ddl(handle: RemDbHandle, path: *const u8) -> RemDbError {
    if handle.is_null() || path.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &*handle;
    let rust_path = c_str_to_rust(path);

    match db.export_ddl(&rust_path) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 导出数据
#[no_mangle]
pub unsafe extern "C" fn remdb_export_data(handle: RemDbHandle, path: *const u8) -> RemDbError {
    if handle.is_null() || path.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &*handle;
    let rust_path = c_str_to_rust(path);

    match db.export_data(&rust_path) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 获取当前HA角色
#[cfg(feature = "ha")]
#[no_mangle]
pub unsafe extern "C" fn remdb_ha_get_role(role: *mut RemDbHARole) -> RemDbError {
    if role.is_null() {
        return RemDbError::ConfigError;
    }

    match crate::ha::get_role() {
        Ok(ha_role) => {
            *role = match ha_role {
                crate::ha::HARole::Master => RemDbHARole::Master,
                crate::ha::HARole::Slave => RemDbHARole::Slave,
                crate::ha::HARole::Auto => RemDbHARole::Auto,
            };
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 提升为Master节点
#[cfg(feature = "ha")]
#[no_mangle]
pub unsafe extern "C" fn remdb_ha_promote_to_master() -> RemDbError {
    match crate::ha::promote_to_master() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 降级为Slave节点
#[cfg(feature = "ha")]
#[no_mangle]
pub unsafe extern "C" fn remdb_ha_demote_to_slave() -> RemDbError {
    match crate::ha::demote_to_slave() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 检查HA状态
#[cfg(feature = "ha")]
#[no_mangle]
pub unsafe extern "C" fn remdb_ha_check_status() -> RemDbError {
    match crate::ha::check_status() {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 获取复制模式
#[cfg(feature = "ha")]
#[no_mangle]
pub unsafe extern "C" fn remdb_ha_get_replication_mode(
    mode: *mut RemDbReplicationMode,
) -> RemDbError {
    if mode.is_null() {
        return RemDbError::ConfigError;
    }

    match crate::ha::get_replication_mode() {
        Ok(replication_mode) => {
            *mode = match replication_mode {
                crate::ha::ReplicationMode::Async => RemDbReplicationMode::Async,
                crate::ha::ReplicationMode::Sync => RemDbReplicationMode::Sync,
            };
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 创建数据库
#[no_mangle]
pub unsafe extern "C" fn remdb_create_database(
    name: *const u8,
    schema: *const u8,
    config: *const RemDbDatabaseConfig,
) -> RemDbError {
    if name.is_null() {
        return RemDbError::ConfigError;
    }

    // 检查数据库名称长度
    let name_len = _c_strlen(name);
    if name_len == 0 || name_len > 128 { // 限制数据库名称长度为128个字符
        return RemDbError::ConfigError;
    }

    let rust_name = c_str_to_rust(name);
    let rust_schema = if schema.is_null() { "" } else { &*Box::leak(Box::new(c_str_to_rust(schema))) };

    // 转换数据库配置
    let rust_config = if config.is_null() {
        None
    } else {
        let c_config = &*config;
        Some(crate::DatabaseConfig {
            name: if c_config.name.is_null() { rust_name.clone() } else { c_str_to_rust(c_config.name) },
            memory_limit: if c_config.memory_limit.is_null() { None } else { Some(*c_config.memory_limit) },
            max_tables: if c_config.max_tables.is_null() { None } else { Some(*c_config.max_tables) },
            wal_mode: if c_config.wal_mode.is_null() { None } else { Some(c_str_to_rust(c_config.wal_mode)) },
            default_index_type: if c_config.default_index_type.is_null() { None } else { Some(crate::types::IndexType::Hash) },
            temp_store: if c_config.temp_store.is_null() { None } else { Some(c_str_to_rust(c_config.temp_store)) },
        })
    };

    // 创建数据库管理器
    let mut db_manager = crate::DatabaseManager::new(10); // 默认最大10个数据库

    match db_manager.create_database(&rust_name, rust_schema, rust_config) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 使用指定数据库
#[no_mangle]
pub unsafe extern "C" fn remdb_use_database(
    handle: RemDbHandle,
    name: *const u8,
) -> RemDbError {
    if handle.is_null() || name.is_null() {
        return RemDbError::ConfigError;
    }

    // 检查数据库名称长度
    let name_len = _c_strlen(name);
    if name_len == 0 || name_len > 128 { // 限制数据库名称长度为128个字符
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_name = c_str_to_rust(name);

    match db.use_database(&rust_name) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 关闭指定数据库
#[no_mangle]
pub unsafe extern "C" fn remdb_close_database(
    handle: RemDbHandle,
    name: *const u8,
) -> RemDbError {
    if handle.is_null() || name.is_null() {
        return RemDbError::ConfigError;
    }

    // 检查数据库名称长度
    let name_len = _c_strlen(name);
    if name_len == 0 || name_len > 128 { // 限制数据库名称长度为128个字符
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_name = c_str_to_rust(name);

    match db.close_database(&rust_name) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 删除指定数据库
#[no_mangle]
pub unsafe extern "C" fn remdb_drop_database(
    handle: RemDbHandle,
    name: *const u8,
) -> RemDbError {
    if handle.is_null() || name.is_null() {
        return RemDbError::ConfigError;
    }

    // 检查数据库名称长度
    let name_len = _c_strlen(name);
    if name_len == 0 || name_len > 128 { // 限制数据库名称长度为128个字符
        return RemDbError::ConfigError;
    }

    let db = &mut *handle;
    let rust_name = c_str_to_rust(name);

    match db.drop_database(&rust_name) {
        Ok(_) => RemDbError::Success,
        Err(e) => e.into(),
    }
}

/// C API: 获取数据库列表
#[no_mangle]
pub unsafe extern "C" fn remdb_get_databases(
    handle: RemDbHandle,
    databases: *mut *mut RemDbDatabaseInfo,
    count: *mut usize,
) -> RemDbError {
    if handle.is_null() || databases.is_null() || count.is_null() {
        return RemDbError::ConfigError;
    }

    let db = &*handle;

    match db.databases() {
        Ok(rust_databases) => {
            // 分配内存存储数据库信息
            let c_databases = alloc::alloc::alloc(
                alloc::alloc::Layout::array::<RemDbDatabaseInfo>(rust_databases.len()).unwrap(),
            ) as *mut RemDbDatabaseInfo;

            if c_databases.is_null() {
                return RemDbError::OutOfMemory;
            }

            // 转换数据库信息
            for (i, rust_db) in rust_databases.iter().enumerate() {
                let c_db = &mut *c_databases.offset(i as isize);
                *c_db = rust_db.clone().into();
            }

            *databases = c_databases;
            *count = rust_databases.len();
            RemDbError::Success
        }
        Err(e) => e.into(),
    }
}

/// C API: 释放数据库列表内存
#[no_mangle]
pub unsafe extern "C" fn remdb_free_databases(
    databases: *mut RemDbDatabaseInfo,
    count: usize,
) -> RemDbError {
    if !databases.is_null() {
        alloc::alloc::dealloc(
            databases as *mut u8,
            alloc::alloc::Layout::array::<RemDbDatabaseInfo>(count).unwrap(),
        );
    }
    RemDbError::Success
}
