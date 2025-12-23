use core::fmt;
use core::mem::size_of;

/// 基本数据类型枚举
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum DataType {
    /// 8位有符号整数
    Int8 = 0,
    /// 16位有符号整数
    Int16 = 1,
    /// 32位有符号整数
    Int32 = 2,
    /// 64位有符号整数
    Int64 = 3,
    /// 32位浮点数
    Float32 = 4,
    /// 64位浮点数
    Float64 = 5,
    /// 布尔值
    Bool = 6,
    /// 时间戳（毫秒）
    Timestamp = 7,
    /// 定长字符串
    String = 8,
}

impl DataType {
    /// 获取数据类型的大小（字节）
    pub const fn size(&self) -> usize {
        match self {
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

/// 通用值类型
#[repr(C)]
pub union Value {
    pub int8: i8,
    pub int16: i16,
    pub int32: i32,
    pub int64: i64,
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

/// 表定义
#[derive(Copy, Clone)]
pub struct TableDef {
    /// 表名称
    pub name: &'static str,
    /// 字段定义
    pub fields: &'static [FieldDef],
    /// 主键字段索引
    pub primary_key: usize,
    /// 辅助索引字段索引（可选）
    pub secondary_index: Option<usize>,
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

/// 记录头
#[repr(C)]
pub struct RecordHeader {
    /// 记录状态
    pub status: RecordStatus,
    /// 版本号（用于事务）
    pub version: u16,
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
        }
    }
}

/// 结果类型
pub type Result<T> = core::result::Result<T, RemDbError>;
