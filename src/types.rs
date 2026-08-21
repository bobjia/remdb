#![allow(unsafe_code)]

use core::fmt;
use core::mem::size_of;

// 引入alloc模块
extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;

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
    /// 定长字符串
    String = 13,
    /// 时间间隔
    Interval = 14,
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
            13 => DataType::String,
            14 => DataType::Interval,
            _ => DataType::String, // 默认为String类型
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
            DataType::Timestamp => core::mem::size_of::<db_timestamp>(),  // 实际大小，包括精度和标志
            DataType::TimestampTZ => core::mem::size_of::<db_timestamp>(), // 实际大小，包括精度和时区偏移
            DataType::Interval => core::mem::size_of::<db_interval>(),   // 实际大小，包括精度和标志
            DataType::String => panic!("String size is variable at compile time"),
        }
    }
    
    /// 将数据类型转换为SQL类型字符串
    pub fn to_sql_type(&self, size: usize) -> &'static str {
        match self {
            // 整数类型
            DataType::UInt8 => "INTEGER",
            DataType::UInt16 => "INTEGER",
            DataType::UInt32 => "INTEGER",
            DataType::UInt64 => "INTEGER",
            DataType::Int8 => "INTEGER",
            DataType::Int16 => "INTEGER",
            DataType::Int32 => "INTEGER",
            DataType::Int64 => "INTEGER",
            // 浮点数类型
            DataType::Float32 => "REAL",
            DataType::Float64 => "REAL",
            // 布尔类型
            DataType::Bool => "BOOL",
            // 时间类型
            DataType::Timestamp => "TIMESTAMP",
            DataType::TimestampTZ => "TIMESTAMPTZ",
            // 时间间隔类型
            DataType::Interval => "INTERVAL",
            // 字符串类型
            DataType::String => "TEXT",
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
            0..=2 => 4,   // 秒级
            3..=5 => 6,   // 毫秒级
            6..=8 => 8,   // 微秒级
            9 => 10,      // 纳秒级
            _ => 8,       // 默认微秒级
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
            0..=2 => 4,   // 秒级
            3..=5 => 6,   // 毫秒级
            6..=8 => 8,   // 微秒级
            9 => 10,      // 纳秒级
            _ => 8,       // 默认微秒级
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
    TimeZone { name: "UTC", offset: 0, uses_dst: false },
    TimeZone { name: "Asia/Shanghai", offset: 8 * 3600, uses_dst: false },
    TimeZone { name: "America/New_York", offset: -5 * 3600, uses_dst: true },
    TimeZone { name: "Europe/London", offset: 0, uses_dst: true },
    TimeZone { name: "Asia/Tokyo", offset: 9 * 3600, uses_dst: false },
];

/// 查找时区信息
pub fn find_timezone(name: &str) -> Option<TimeZone> {
    TIME_ZONES.iter().find(|tz| tz.name.eq_ignore_ascii_case(name)).copied()
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
    find_timezone(timezone_name)
        .map(|tz| tz.offset as i16)
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
    pub fn to_iso8601(timestamp: &super::db_timestamp) -> alloc::string::String {
        // 实现ISO 8601格式化
        // 这里使用简化实现，实际应该根据精度和时区偏移进行完整格式化
        alloc::format!("2023-01-01T12:00:00.000000+00:00")
    }
    
    /// 将db_timestamp转换为指定格式的字符串
    pub fn to_char(timestamp: &super::db_timestamp, format: &str) -> alloc::string::String {
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
#[derive(Copy, Clone)]
pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    Timestamp(u64),
    Time(db_timestamp),
    Interval(db_interval),
    String([u8; MAX_STRING_LEN]),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::U8(v) => write!(f, "U8({})", v),
            Value::U16(v) => write!(f, "U16({})", v),
            Value::U32(v) => write!(f, "U32({})", v),
            Value::U64(v) => write!(f, "U64({})", v),
            Value::I8(v) => write!(f, "I8({})", v),
            Value::I16(v) => write!(f, "I16({})", v),
            Value::I32(v) => write!(f, "I32({})", v),
            Value::I64(v) => write!(f, "I64({})", v),
            Value::Float32(v) => write!(f, "Float32({})", v),
            Value::Float64(v) => write!(f, "Float64({})", v),
            Value::Bool(v) => write!(f, "Bool({})", v),
            Value::Timestamp(v) => write!(f, "Timestamp({})", v),
            Value::Time(v) => write!(f, "Time({:?})", v),
            Value::Interval(v) => write!(f, "Interval({:?})", v),
            Value::String(v) => {
                let s = core::str::from_utf8(v).unwrap_or("<invalid utf8>");
                write!(f, "String({:?})", s)
            }
        }
    }
}

