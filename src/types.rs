use core::fmt;
use core::mem::size_of;
use crate::utf8::get_global_utf8_processor;

// 引入alloc模块
extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;

/// 向量距离度量类型
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Default)]
pub enum DistanceType {
    /// L2距离（欧几里得距离）
    #[default]
    L2 = 0,
    /// 内积
    InnerProduct = 1,
    /// 余弦相似度
    Cosine = 2,
}

/// 向量索引类型
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Default)]
pub enum VectorIndexType {
    /// HNSW索引（Hierarchical Navigable Small World）
    #[default]
    HNSW = 0,
    /// HNSW_SQ索引（带标量量化的HNSW）
    HNSW_SQ = 1,
    /// HNSW_BQ索引（带二进制量化的HNSW）
    HNSW_BQ = 2,
    /// IVF索引（Inverted File）
    IVF = 3,
    /// IVF_PQ索引（带乘积量化的IVF）
    IVF_PQ = 4,
}

/// 向量元数据
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct VectorMetadata {
    /// 向量维度
    pub dimension: u16,
    /// 距离度量类型
    pub distance_type: DistanceType,
    /// 索引类型
    pub index_type: VectorIndexType,
    /// 是否启用压缩（默认为false）
    pub compression_enabled: bool,
    /// 压缩方案（默认为0，无压缩）
    pub compression_scheme: u8,
    /// 压缩级别（默认为3）
    pub compression_level: u8,
    /// HNSW参数：每个节点的最大连接数
    pub hnsw_m: u8,
    /// HNSW参数：构建索引时的候选列表大小
    pub hnsw_ef_construction: u32,
    /// HNSW参数：搜索时的候选列表大小
    pub hnsw_ef_search: u32,
    /// IVF参数：聚类中心数量
    pub ivf_nlist: u32,
    /// IVF参数：搜索时检查的聚类中心数量
    pub ivf_nprobe: u32,
}

/// 为VectorMetadata实现自定义构造函数，兼容旧代码
impl VectorMetadata {
    /// 创建向量元数据（兼容旧代码，只需要维度、距离类型和索引类型）
    pub const fn new(
        dimension: u16,
        distance_type: DistanceType,
        index_type: VectorIndexType,
    ) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled: false,
            compression_scheme: 0,
            compression_level: 3,
            // HNSW默认参数
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 128,
            // IVF默认参数
            ivf_nlist: 1024,
            ivf_nprobe: 16,
        }
    }
    
    /// 创建向量元数据，支持部分字段初始化
    pub const fn with_compression(
        dimension: u16,
        distance_type: DistanceType,
        index_type: VectorIndexType,
        compression_enabled: bool,
        compression_scheme: u8,
        compression_level: u8,
    ) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled,
            compression_scheme,
            compression_level,
            // HNSW默认参数
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 128,
            // IVF默认参数
            ivf_nlist: 1024,
            ivf_nprobe: 16,
        }
    }
    
    /// 创建向量元数据，支持完整参数初始化
    pub const fn with_all_params(
        dimension: u16,
        distance_type: DistanceType,
        index_type: VectorIndexType,
        compression_enabled: bool,
        compression_scheme: u8,
        compression_level: u8,
        hnsw_m: u8,
        hnsw_ef_construction: u32,
        hnsw_ef_search: u32,
        ivf_nlist: u32,
        ivf_nprobe: u32,
    ) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled,
            compression_scheme,
            compression_level,
            hnsw_m,
            hnsw_ef_construction,
            hnsw_ef_search,
            ivf_nlist,
            ivf_nprobe,
        }
    }
}

/// JSON元数据
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct JsonMetadata {
    /// JSON路径（用于索引特定的路径）
    pub path: String,
    /// 值类型（如果路径指向特定类型）
    pub value_type: Option<DataType>,
    /// 是否创建虚拟生成列
    pub virtual_column: bool,
    /// 虚拟列名称（如果virtual_column为true）
    pub virtual_column_name: Option<String>,
    /// 索引配置
    pub index_config: JsonIndexConfig,
}

/// JSON索引配置
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct JsonIndexConfig {
    /// 索引类型（默认为BTree）
    pub index_type: IndexType,
    /// 是否启用路径索引
    pub path_index_enabled: bool,
    /// 索引的最大路径深度
    pub max_depth: u8,
    /// 是否索引数组元素
    pub index_array_elements: bool,
    /// 是否索引对象键
    pub index_object_keys: bool,
}

/// 为JsonMetadata实现构造函数
impl JsonMetadata {
    /// 创建JSON元数据
    pub fn new(path: String) -> Self {
        Self {
            path,
            value_type: None,
            virtual_column: false,
            virtual_column_name: None,
            index_config: JsonIndexConfig::default(),
        }
    }
    
    /// 创建带有虚拟生成列的JSON元数据
    pub fn with_virtual_column(path: String, column_name: String) -> Self {
        Self {
            path: path.clone(),
            value_type: None,
            virtual_column: true,
            virtual_column_name: Some(column_name),
            index_config: JsonIndexConfig::default(),
        }
    }
    
