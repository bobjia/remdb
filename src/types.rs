use core::fmt;
use core::mem::size_of;

/// 基本数据类型枚举
#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
pub enum DataType {
    /// 8位无符号整数
    UInt8 = 0,
    /// 16位无符号整数
    UInt16 = 1,
    /// 32位无符号整数
    UInt32 = 2,
    /// 64位无符号整数
    UInt64 = 3,
    /// 8位有符号整数
    Int8 = 4,
    /// 16位有符号整数
    Int16 = 5,
    /// 32位有符号整数
    Int32 = 6,
    /// 64位有符号整数
    Int64 = 7,
    /// 32位浮点数
    Float32 = 8,
    /// 64位浮点数
    Float64 = 9,
    /// 布尔值
    Bool = 10,
    /// 时间戳（毫秒）
    Timestamp = 11,
    /// 定长字符串
    String = 12,
}

impl DataType {
    /// 获取数据类型的大小（字节）
    pub const fn size(&self) -> usize {
        match self {
            DataType::UInt8 => 1,
            DataType::UInt16 => 2,
            DataType::UInt32 => 4,
            DataType::UInt64 => 8,
            DataType::Int8 => 1,
            DataType::Int16 => 2,
            DataType::Int32 => 4,
            DataType::Int64 => 8,
            DataType::Float32 => 4,
            DataType::Float64 => 8,
            DataType::Bool => 1,
            DataType::Timestamp => 8,
            DataType::String => panic!("String size is variable at compile time"),
        }
    }
}

/// 时间相关辅助方法
pub mod time_utils {
    /// 将秒转换为毫秒
    pub const fn seconds_to_millis(seconds: u64) -> u64 {
        seconds * 1000
    }
    
    /// 将毫秒转换为秒
    pub const fn millis_to_seconds(millis: u64) -> u64 {
        millis / 1000
    }
    
    /// 将微秒转换为毫秒
    pub const fn micros_to_millis(micros: u64) -> u64 {
        micros / 1000
    }
    
    /// 将毫秒转换为微秒
    pub const fn millis_to_micros(millis: u64) -> u64 {
        millis * 1000
    }
    
    /// 将纳秒转换为毫秒
    pub const fn nanos_to_millis(nanos: u64) -> u64 {
        nanos / 1000000
    }
    
    /// 将毫秒转换为纳秒
    pub const fn millis_to_nanos(millis: u64) -> u64 {
        millis * 1000000
    }
    
    /// 计算两个时间戳之间的时间差（毫秒）
    pub fn time_diff(start: u64, end: u64) -> u64 {
        if end > start {
            end - start
        } else {
            start - end
        }
    }
    
    /// 检查时间戳是否在指定范围内
    pub fn is_in_time_range(timestamp: u64, start: u64, end: u64) -> bool {
        timestamp >= start && timestamp <= end
    }
    
    /// 获取当前时间戳（毫秒）
    /// 注意：在no_std环境中，此函数需要平台支持
    #[cfg(feature = "std")]
    pub fn now_millis() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }
    
    /// 获取当前时间戳（微秒）
    /// 注意：在no_std环境中，此函数需要平台支持
    #[cfg(feature = "std")]
    pub fn now_micros() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_micros() as u64
    }
}

/// 通用值类型
#[repr(C)]
pub union Value {
    pub u8: u8,
    pub u16: u16,
    pub u32: u32,
    pub u64: u64,
    pub i8: i8,
    pub i16: i16,
    pub i32: i32,
    pub i64: i64,
    pub float32: f32,
    pub float64: f64,
    pub bool: bool,
    pub timestamp: u64,
    pub string: [u8; MAX_STRING_LEN],
}

/// 定长字符串最大长度
pub const MAX_STRING_LEN: usize = 64;

/// 字段定义
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FieldDef {
    /// 字段名称（编译时固定）
    pub name: &'static str,
    /// 数据类型
    pub data_type: DataType,
    /// 字段大小（字节）
    pub size: usize,
    /// 偏移量（在记录中的位置）
    pub offset: usize,
}

/// 索引类型枚举
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum IndexType {
    /// 哈希索引（仅用于主键）
    Hash = 0,
    /// 有序数组索引
    SortedArray = 1,
    /// B-Tree索引
    BTree = 2,
    /// T-Tree索引
    TTree = 3,
}