impl Value {
    pub fn as_u8(&self) -> u8 { match self { Value::U8(v) => *v, _ => 0 } }
    pub fn as_u16(&self) -> u16 { match self { Value::U16(v) => *v, _ => 0 } }
    pub fn as_u32(&self) -> u32 { match self { Value::U32(v) => *v, _ => 0 } }
    pub fn as_u64(&self) -> u64 { match self { Value::U64(v) => *v, _ => 0 } }
    pub fn as_i8(&self) -> i8 { match self { Value::I8(v) => *v, _ => 0 } }
    pub fn as_i16(&self) -> i16 { match self { Value::I16(v) => *v, _ => 0 } }
    pub fn as_i32(&self) -> i32 { match self { Value::I32(v) => *v, _ => 0 } }
    pub fn as_i64(&self) -> i64 { match self { Value::I64(v) => *v, _ => 0 } }
    pub fn as_float32(&self) -> f32 { match self { Value::Float32(v) => *v, _ => 0.0 } }
    pub fn as_float64(&self) -> f64 { match self { Value::Float64(v) => *v, _ => 0.0 } }
    pub fn as_bool(&self) -> bool { match self { Value::Bool(v) => *v, _ => false } }
    pub fn as_timestamp(&self) -> u64 { match self { Value::Timestamp(v) => *v, Value::Time(v) => v.value as u64, _ => 0 } }
    pub fn as_time(&self) -> db_timestamp { match self { Value::Time(v) => *v, Value::Timestamp(v) => db_timestamp::new(*v as i64, 0, 0, 0), _ => db_timestamp::new(0, 0, 0, 0) } }
    pub fn as_interval(&self) -> db_interval { match self { Value::Interval(v) => *v, _ => db_interval::new(0, 0, 0) } }
    pub fn as_string(&self) -> &[u8; MAX_STRING_LEN] {
        match self {
            Value::String(v) => v,
            _ => {
                const EMPTY_STRING: [u8; MAX_STRING_LEN] = [0u8; MAX_STRING_LEN];
                &EMPTY_STRING
            }
        }
    }
    pub fn as_string_mut(&mut self) -> &mut [u8; MAX_STRING_LEN] {
        match self {
            Value::String(v) => v,
            _ => panic!("Value is not a string"),
        }
    }
}

/// 带类型的值
#[derive(Copy, Clone)]
pub struct TypedValue {
    /// 值的数据类型
    pub value_type: DataType,
    /// 实际值
    pub value: Value,
}