    /// 创建带有索引配置的JSON元数据
    pub fn with_index_config(
        path: String,
        index_config: JsonIndexConfig,
    ) -> Self {
        Self {
            path,
            value_type: None,
            virtual_column: false,
            virtual_column_name: None,
            index_config,
        }
    }
}

/// 为JsonIndexConfig实现默认值
impl Default for JsonIndexConfig {
    fn default() -> Self {
        Self {
            index_type: IndexType::BTree,
            path_index_enabled: true,
            max_depth: 10,
            index_array_elements: true,
            index_object_keys: true,
        }
    }
}

/// 允许使用元组语法初始化VectorMetadata，自动添加默认压缩字段
impl From<(u16, DistanceType, VectorIndexType)> for VectorMetadata {
    fn from((dimension, distance_type, index_type): (u16, DistanceType, VectorIndexType)) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled: false,
            compression_scheme: 0,
            compression_level: 3,
            // HNSW默认参数
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 128,
            // IVF默认参数
            ivf_nlist: 1024,
            ivf_nprobe: 16,
        }
    }
}

/// 允许使用元组语法初始化VectorMetadata，包含压缩字段
impl From<(u16, DistanceType, VectorIndexType, bool, u8, u8)> for VectorMetadata {
    fn from((dimension, distance_type, index_type, compression_enabled, compression_scheme, compression_level): (u16, DistanceType, VectorIndexType, bool, u8, u8)) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled,
            compression_scheme,
            compression_level,
            // HNSW默认参数
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 128,
            // IVF默认参数
            ivf_nlist: 1024,
            ivf_nprobe: 16,
        }
    }
}

/// 允许使用元组语法初始化VectorMetadata，包含完整参数
impl From<(u16, DistanceType, VectorIndexType, bool, u8, u8, u8, u32, u32, u32, u32)> for VectorMetadata {
    fn from((dimension, distance_type, index_type, compression_enabled, compression_scheme, compression_level, hnsw_m, hnsw_ef_construction, hnsw_ef_search, ivf_nlist, ivf_nprobe): (u16, DistanceType, VectorIndexType, bool, u8, u8, u8, u32, u32, u32, u32)) -> Self {
        Self {
            dimension,
            distance_type,
            index_type,
            compression_enabled,
            compression_scheme,
            compression_level,
            hnsw_m,
            hnsw_ef_construction,
            hnsw_ef_search,
            ivf_nlist,
            ivf_nprobe,
        }
    }
}

/// 基本数据类型枚举
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
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
    /// 时间戳（精度可调）
    Timestamp = 11,
    /// 带时区的时间戳（精度可调）
    TimestampTZ = 12,
    /// 可变长度字符串
    VarChar = 13,
    /// 固定长度字符串
    Char = 14,
    /// 大文本字符串
    Text = 15,
    /// 时间间隔
    Interval = 16,
    /// 向量类型
    Vector = 17,
    /// JSON类型
    Json = 18,
}

/// 实现从u8到DataType的转换
impl From<u8> for DataType {
    fn from(value: u8) -> Self {
        match value {
            0 => DataType::UInt8,
            1 => DataType::UInt16,
            2 => DataType::UInt32,
            3 => DataType::UInt64,
            4 => DataType::Int8,
            5 => DataType::Int16,
            6 => DataType::Int32,
            7 => DataType::Int64,
            8 => DataType::Float32,
            9 => DataType::Float64,
            10 => DataType::Bool,
            11 => DataType::Timestamp,
            12 => DataType::TimestampTZ,
            13 => DataType::VarChar,
            14 => DataType::Char,
            15 => DataType::Text,
            16 => DataType::Interval,
            17 => DataType::Vector,
            18 => DataType::Json,
            _ => DataType::VarChar, // 默认为VarChar类型
        }
    }
}