/// 表定义
#[derive(Copy, Clone)]
pub struct TableDef {
    /// 表ID
    pub id: u8,
    /// 表名称
    pub name: &'static str,
    /// 字段定义
    pub fields: &'static [FieldDef],
    /// 主键字段索引
    pub primary_key: usize,
    /// 辅助索引字段索引（可选）
    pub secondary_index: Option<usize>,
    /// 辅助索引类型
    pub secondary_index_type: IndexType,
    /// 单条记录大小
    pub record_size: usize,
    /// 最大记录数
    pub max_records: usize,
}

/// 记录状态
#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum RecordStatus {
    /// 空闲
    Free = 0,
    /// 已占用
    Used = 1,
    /// 已删除（标记）
    Deleted = 2,
}

/// 锁类型
#[repr(u8)]
pub enum LockType {
    /// 无锁
    None = 0,
    /// 共享锁（读锁）
    Shared = 1,
    /// 排他锁（写锁）
    Exclusive = 2,
}

/// 记录头
#[repr(C)]
pub struct RecordHeader {
    /// 记录状态
    pub status: RecordStatus,
    /// 版本号（用于事务）
    pub version: u16,
    /// 锁类型
    pub lock_type: LockType,
    /// 持有锁的事务ID
    pub lock_owner: u32,
    /// 锁计数器（用于共享锁）
    pub lock_count: u8,
}

impl RecordHeader {
    /// 记录头大小
    pub const SIZE: usize = size_of::<Self>();
}

/// 确保所有类型大小正确
const _: () = {
    assert!(size_of::<RecordStatus>() == 1);
    assert!(size_of::<DataType>() == 1);
};

/// 错误类型
#[derive(Debug, PartialEq, Eq)]
pub enum RemDbError {
    /// 内存不足
    OutOfMemory,
    /// 记录未找到
    RecordNotFound,
    /// 主键重复
    DuplicateKey,
    /// 字段不存在
    FieldNotFound,
    /// 类型不匹配
    TypeMismatch,
    /// 事务错误
    TransactionError,
    /// 配置错误
    ConfigError,
    /// 操作不支持
    UnsupportedOperation,
    /// 文件I/O错误
    FileIoError,
    /// 快照格式错误
    SnapshotFormatError,
    /// CRC校验失败
    Crc32Error,
    /// 日志格式错误
    LogFormatError,
    /// 日志记录未找到
    LogRecordNotFound,
    /// 日志校验和错误
    LogChecksumError,
    /// 锁冲突
    LockConflict,
    /// 锁超时
    LockTimeout,
    /// 表未找到
    TableNotFound,
    /// 记录大小无效
    InvalidRecordSize,
    /// 无效的SQL查询
    InvalidSqlQuery,
}

impl fmt::Display for RemDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemDbError::OutOfMemory => write!(f, "Out of memory"),
            RemDbError::RecordNotFound => write!(f, "Record not found"),
            RemDbError::DuplicateKey => write!(f, "Duplicate key"),
            RemDbError::FieldNotFound => write!(f, "Field not found"),
            RemDbError::TypeMismatch => write!(f, "Type mismatch"),
            RemDbError::TransactionError => write!(f, "Transaction error"),
            RemDbError::ConfigError => write!(f, "Config error"),
            RemDbError::UnsupportedOperation => write!(f, "Unsupported operation"),
            RemDbError::FileIoError => write!(f, "File I/O error"),
            RemDbError::SnapshotFormatError => write!(f, "Snapshot format error"),
            RemDbError::Crc32Error => write!(f, "CRC32 checksum error"),
            RemDbError::LogFormatError => write!(f, "Log format error"),
            RemDbError::LogRecordNotFound => write!(f, "Log record not found"),
            RemDbError::LogChecksumError => write!(f, "Log checksum error"),
            RemDbError::LockConflict => write!(f, "Lock conflict"),
            RemDbError::LockTimeout => write!(f, "Lock timeout"),
            RemDbError::TableNotFound => write!(f, "Table not found"),
            RemDbError::InvalidRecordSize => write!(f, "Invalid record size"),
            RemDbError::InvalidSqlQuery => write!(f, "Invalid SQL query"),
        }
    }
}

/// 结果类型
pub type Result<T> = core::result::Result<T, RemDbError>;