/// 手动实现PartialEq trait，因为Rust不支持为union类型自动派生PartialEq
impl PartialEq for TypedValue {
    fn eq(&self, other: &Self) -> bool {
        // 首先比较类型
        if self.value_type != other.value_type {
            return false;
        }
        
        {
            match self.value_type {
                DataType::UInt8 => self.value.as_u8() == other.value.as_u8(),
                DataType::UInt16 => self.value.as_u16() == other.value.as_u16(),
                DataType::UInt32 => self.value.as_u32() == other.value.as_u32(),
                DataType::UInt64 => self.value.as_u64() == other.value.as_u64(),
                DataType::Int8 => self.value.as_i8() == other.value.as_i8(),
                DataType::Int16 => self.value.as_i16() == other.value.as_i16(),
                DataType::Int32 => self.value.as_i32() == other.value.as_i32(),
                DataType::Int64 => self.value.as_i64() == other.value.as_i64(),
                DataType::Float32 => {
                    // 处理浮点数的特殊比较：NaN 和无穷大
                    let a = self.value.as_float32();
                    let b = other.value.as_float32();
                    if a.is_nan() && b.is_nan() {
                        true // 两个都是 NaN 时认为相等
                    } else {
                        a == b
                    }
                }
                DataType::Float64 => {
                    let a = self.value.as_float64();
                    let b = other.value.as_float64();
                    if a.is_nan() && b.is_nan() {
                        true
                    } else {
                        a == b
                    }
                }
                DataType::Bool => self.value.as_bool() == other.value.as_bool(),
                DataType::Timestamp => self.value.as_time().value == other.value.as_time().value,
                DataType::TimestampTZ => {
                    // 比较值和时区偏移
                    self.value.as_time().value == other.value.as_time().value && 
                    self.value.as_time().tz_offset == other.value.as_time().tz_offset
                },
                DataType::Interval => self.value.as_interval().value == other.value.as_interval().value,
                DataType::String => {
                    // 比较字符串数组
                    let a_str = core::str::from_utf8(self.value.as_string()).unwrap_or("");
                    let b_str = core::str::from_utf8(other.value.as_string()).unwrap_or("");
                    a_str.trim_end_matches(char::from(0)) == b_str.trim_end_matches(char::from(0))
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
        
        {
            match self.value_type {
                DataType::UInt8 => self.value.as_u8().hash(state),
                DataType::UInt16 => self.value.as_u16().hash(state),
                DataType::UInt32 => self.value.as_u32().hash(state),
                DataType::UInt64 => self.value.as_u64().hash(state),
                DataType::Int8 => self.value.as_i8().hash(state),
                DataType::Int16 => self.value.as_i16().hash(state),
                DataType::Int32 => self.value.as_i32().hash(state),
                DataType::Int64 => self.value.as_i64().hash(state),
                DataType::Float32 => {
                    // 处理浮点数的特殊情况：NaN 和无穷大
                    let a = self.value.as_float32();
                    if a.is_nan() {
                        // 所有NaN使用相同的哈希值
                        state.write_u32(0x7FC00000);
                    } else {
                        a.to_bits().hash(state);
                    }
                },
                DataType::Float64 => {
                    let a = self.value.as_float64();
                    if a.is_nan() {
                        // 所有NaN使用相同的哈希值
                        state.write_u64(0x7FF8000000000000);
                    } else {
                        a.to_bits().hash(state);
                    }
                },
                DataType::Bool => self.value.as_bool().hash(state),
                DataType::Timestamp => self.value.as_time().value.hash(state),
                DataType::TimestampTZ => {
                    // 哈希值和时区偏移
                    self.value.as_time().value.hash(state);
                    self.value.as_time().tz_offset.hash(state);
                },
                DataType::Interval => self.value.as_interval().value.hash(state),
                DataType::String => {
                    // 哈希字符串内容，忽略末尾的空字符
                    let s = core::str::from_utf8(self.value.as_string()).unwrap_or("");
                    s.trim_end_matches(char::from(0)).hash(state);
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
                {
                    match self.value_type {
                        DataType::UInt8 => Some(self.value.as_u8().cmp(&other.value.as_u8())),
                        DataType::UInt16 => Some(self.value.as_u16().cmp(&other.value.as_u16())),
                        DataType::UInt32 => Some(self.value.as_u32().cmp(&other.value.as_u32())),
                        DataType::UInt64 => Some(self.value.as_u64().cmp(&other.value.as_u64())),
                        DataType::Int8 => Some(self.value.as_i8().cmp(&other.value.as_i8())),
                        DataType::Int16 => Some(self.value.as_i16().cmp(&other.value.as_i16())),
                        DataType::Int32 => Some(self.value.as_i32().cmp(&other.value.as_i32())),
                        DataType::Int64 => Some(self.value.as_i64().cmp(&other.value.as_i64())),
                        DataType::Float32 => {
                            // 处理浮点数的特殊情况：NaN
                            let a = self.value.as_float32();
                            let b = other.value.as_float32();
                            if a.is_nan() || b.is_nan() {
                                None // NaN 无法比较
                            } else {
                                Some(a.partial_cmp(&b).unwrap())
                            }
                        },
                        DataType::Float64 => {
                            let a = self.value.as_float64();
                            let b = other.value.as_float64();
                            if a.is_nan() || b.is_nan() {
                                None // NaN 无法比较
                            } else {
                                Some(a.partial_cmp(&b).unwrap())
                            }
                        },
                        DataType::Bool => Some(self.value.as_bool().cmp(&other.value.as_bool())),
                        DataType::Timestamp => Some(self.value.as_time().value.cmp(&other.value.as_time().value)),
                        DataType::TimestampTZ => {
                            // 先比较时间值，再比较时区偏移
                            match self.value.as_time().value.cmp(&other.value.as_time().value) {
                                core::cmp::Ordering::Equal => {
                                    Some(self.value.as_time().tz_offset.cmp(&other.value.as_time().tz_offset))
                                },
                                ordering => Some(ordering),
                            }
                        },
                        DataType::Interval => Some(self.value.as_interval().value.cmp(&other.value.as_interval().value)),
                        DataType::String => {
                            // 比较字符串内容
                            let a_str = core::str::from_utf8(self.value.as_string()).unwrap_or("");
                            let b_str = core::str::from_utf8(other.value.as_string()).unwrap_or("");
                            Some(a_str.trim_end_matches(char::from(0)).cmp(b_str.trim_end_matches(char::from(0))))
                        },
                    }
                }
            },
            ordering => Some(ordering),
        }
    }
}

/// 手动实现Ord trait，用于在BTreeMap中用作键
impl Ord for TypedValue {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(core::cmp::Ordering::Equal)
    }
}

/// 手动实现Debug trait，因为Rust不支持为union类型自动派生Debug
impl fmt::Debug for TypedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        {
            match self.value_type {
                DataType::UInt8 => write!(f, "TypedValue(UInt8, {})", self.value.as_u8()),
                DataType::UInt16 => write!(f, "TypedValue(UInt16, {})", self.value.as_u16()),
                DataType::UInt32 => write!(f, "TypedValue(UInt32, {})", self.value.as_u32()),
                DataType::UInt64 => write!(f, "TypedValue(UInt64, {})", self.value.as_u64()),
                DataType::Int8 => write!(f, "TypedValue(Int8, {})", self.value.as_i8()),
                DataType::Int16 => write!(f, "TypedValue(Int16, {})", self.value.as_i16()),
                DataType::Int32 => write!(f, "TypedValue(Int32, {})", self.value.as_i32()),
                DataType::Int64 => write!(f, "TypedValue(Int64, {})", self.value.as_i64()),
                DataType::Float32 => write!(f, "TypedValue(Float32, {})", self.value.as_float32()),
                DataType::Float64 => write!(f, "TypedValue(Float64, {})", self.value.as_float64()),
                DataType::Bool => write!(f, "TypedValue(Bool, {})", self.value.as_bool()),
                DataType::Timestamp => {
                    write!(f, "TypedValue(Timestamp, value: {}, precision: {})", 
                           self.value.as_time().value, self.value.as_time().precision)
                },
                DataType::TimestampTZ => {
                    write!(f, "TypedValue(TimestampTZ, value: {}, tz_offset: {}s, precision: {})", 
                           self.value.as_time().value, self.value.as_time().tz_offset, self.value.as_time().precision)
                },
                DataType::String => {
                    let s = core::str::from_utf8(self.value.as_string()).unwrap_or("").trim_end_matches(char::from(0));
                    write!(f, "TypedValue(String, \"{}\")", s)
                },
                DataType::Interval => {
                    write!(f, "TypedValue(Interval, value: {}, precision: {})", 
                           self.value.as_interval().value, self.value.as_interval().precision)
                }
            }
        }
    }
}

/// 定长字符串最大长度
pub const MAX_STRING_LEN: usize = 64;

/// 字段定义
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct FieldDef {
    /// 字段名称（编译时固定）
    pub name: &'static str,
    /// 数据类型
    pub data_type: DataType,
    /// 字段大小（字节）
    pub size: usize,
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
        
        if let Some(default) = self.default_value {
            constraints.push_str(" DEFAULT ");
            {
                match self.data_type {
                    DataType::String => {
                        let s = core::str::from_utf8(default.as_string()).unwrap_or("").trim_end_matches(char::from(0));
                        constraints.push_str(&alloc::format!("'{}'", s));
                    },
                    DataType::Bool => {
                        let b = default.as_bool();
                        constraints.push_str(if b { "TRUE" } else { "FALSE" });
                    },
                    DataType::UInt8 => constraints.push_str(&default.as_u8().to_string()),
                    DataType::UInt16 => constraints.push_str(&default.as_u16().to_string()),
                    DataType::UInt32 => constraints.push_str(&default.as_u32().to_string()),
                    DataType::UInt64 => constraints.push_str(&default.as_u64().to_string()),
                    DataType::Int8 => constraints.push_str(&default.as_i8().to_string()),
                    DataType::Int16 => constraints.push_str(&default.as_i16().to_string()),
                    DataType::Int32 => constraints.push_str(&default.as_i32().to_string()),
                    DataType::Int64 => constraints.push_str(&default.as_i64().to_string()),
                    DataType::Float32 => constraints.push_str(&default.as_float32().to_string()),
                    DataType::Float64 => constraints.push_str(&default.as_float64().to_string()),
                    DataType::Timestamp => constraints.push_str(&default.as_timestamp().to_string()),
                    DataType::TimestampTZ => constraints.push_str(&default.as_timestamp().to_string()),
                    DataType::Interval => constraints.push_str(&default.as_interval().value.to_string()),
                }
            }
        }
        
        constraints
    }
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

/// 为IndexType实现From<u8> trait，允许从u8转换为IndexType
impl From<u8> for IndexType {
    fn from(value: u8) -> Self {
        match value {
            0 => IndexType::Hash,
            1 => IndexType::SortedArray,
            2 => IndexType::BTree,
            3 => IndexType::TTree,
            _ => IndexType::SortedArray, // 默认使用SortedArray
        }
    }
}

/// 表定义
#[derive(Copy, Clone, Debug)]
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
            RemDbError::InternalError => write!(f, "Internal error"),
        }
    }
}

/// 结果类型
pub type Result<T> = core::result::Result<T, RemDbError>;