impl DataType {
    /// 获取数据类型的大小（字节）
    /// 时间类型根据精度自动调整大小：
    /// - 精度 0-2: 4字节（秒级）
    /// - 精度 3-5: 6字节（毫秒级）
    /// - 精度 6-8: 8字节（微秒级，默认）
    /// - 精度 9: 10字节（纳秒级）
    /// 向量类型：维度 * 4字节（float32）
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
            DataType::Timestamp => core::mem::size_of::<db_timestamp>(), // 实际大小，包括精度和标志
            DataType::TimestampTZ => core::mem::size_of::<db_timestamp>(), // 实际大小，包括精度和时区偏移
            DataType::Interval => core::mem::size_of::<db_interval>(), // 实际大小，包括精度和标志
            DataType::VarChar => panic!("VarChar size is variable at compile time"),
            DataType::Char => panic!("Char size is variable at compile time"),
            DataType::Text => panic!("Text size is variable at compile time"),
            DataType::Vector => panic!("Vector size depends on dimension at runtime"),
            DataType::Json => panic!("Json size is variable at runtime"),
        }
    }

    /// 将数据类型转换为SQL类型字符串
    pub fn to_sql_type(&self, size: usize) -> alloc::string::String {
        match self {
            // 整数类型
            DataType::UInt8 => "INTEGER".to_string(),
            DataType::UInt16 => "INTEGER".to_string(),
            DataType::UInt32 => "INTEGER".to_string(),
            DataType::UInt64 => "INTEGER".to_string(),
            DataType::Int8 => "INTEGER".to_string(),
            DataType::Int16 => "INTEGER".to_string(),
            DataType::Int32 => "INTEGER".to_string(),
            DataType::Int64 => "INTEGER".to_string(),
            // 浮点数类型
            DataType::Float32 => "REAL".to_string(),
            DataType::Float64 => "REAL".to_string(),
            // 布尔类型
            DataType::Bool => "BOOL".to_string(),
            // 时间类型
            DataType::Timestamp => "TIMESTAMP".to_string(),
            DataType::TimestampTZ => "TIMESTAMPTZ".to_string(),
            // 时间间隔类型
            DataType::Interval => "INTERVAL".to_string(),
            // 字符串类型
            DataType::VarChar => alloc::format!("VARCHAR({})", size),
            DataType::Char => alloc::format!("CHAR({})", size),
            DataType::Text => "TEXT".to_string(),
            // 向量类型
            DataType::Vector => "VECTOR".to_string(),
            // JSON类型
            DataType::Json => "JSON".to_string(),
        }
    }
}

impl Default for DataType {
    fn default() -> Self {
        DataType::Int32
    }
}

/// 时间间隔结构体
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct db_interval {
    /// 微秒数
    pub value: i64,
    /// 精度标记(0-9)
    pub precision: u8,
    /// 标志位
    pub flags: u8,
}

impl db_interval {
    /// 创建新的时间间隔
    pub const fn new(value: i64, precision: u8, flags: u8) -> Self {
        Self {
            value,
            precision,
            flags,
        }
    }

    /// 根据精度获取存储大小
    pub const fn storage_size(precision: u8) -> usize {
        match precision {
            0..=2 => 4, // 秒级
            3..=5 => 6, // 毫秒级
            6..=8 => 8, // 微秒级
            9 => 10,    // 纳秒级
            _ => 8,     // 默认微秒级
        }
    }

    /// 获取当前时间间隔的存储大小
    pub const fn size(&self) -> usize {
        Self::storage_size(self.precision)
    }
}

/// 时间戳结构体，根据设计方案实现
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct db_timestamp {
    /// 自2000-01-01的微秒数
    pub value: i64,
    /// 时区偏移（秒），TIMESTAMPTZ专用
    pub tz_offset: i16,
    /// 精度标记(0-9)
    pub precision: u8,
    /// 标志位
    pub flags: u8,
}

impl db_timestamp {
    /// 创建新的时间戳
    pub const fn new(value: i64, tz_offset: i16, precision: u8, flags: u8) -> Self {
        Self {
            value,
            tz_offset,
            precision,
            flags,
        }
    }

    /// 根据精度获取存储大小
    pub const fn storage_size(precision: u8) -> usize {
        match precision {
            0..=2 => 4, // 秒级
            3..=5 => 6, // 毫秒级
            6..=8 => 8, // 微秒级
            9 => 10,    // 纳秒级
            _ => 8,     // 默认微秒级
        }
    }

    /// 获取当前时间戳的存储大小
    pub const fn size(&self) -> usize {
        Self::storage_size(self.precision)
    }

    /// 时间戳加法运算
    pub fn add(&self, interval: &db_interval) -> Self {
        Self {
            value: self.value + interval.value,
            tz_offset: self.tz_offset,
            precision: core::cmp::max(self.precision, interval.precision),
            flags: self.flags,
        }
    }

    /// 时间戳减法运算
    pub fn sub(&self, interval: &db_interval) -> Self {
        Self {
            value: self.value - interval.value,
            tz_offset: self.tz_offset,
            precision: core::cmp::max(self.precision, interval.precision),
            flags: self.flags,
        }
    }

    /// 计算两个时间戳之间的时间差
    pub fn diff(&self, other: &db_timestamp) -> db_interval {
        let diff_value = self.value - other.value;
        let precision = core::cmp::max(self.precision, other.precision);
        db_interval::new(diff_value, precision, 0)
    }
}

/// JSON存储方式枚举
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum JsonStorage {
    /// 内联存储（最多64字节）
    Inline([u8; 64]),
    /// 外部存储（超过64字节）
    External {
        /// 内存池ID
        pool_id: u8,
        /// 偏移量
        offset: u32,
        /// 长度
        length: u32,
    },
    /// NULL值
    Null,
}

/// 时区信息
#[derive(Copy, Clone, Debug)]
pub struct TimeZone {
    /// 时区名称
    pub name: &'static str,
    /// 时区偏移（秒）
    pub offset: i32,
    /// 是否使用夏令时
    pub uses_dst: bool,
}

