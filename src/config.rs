use core::mem::size_of;
use crate::types::TableDef;

/// 数据库全局配置
pub struct DbConfig {
    /// 表定义列表
    pub tables: &'static [TableDef],
    /// 总内存大小
    pub total_memory: usize,
}

/// 编译时表配置宏
#[macro_export]
macro_rules! table {
    // 有辅助索引，有逗号
    (
        $table_name:ident, 
        $max_records:expr, 
        primary_key: $primary_key:ident, 
        secondary_index: $secondary_index:ident, 
        fields: {
            $($field_defs:tt)*
        }
    ) => {
        // 直接生成静态表定义
        static $table_name: $crate::types::TableDef = $crate::types::TableDef {
            id: 0,
            name: stringify!($table_name),
            fields: &$crate::table_fields!($($field_defs)*),
            primary_key: 0,
            secondary_index: None,
            record_size: 0,
            max_records: $max_records,
        };
    };
    
    // 没有辅助索引，有逗号
    (
        $table_name:ident, 
        $max_records:expr, 
        primary_key: $primary_key:ident, 
        fields: {
            $($field_defs:tt)*
        }
    ) => {
        // 直接生成静态表定义
        static $table_name: $crate::types::TableDef = $crate::types::TableDef {
            id: 0,
            name: stringify!($table_name),
            fields: &$crate::table_fields!($($field_defs)*),
            primary_key: 0,
            secondary_index: None,
            record_size: 0,
            max_records: $max_records,
        };
    };
}

/// 辅助宏：生成单个字段定义
#[macro_export]
macro_rules! table_field {
    // 字符串类型字段
    ($field_name:ident: str($field_len:expr)) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::String,
            size: $field_len,
            offset: 0,
        }
    };
    // i8类型字段
    ($field_name:ident: i8) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Int8,
            size: 1,
            offset: 0,
        }
    };
    // i16类型字段
    ($field_name:ident: i16) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Int16,
            size: 2,
            offset: 0,
        }
    };
    // i32类型字段
    ($field_name:ident: i32) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Int32,
            size: 4,
            offset: 0,
        }
    };
    // i64类型字段
    ($field_name:ident: i64) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Int64,
            size: 8,
            offset: 0,
        }
    };
    // f32类型字段
    ($field_name:ident: f32) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Float32,
            size: 4,
            offset: 0,
        }
    };
    // f64类型字段
    ($field_name:ident: f64) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Float64,
            size: 8,
            offset: 0,
        }
    };
    // bool类型字段
    ($field_name:ident: bool) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Bool,
            size: 1,
            offset: 0,
        }
    };
    // u64类型字段
    ($field_name:ident: u64) => {
        $crate::types::FieldDef {
            name: stringify!($field_name),
            data_type: $crate::types::DataType::Timestamp,
            size: 8,
            offset: 0,
        }
    };
}

/// 辅助宏：生成字段定义列表
#[macro_export]
macro_rules! table_fields {
    // 处理单个字段
    ($($field_name:ident: $field_type:tt $(($field_len:expr))?);* $(;)?) => {
        [
            $(
                $crate::table_field!($field_name: $field_type $(($field_len))?)
            ),*
        ]
    };
    
    // 处理单个字段（带逗号分隔）
    ($($field_name:ident: $field_type:tt $(($field_len:expr))?),* $(,)?) => {
        [
            $(
                $crate::table_field!($field_name: $field_type $(($field_len))?)
            ),*
        ]
    };
}

/// 编译时数据库配置宏
#[macro_export]
macro_rules! database {
    (
        $db_name:ident,
        tables: [
            $($table:ident),*
        ]
    ) => {
        // 生成数据库配置静态变量
        static $db_name: $crate::config::DbConfig = $crate::config::DbConfig {
            tables: &[
                $($table),*
            ],
            total_memory: 0,
        };
    };
}

/// 编译时配置检查
pub const fn validate_config(config: &DbConfig) -> bool {
    // 检查表数量
    if config.tables.len() > 32 {
        return false;
    }
    
    // 检查每个表（使用常量兼容的方式）
    let mut i = 0;
    while i < config.tables.len() {
        let table = &config.tables[i];
        
        // 检查记录大小
        if table.record_size > 512 {
            return false;
        }
        
        // 检查最大记录数
        if table.max_records > 500000 {
            return false;
        }
        
        // 检查主键存在
        if table.primary_key >= table.fields.len() {
            return false;
        }
        
        // 检查辅助索引（如果有）
        let has_secondary = table.secondary_index.is_some();
        if has_secondary {
            let secondary_index = table.secondary_index.unwrap();
            if secondary_index >= table.fields.len() {
                return false;
            }
        }
        
        i += 1;
    }
    
    true
}

/// 计算表的内存占用
pub const fn table_memory_usage(table: &TableDef) -> usize {
    // 记录内存
    let record_memory = table.record_size * table.max_records;
    
    // 索引内存
    let index_memory = table.max_records * size_of::<u32>(); // 主键哈希表
    
    // 辅助索引内存（如果有）
    let secondary_index_memory = if table.secondary_index.is_some() {
        let primary_key_field = &table.fields[table.primary_key];
        table.max_records * (primary_key_field.size + size_of::<u16>())
    } else {
        0
    };
    
    record_memory + index_memory + secondary_index_memory
}

/// 计算数据库总内存占用
pub const fn total_memory_usage(config: &DbConfig) -> usize {
    let mut total = 0;
    let mut i = 0;
    while i < config.tables.len() {
        total += table_memory_usage(&config.tables[i]);
        i += 1;
    }
    total
}