/// 内置时区列表
pub const TIME_ZONES: &[TimeZone] = &[
    TimeZone {
        name: "UTC",
        offset: 0,
        uses_dst: false,
    },
    TimeZone {
        name: "Asia/Shanghai",
        offset: 8 * 3600,
        uses_dst: false,
    },
    TimeZone {
        name: "America/New_York",
        offset: -5 * 3600,
        uses_dst: true,
    },
    TimeZone {
        name: "Europe/London",
        offset: 0,
        uses_dst: true,
    },
    TimeZone {
        name: "Asia/Tokyo",
        offset: 9 * 3600,
        uses_dst: false,
    },
];

/// 查找时区信息
pub fn find_timezone(name: &str) -> Option<TimeZone> {
    TIME_ZONES
        .iter()
        .find(|tz| tz.name.eq_ignore_ascii_case(name))
        .copied()
}

/// 转换时间戳到指定时区
/// 将TIMESTAMP转换为TIMESTAMPTZ，或调整TIMESTAMPTZ的时区
pub fn convert_timezone(timestamp: &db_timestamp, tz_offset: i16) -> db_timestamp {
    // 创建新的时间戳，保持原有精度和标志
    db_timestamp {
        value: timestamp.value,
        tz_offset: tz_offset,
        precision: timestamp.precision,
        flags: timestamp.flags,
    }
}

/// 根据时区名称获取时区偏移（秒）
pub fn get_timezone_offset(timezone_name: &str) -> Option<i16> {
    find_timezone(timezone_name).map(|tz| tz.offset as i16)
}

/// 根据时区偏移（秒）创建时区信息
pub fn create_timezone_from_offset(offset: i16) -> TimeZone {
    TimeZone {
        name: "UTC",
        offset: offset as i32,
        uses_dst: false,
    }
}

/// 时间格式化辅助函数
pub mod time_format {
    /// 将db_timestamp转换为ISO 8601格式字符串
    pub fn to_iso8601(_timestamp: &super::db_timestamp) -> alloc::string::String {
        // 实现ISO 8601格式化
        // 这里使用简化实现，实际应该根据精度和时区偏移进行完整格式化
        alloc::format!("2023-01-01T12:00:00.000000+00:00")
    }

    /// 将db_timestamp转换为指定格式的字符串
    pub fn to_char(timestamp: &super::db_timestamp, _format: &str) -> alloc::string::String {
        // 实现指定格式的格式化
        // 这里使用简化实现，实际应该支持各种格式说明符
        alloc::format!("{}", timestamp.value)
    }

    /// 将db_timestamp转换为 epoch 时间戳（秒）
    pub fn to_epoch(timestamp: &super::db_timestamp) -> f64 {
        // 转换为秒级epoch时间
        timestamp.value as f64 / 1000000.0
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
    pub timestamp: u64,        // 兼容旧版本
    pub time: db_timestamp,    // 新的时间戳类型
    pub interval: db_interval, // 时间间隔类型
    pub string: [u8; MAX_STRING_LEN],
    pub vector: *const f32,              // 向量类型（指向float32数组的指针）
    pub vector_metadata: VectorMetadata, // 向量元数据
    pub json_storage: JsonStorage,       // JSON存储
}

// 手动实现Clone trait，因为Rust不支持为union类型自动派生Clone
impl Clone for Value {
    fn clone(&self) -> Self {
        // 对于union，我们需要复制整个内存区域
        // 这是安全的，因为我们只是复制原始数据，不涉及指针解引用
        // 对于指针类型，我们只是复制指针值，不涉及所有权转移
        unsafe {
            core::mem::transmute_copy(self)
        }
    }
}

// 手动实现Debug trait，因为Rust不支持为union类型自动派生Debug
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 默认打印为u64值，实际使用时需要根据数据类型进行转换
        unsafe { write!(f, "Value(0x{:x})", self.u64) }
    }
}

// 手动实现Send和Sync trait，确保Value类型可以在多线程间安全共享
unsafe impl Send for Value {}
unsafe impl Sync for Value {}

// 手动实现Send和Sync trait，确保FieldDef类型可以在多线程间安全共享
unsafe impl Send for FieldDef {}
unsafe impl Sync for FieldDef {}

// 手动实现Send和Sync trait，确保TableDef类型可以在多线程间安全共享
unsafe impl Send for TableDef {}
unsafe impl Sync for TableDef {}

/// 带类型的值
pub struct TypedValue {
    /// 值的数据类型
    pub value_type: DataType,
    /// 实际值
    pub value: Value,
}

/// 手动实现Clone trait，因为Rust不支持为union类型自动派生Clone
impl Clone for TypedValue {
    fn clone(&self) -> Self {
        let mut new_value = Value {
            // Initialize with a default value
            u64: 0
        };
        
        unsafe {
            // Copy the appropriate field based on the value_type
            match self.value_type {
                DataType::UInt8 => new_value.u8 = self.value.u8,
                DataType::UInt16 => new_value.u16 = self.value.u16,
                DataType::UInt32 => new_value.u32 = self.value.u32,
                DataType::UInt64 => new_value.u64 = self.value.u64,
                DataType::Int8 => new_value.i8 = self.value.i8,
                DataType::Int16 => new_value.i16 = self.value.i16,
                DataType::Int32 => new_value.i32 = self.value.i32,
                DataType::Int64 => new_value.i64 = self.value.i64,
                DataType::Float32 => new_value.float32 = self.value.float32,
                DataType::Float64 => new_value.float64 = self.value.float64,
                DataType::Bool => new_value.bool = self.value.bool,
                DataType::Timestamp => new_value.time = self.value.time,
                DataType::TimestampTZ => new_value.time = self.value.time,
                DataType::Interval => new_value.interval = self.value.interval,
                DataType::VarChar | DataType::Char | DataType::Text => new_value.string = self.value.string,
                DataType::Vector => {
                    // For vectors, we don't copy the actual vector data,
                    // just copy the pointer
                    new_value.vector = self.value.vector;
                }
                DataType::Json => {
                    // For JSON, copy the storage information
                    new_value.json_storage = self.value.json_storage;
                }
            }
        }
        
        TypedValue {
            value_type: self.value_type,
            value: new_value,
        }
    }
}

/// 手动实现PartialEq trait，因为Rust不支持为union类型自动派生PartialEq
impl PartialEq for TypedValue {
    fn eq(&self, other: &Self) -> bool {
        // 首先比较类型
        if self.value_type != other.value_type {
            return false;
        }

        unsafe {
            match self.value_type {
                DataType::UInt8 => self.value.u8 == other.value.u8,
                DataType::UInt16 => self.value.u16 == other.value.u16,
                DataType::UInt32 => self.value.u32 == other.value.u32,
                DataType::UInt64 => self.value.u64 == other.value.u64,
                DataType::Int8 => self.value.i8 == other.value.i8,
                DataType::Int16 => self.value.i16 == other.value.i16,
                DataType::Int32 => self.value.i32 == other.value.i32,
                DataType::Int64 => self.value.i64 == other.value.i64,
                DataType::Float32 => {
                    // 处理浮点数的特殊比较：NaN 和无穷大
                    let a = self.value.float32;
                    let b = other.value.float32;
                    if a.is_nan() && b.is_nan() {
                        true // 两个都是 NaN 时认为相等
                    } else {
                        a == b
                    }
                }
                DataType::Float64 => {
                    let a = self.value.float64;
                    let b = other.value.float64;
                    if a.is_nan() && b.is_nan() {
                        true
                    } else {
                        a == b
                    }
                }
                DataType::Bool => self.value.bool == other.value.bool,
                DataType::Timestamp => self.value.time.value == other.value.time.value,
                DataType::TimestampTZ => {
                    // 比较值和时区偏移
                    self.value.time.value == other.value.time.value
                        && self.value.time.tz_offset == other.value.time.tz_offset
                }
                DataType::Interval => self.value.interval.value == other.value.interval.value,
                DataType::VarChar | DataType::Char | DataType::Text => {
                    // 使用UTF-8处理器比较字符串
                    let a_str = self.value.string.as_ref();
                    let b_str = other.value.string.as_ref();
                    get_global_utf8_processor().compare(a_str, b_str) == core::cmp::Ordering::Equal
                }
                DataType::Vector => {
                    // 向量比较：比较向量指针
                    // 注意：实际使用中需要比较向量的每个元素，但这需要向量维度信息
                    // 由于这是一个简化实现，我们比较向量指针
                    self.value.vector == other.value.vector
                }
                DataType::Json => {
                    // JSON比较：比较存储信息
                    self.value.json_storage == other.value.json_storage
                }
            }
        }
    }
}

/// 手动实现Eq trait，因为Rust不支持为union类型自动派生Eq
impl Eq for TypedValue {}

/// 手动实现Hash trait，用于在HashSet中使用
use core::hash::{Hash, Hasher};

impl Hash for TypedValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // 首先哈希类型
        self.value_type.hash(state);

        unsafe {
            match self.value_type {
                DataType::UInt8 => self.value.u8.hash(state),
                DataType::UInt16 => self.value.u16.hash(state),
                DataType::UInt32 => self.value.u32.hash(state),
                DataType::UInt64 => self.value.u64.hash(state),
                DataType::Int8 => self.value.i8.hash(state),
                DataType::Int16 => self.value.i16.hash(state),
                DataType::Int32 => self.value.i32.hash(state),
                DataType::Int64 => self.value.i64.hash(state),
                DataType::Float32 => {
                    // 处理浮点数的特殊情况：NaN 和无穷大
                    let a = self.value.float32;
                    if a.is_nan() {
                        // 所有NaN使用相同的哈希值
                        state.write_u32(0x7FC00000);
                    } else {
                        a.to_bits().hash(state);
                    }
                }
                DataType::Float64 => {
                    let a = self.value.float64;
                    if a.is_nan() {
                        // 所有NaN使用相同的哈希值
                        state.write_u64(0x7FF8000000000000);
                    } else {
                        a.to_bits().hash(state);
                    }
                }
                DataType::Bool => self.value.bool.hash(state),
                DataType::Timestamp => self.value.time.value.hash(state),
                DataType::TimestampTZ => {
                    // 哈希值和时区偏移
                    self.value.time.value.hash(state);
                    self.value.time.tz_offset.hash(state);
                }
                DataType::Interval => self.value.interval.value.hash(state),
                DataType::VarChar | DataType::Char | DataType::Text => {
                    // 使用UTF-8处理器哈希字符串内容
                    if let Some(s) = get_global_utf8_processor().to_string(&self.value.string) {
                        s.trim_end_matches(char::from(0)).hash(state);
                    } else {
                        // 如果转换失败，回退到字节哈希
                        self.value.string.hash(state);
                    }
                }
                DataType::Vector => {
                    // 向量哈希：哈希向量指针
                    // 注意：实际使用中可能需要哈希向量的部分或全部元素
                    self.value.vector.hash(state);
                }
                DataType::Json => {
                    // JSON哈希：哈希存储信息
                    self.value.json_storage.hash(state);
                }
            }
        }
    }
}

/// 手动实现PartialOrd trait，用于比较TypedValue
impl PartialOrd for TypedValue {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        // 首先比较类型
        match self.value_type.cmp(&other.value_type) {
            core::cmp::Ordering::Equal => {
                // 类型相同，比较值
                unsafe {
                    match self.value_type {
                        DataType::UInt8 => Some(self.value.u8.cmp(&other.value.u8)),
                        DataType::UInt16 => Some(self.value.u16.cmp(&other.value.u16)),
                        DataType::UInt32 => Some(self.value.u32.cmp(&other.value.u32)),
                        DataType::UInt64 => Some(self.value.u64.cmp(&other.value.u64)),
                        DataType::Int8 => Some(self.value.i8.cmp(&other.value.i8)),
                        DataType::Int16 => Some(self.value.i16.cmp(&other.value.i16)),
                        DataType::Int32 => Some(self.value.i32.cmp(&other.value.i32)),
                        DataType::Int64 => Some(self.value.i64.cmp(&other.value.i64)),
                        DataType::Float32 => {
                            // 处理浮点数的特殊情况：NaN
                            let a = self.value.float32;
                            let b = other.value.float32;
                            if a.is_nan() || b.is_nan() {
                                None // NaN 无法比较
                            } else {
                                Some(a.partial_cmp(&b).unwrap())
                            }
                        }
                        DataType::Float64 => {
                            let a = self.value.float64;
                            let b = other.value.float64;
                            if a.is_nan() || b.is_nan() {
                                None // NaN 无法比较
                            } else {
                                Some(a.partial_cmp(&b).unwrap())
                            }
                        }
                        DataType::Bool => Some(self.value.bool.cmp(&other.value.bool)),
                        DataType::Timestamp => {
                            Some(self.value.time.value.cmp(&other.value.time.value))
                        }
                        DataType::TimestampTZ => {
                            // 先比较时间值，再比较时区偏移
                            match self.value.time.value.cmp(&other.value.time.value) {
                                core::cmp::Ordering::Equal => {
                                    Some(self.value.time.tz_offset.cmp(&other.value.time.tz_offset))
                                }
                                ordering => Some(ordering),
                            }
                        }
                        DataType::Interval => {
                    Some(self.value.interval.value.cmp(&other.value.interval.value))
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    // 使用UTF-8处理器比较字符串
                    let a_str = self.value.string.as_ref();
                    let b_str = other.value.string.as_ref();
                    Some(get_global_utf8_processor().compare(a_str, b_str))
                }
                DataType::Vector => {
                            // 向量比较：比较向量指针
                            // 注意：实际使用中可能需要比较向量的距离或相似度
                            Some(self.value.vector.cmp(&other.value.vector))
                        }
                        DataType::Json => {
                            // JSON比较：基于存储信息的比较
                            // 注意：实际使用中可能需要深度比较JSON内容
                            Some(core::cmp::Ordering::Equal)
                        }
                    }
                }
            }
            ordering => Some(ordering),
        }
    }
}

/// 手动实现Ord trait，用于在BTreeMap中用作键
impl Ord for TypedValue {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.partial_cmp(other)
            .unwrap_or_else(|| {
                // 处理无法比较的情况（如NaN），将它们视为相等
                // 这确保在BTreeMap中NaN值不会导致崩溃
                core::cmp::Ordering::Equal
            })
    }
}

/// 手动实现Debug trait，因为Rust不支持为union类型自动派生Debug
impl fmt::Debug for TypedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            match self.value_type {
                DataType::UInt8 => write!(
                    f,
                    "TypedValue(UInt8, {})
",
                    self.value.u8
                ),
                DataType::UInt16 => write!(
                    f,
                    "TypedValue(UInt16, {})
",
                    self.value.u16
                ),
                DataType::UInt32 => write!(
                    f,
                    "TypedValue(UInt32, {})
",
                    self.value.u32
                ),
                DataType::UInt64 => write!(
                    f,
                    "TypedValue(UInt64, {})
",
                    self.value.u64
                ),
                DataType::Int8 => write!(
                    f,
                    "TypedValue(Int8, {})
",
                    self.value.i8
                ),
                DataType::Int16 => write!(
                    f,
                    "TypedValue(Int16, {})
",
                    self.value.i16
                ),
                DataType::Int32 => write!(
                    f,
                    "TypedValue(Int32, {})
",
                    self.value.i32
                ),
                DataType::Int64 => write!(
                    f,
                    "TypedValue(Int64, {})
",
                    self.value.i64
                ),
                DataType::Float32 => write!(
                    f,
                    "TypedValue(Float32, {})
",
                    self.value.float32
                ),
                DataType::Float64 => write!(
                    f,
                    "TypedValue(Float64, {})
",
                    self.value.float64
                ),
                DataType::Bool => write!(
                    f,
                    "TypedValue(Bool, {})
",
                    self.value.bool
                ),
                DataType::Timestamp => {
                    write!(
                        f,
                        "TypedValue(Timestamp, value: {}, precision: {})
",
                        self.value.time.value, self.value.time.precision
                    )
                }
                DataType::TimestampTZ => {
                    write!(
                        f,
                        "TypedValue(TimestampTZ, value: {}, tz_offset: {}s, precision: {})
",
                        self.value.time.value, self.value.time.tz_offset, self.value.time.precision
                    )
                }
                DataType::VarChar | DataType::Char | DataType::Text => {
                    let s = get_global_utf8_processor().to_string(&self.value.string)
                        .unwrap_or("")
                        .trim_end_matches(char::from(0));
                    write!(
                        f,
                        "TypedValue({}, \"{}\")\n",
                        self.value_type.to_sql_type(0).as_str(),
                        s
                    )
                }
                DataType::Interval => {
                    write!(
                        f,
                        "TypedValue(Interval, value: {}, precision: {})
",
                        self.value.interval.value, self.value.interval.precision
                    )
                }
                DataType::Vector => {
                    write!(
                        f,
                        "TypedValue(Vector, pointer: {:?})",
                        self.value.vector
                    )
                }
                DataType::Json => {
                    write!(
                        f,
                        "TypedValue(Json, storage: {:?})",
                        self.value.json_storage
                    )
                }
            }
        }
    }
}

/// 定长字符串最大长度
pub const MAX_STRING_LEN: usize = 64;

/// 字段定义
#[derive(Clone, Debug)]
pub struct FieldDef {
    /// 字段名称
    pub name: String,
    /// 数据类型
    pub data_type: DataType,
    /// 字段大小（字节）
    pub size: usize,
    /// 字符串类型的长度限制（仅适用于 VARCHAR 和 CHAR 类型）
    pub string_length: Option<usize>,
    /// 偏移量（在记录中的位置）
    pub offset: usize,
    /// 是否为主键
    pub primary_key: bool,
    /// 是否非空
    pub not_null: bool,
    /// 是否唯一
    pub unique: bool,
    /// 是否自增
    pub auto_increment: bool,
    /// 默认值
    pub default_value: Option<Value>,
    /// 向量元数据（仅向量类型使用）
    pub vector_metadata: Option<VectorMetadata>,
    /// JSON元数据（仅JSON类型使用）
    pub json_metadata: Option<JsonMetadata>,
}

impl Default for FieldDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            data_type: DataType::Int32,
            size: 0,
            string_length: None,
            offset: 0,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        }
    }
}

impl FieldDef {
    /// 生成字段的SQL约束字符串
    pub fn constraints_to_sql(&self) -> alloc::string::String {
        let mut constraints = alloc::string::String::new();

        if self.primary_key {
            constraints.push_str(" PRIMARY KEY");
        }

        if self.auto_increment {
            constraints.push_str(" AUTO_INCREMENT");
        }

        if self.not_null {
            constraints.push_str(" NOT NULL");
        }

        if self.unique && !self.primary_key {
            constraints.push_str(" UNIQUE");
        }

        if let Some(default) = &self.default_value {
            constraints.push_str(" DEFAULT ");
            unsafe {
                match self.data_type {
                    DataType::VarChar | DataType::Char | DataType::Text => {
                        let s = get_global_utf8_processor().to_string(&default.string)
                            .unwrap_or("")
                            .trim_end_matches(char::from(0));
                        constraints.push_str(&alloc::format!("'{}'", s));
                    }
                    DataType::Bool => {
                        let b = default.bool;
                        constraints.push_str(if b { "TRUE" } else { "FALSE" });
                    }
                    DataType::UInt8 => constraints.push_str(&default.u8.to_string()),
                    DataType::UInt16 => constraints.push_str(&default.u16.to_string()),
                    DataType::UInt32 => constraints.push_str(&default.u32.to_string()),
                    DataType::UInt64 => constraints.push_str(&default.u64.to_string()),
                    DataType::Int8 => constraints.push_str(&default.i8.to_string()),
                    DataType::Int16 => constraints.push_str(&default.i16.to_string()),
                    DataType::Int32 => constraints.push_str(&default.i32.to_string()),
                    DataType::Int64 => constraints.push_str(&default.i64.to_string()),
                    DataType::Float32 => constraints.push_str(&default.float32.to_string()),
                    DataType::Float64 => constraints.push_str(&default.float64.to_string()),
                    DataType::Timestamp => constraints.push_str(&default.timestamp.to_string()),
                    DataType::TimestampTZ => constraints.push_str(&default.timestamp.to_string()),
                    DataType::Interval => constraints.push_str(&default.interval.value.to_string()),
                    DataType::Vector => {
                        constraints.push_str("NULL"); // 向量类型暂不支持默认值
                    }
                    DataType::Json => {
                        constraints.push_str("NULL"); // JSON类型暂不支持默认值
                    }
                }
            }
        }

        // 向量类型特殊处理：添加距离度量和索引类型
        if self.data_type == DataType::Vector {
            if let Some(meta) = self.vector_metadata {
                let distance_str = match meta.distance_type {
                    DistanceType::L2 => "L2",
                    DistanceType::InnerProduct => "INNER_PRODUCT",
                    DistanceType::Cosine => "COSINE",
                };
                constraints.push_str(&alloc::format!(" WITH DISTANCE={}", distance_str));
            }
        }

        constraints
    }
}

/// 索引类型枚举
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Ord, PartialOrd)]
pub enum IndexType {
    /// 哈希索引（仅用于主键）
    Hash = 0,
    /// 有序数组索引
    SortedArray = 1,
    /// B-Tree索引
    BTree = 2,
    /// T-Tree索引
    TTree = 3,
    /// 向量索引
    Vector = 4,
    /// JSON索引（虚拟生成列和路径索引）
    Json = 5,
}

/// 为IndexType实现From<u8> trait，允许从u8转换为IndexType
impl From<u8> for IndexType {
    fn from(value: u8) -> Self {
        match value {
            0 => IndexType::Hash,
            1 => IndexType::SortedArray,
            2 => IndexType::BTree,
            3 => IndexType::TTree,
            4 => IndexType::Vector,
            5 => IndexType::Json,
            _ => IndexType::SortedArray, // 默认使用SortedArray
        }
    }
}

/// 表定义
#[derive(Clone, Debug)]
pub struct TableDef {
    /// 表ID
    pub id: u8,
    /// 表名称
    pub name: String,
    /// 字段定义
    pub fields: Vec<FieldDef>,
    /// 主键字段索引列表（复合主键）
    pub primary_key: Vec<usize>,
    /// 辅助索引字段索引列表（复合索引）
    pub secondary_index: Option<Vec<usize>>,
    /// 辅助索引类型
    pub secondary_index_type: IndexType,
    /// 单条记录大小
    pub record_size: usize,
    /// 最大记录数
    pub max_records: usize,
    /// 表结构版本号，用于跟踪表结构变更
    pub version: u32,
    /// 表创建时间戳
    pub created_at: u64,
    /// 表最后修改时间戳
    pub updated_at: u64,
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
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
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
    /// 创建事务ID（用于MVCC）
    pub create_tx_id: u32,
    /// 删除事务ID（用于MVCC，0表示未删除）
    pub delete_tx_id: u32,
    /// 下一个版本的指针（用于MVCC版本链）
    pub next_version_ptr: usize,
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
    /// NOT NULL约束违反
    NotNullViolation,
    /// 事务错误
    TransactionError,
    /// 配置错误
    ConfigError,
    /// 不支持多个索引
    TwoMoreIndexNotSupported,
    /// 操作不支持
    UnsupportedOperation,
    /// 文件I/O错误
    FileIoError,
    /// 数据库不存在
    DatabaseNotFound,
    /// 数据库已存在
    DatabaseExists,
    /// 数据库已关闭
    DatabaseClosed,
    /// 数据库数量达到上限
    MaxDatabasesReached,
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
    /// 没有可覆盖的记录
    NoRecordsToOverwrite,
    /// 内部错误
    InternalError,
}

impl fmt::Display for RemDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemDbError::OutOfMemory => write!(f, "Out of memory"),
            RemDbError::RecordNotFound => write!(f, "Record not found"),
            RemDbError::DuplicateKey => write!(f, "Duplicate key"),
            RemDbError::FieldNotFound => write!(f, "Field not found"),
            RemDbError::TypeMismatch => write!(f, "Type mismatch"),
            RemDbError::NotNullViolation => write!(f, "NOT NULL constraint violation"),
            RemDbError::TransactionError => write!(f, "Transaction error"),
            RemDbError::ConfigError => write!(f, "Config error"),
            RemDbError::TwoMoreIndexNotSupported => write!(f, "Two more index not supported"),
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
            RemDbError::NoRecordsToOverwrite => write!(f, "No records to overwrite"),
            RemDbError::DatabaseNotFound => write!(f, "Database not found"),
            RemDbError::DatabaseExists => write!(f, "Database exists"),
            RemDbError::DatabaseClosed => write!(f, "Database closed"),
            RemDbError::MaxDatabasesReached => write!(f, "Maximum databases reached"),
            RemDbError::InternalError => write!(f, "Internal error"),
        }
    }
}

/// 结果类型
pub type Result<T> = core::result::Result<T, RemDbError>;
