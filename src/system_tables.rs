use crate::platform::{memcpy, memset};
use crate::types::{RemDbError, Result, Value};

// 系统表名称
pub const SYSTEM_CONFIG_TABLE: &str = "__remdb_system_config";
pub const SYSTEM_ROLES_TABLE: &str = "__remdb_system_roles";
pub const SYSTEM_ROLE_PERMISSIONS_TABLE: &str = "__remdb_system_role_permissions";
pub const SYSTEM_USERS_TABLE: &str = "__remdb_system_users";
pub const SYSTEM_USER_ROLES_TABLE: &str = "__remdb_system_user_roles";

// 配置项缓存
static mut CONFIG_CACHE: Option<ConfigCache> = None;

/// 配置项缓存结构
#[derive(Clone)]
pub struct ConfigCache {
    /// 全局向量压缩开关
    pub vector_compression_enabled: bool,
    /// 向量压缩方案（0=不压缩, 1=float16, 2=zstd）
    pub vector_compression_scheme: u8,
    /// 压缩级别（1-9）
    pub vector_compression_level: u8,
    /// 查询内存限制（MB）
    pub max_query_memory_mb: u32,
    /// 查询超时时间（毫秒）
    pub query_timeout_ms: u32,
}

/// 压缩方案常量
pub const COMPRESSION_NONE: u8 = 0;
pub const COMPRESSION_FLOAT16: u8 = 1;
pub const COMPRESSION_ZSTD: u8 = 2;

/// 初始化系统表
pub unsafe fn init_system_tables(db: &mut crate::RemDb) -> Result<()> {
    // 检查系统表是否已存在
    let system_config_table_exists = db.tables.iter().any(|table_opt| {
        table_opt.as_ref().map(|table| table.def.name == SYSTEM_CONFIG_TABLE).unwrap_or(false)
    });
    
    if !system_config_table_exists {
        // 创建系统配置表
        create_system_config_table(db)?;
        // 插入默认配置
        insert_default_configs(db)?;
    }
    
    // 检查RBAC系统表是否存在
    let roles_table_exists = db.tables.iter().any(|table_opt| {
        table_opt.as_ref().map(|table| table.def.name == SYSTEM_ROLES_TABLE).unwrap_or(false)
    });
    
    if !roles_table_exists {
        // 创建RBAC系统表
        create_system_roles_table(db)?;
        create_system_role_permissions_table(db)?;
        create_system_users_table(db)?;
        create_system_user_roles_table(db)?;
    }
    
    // 初始化配置缓存
    load_config_cache(db)?;
    
    Ok(())
}

/// 创建系统配置表
unsafe fn create_system_config_table(db: &mut crate::RemDb) -> Result<()> {
    // 直接创建TableDef，使用较小的max_records值
    let now = crate::platform::get_timestamp_us();
    
    // 定义字段
    let _offset = 0;
    let record_size = 64 + 256 + 128 + 8 + 8; // 计算系统表记录大小
    
    // 创建字段定义
    let fields = vec![
        crate::types::FieldDef {
            name: "config_key".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "config_value".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 256,
            string_length: Some(256),
            offset: 64,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "description".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 128,
            string_length: Some(128),
            offset: 64 + 256,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "updated_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256 + 128,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "created_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256 + 128 + 8,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];
    
    // 创建表定义，使用较小的max_records值
    let table_def = crate::types::TableDef {
        id: (db.tables.len() + 1) as u8, // 系统表ID从1开始
        name: SYSTEM_CONFIG_TABLE.to_string(),
        fields,
        primary_key: vec![0], // 主键是config_key字段
        secondary_index: None,
        secondary_index_type: crate::types::IndexType::SortedArray,
        record_size,
        max_records: 512, // 增加系统表记录限制
        version: 1,
        created_at: now,
        updated_at: now,
    };
    
    // 创建MemoryTable
    let table = crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def))?;
    
    // 添加到数据库
    db.tables.push(Some(table));
    db.primary_indices.push(None);
    db.secondary_indices.push(None);
    
    Ok(())
}

/// 创建系统角色表
unsafe fn create_system_roles_table(db: &mut crate::RemDb) -> Result<()> {
    // 直接创建TableDef，使用较小的max_records值
    let now = crate::platform::get_timestamp_us();
    
    // 定义字段
    let record_size = 64 + 256 + 8 + 8; // 计算系统表记录大小
    
    // 创建字段定义
    let fields = vec![
        crate::types::FieldDef {
            name: "role_name".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "description".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 256,
            string_length: Some(256),
            offset: 64,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "created_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "updated_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256 + 8,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];
    
    // 创建表定义，使用较小的max_records值
    let table_def = crate::types::TableDef {
        id: (db.tables.len() + 1) as u8, // 系统表ID从1开始
        name: SYSTEM_ROLES_TABLE.to_string(),
        fields,
        primary_key: vec![0], // 主键是role_name字段
        secondary_index: None,
        secondary_index_type: crate::types::IndexType::SortedArray,
        record_size,
        max_records: 512, // 增加系统表记录限制
        version: 1,
        created_at: now,
        updated_at: now,
    };
    
    // 创建MemoryTable
    let table = crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def))?;
    
    // 添加到数据库
    db.tables.push(Some(table));
    db.primary_indices.push(None);
    db.secondary_indices.push(None);
    
    Ok(())
}

/// 创建系统角色权限表
unsafe fn create_system_role_permissions_table(db: &mut crate::RemDb) -> Result<()> {
    // 直接创建TableDef，使用较小的max_records值
    let now = crate::platform::get_timestamp_us();
    
    // 定义字段
    let record_size = 64 + 64 + 256 + 8; // 计算系统表记录大小
    
    // 创建字段定义
    let fields = vec![
        crate::types::FieldDef {
            name: "role_name".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "permission".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 64,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "table_name".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 256,
            string_length: Some(256),
            offset: 64 + 64,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "created_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 64 + 256,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];
    
    // 创建表定义，使用较小的max_records值
    let table_def = crate::types::TableDef {
        id: (db.tables.len() + 1) as u8, // 系统表ID从1开始
        name: SYSTEM_ROLE_PERMISSIONS_TABLE.to_string(),
        fields,
        primary_key: vec![0, 1], // 联合主键：role_name + permission
        secondary_index: None,
        secondary_index_type: crate::types::IndexType::SortedArray,
        record_size,
        max_records: 512, // 增加系统表记录限制
        version: 1,
        created_at: now,
        updated_at: now,
    };
    
    // 创建MemoryTable
    let table = crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def))?;
    
    // 添加到数据库
    db.tables.push(Some(table));
    db.primary_indices.push(None);
    db.secondary_indices.push(None);
    
    Ok(())
}

/// 创建系统用户表
unsafe fn create_system_users_table(db: &mut crate::RemDb) -> Result<()> {
    // 直接创建TableDef，使用较小的max_records值
    let now = crate::platform::get_timestamp_us();
    
    // 定义字段
    let record_size = 64 + 256 + 8 + 8; // 计算系统表记录大小
    
    // 创建字段定义
    let fields = vec![
        crate::types::FieldDef {
            name: "username".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "description".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 256,
            string_length: Some(256),
            offset: 64,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "created_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "updated_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 256 + 8,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];
    
    // 创建表定义，使用较小的max_records值
    let table_def = crate::types::TableDef {
        id: (db.tables.len() + 1) as u8, // 系统表ID从1开始
        name: SYSTEM_USERS_TABLE.to_string(),
        fields,
        primary_key: vec![0], // 主键是username字段
        secondary_index: None,
        secondary_index_type: crate::types::IndexType::SortedArray,
        record_size,
        max_records: 512, // 增加系统表记录限制
        version: 1,
        created_at: now,
        updated_at: now,
    };
    
    // 创建MemoryTable
    let table = crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def))?;
    
    // 添加到数据库
    db.tables.push(Some(table));
    db.primary_indices.push(None);
    db.secondary_indices.push(None);
    
    Ok(())
}

/// 创建系统用户角色表
unsafe fn create_system_user_roles_table(db: &mut crate::RemDb) -> Result<()> {
    // 直接创建TableDef，使用较小的max_records值
    let now = crate::platform::get_timestamp_us();
    
    // 定义字段
    let record_size = 64 + 64 + 8; // 计算系统表记录大小
    
    // 创建字段定义
    let fields = vec![
        crate::types::FieldDef {
            name: "username".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "role_name".to_string(),
            data_type: crate::types::DataType::VarChar,
            size: 64,
            string_length: Some(64),
            offset: 64,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        crate::types::FieldDef {
            name: "created_at".to_string(),
            data_type: crate::types::DataType::Timestamp,
            size: 8,
            string_length: None,
            offset: 64 + 64,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ];
    
    // 创建表定义，使用较小的max_records值
    let table_def = crate::types::TableDef {
        id: (db.tables.len() + 1) as u8, // 系统表ID从1开始
        name: SYSTEM_USER_ROLES_TABLE.to_string(),
        fields,
        primary_key: vec![0, 1], // 联合主键：username + role_name
        secondary_index: None,
        secondary_index_type: crate::types::IndexType::SortedArray,
        record_size,
        max_records: 512, // 增加系统表记录限制
        version: 1,
        created_at: now,
        updated_at: now,
    };
    
    // 创建MemoryTable
    let table = crate::table::MemoryTable::new(alloc::sync::Arc::new(table_def))?;
    
    // 添加到数据库
    db.tables.push(Some(table));
    db.primary_indices.push(None);
    db.secondary_indices.push(None);
    
    Ok(())
}

/// 插入默认配置
unsafe fn insert_default_configs(db: &mut crate::RemDb) -> Result<()> {
    // 获取系统表索引
    let table_id = db.tables.iter()
        .position(|table_opt| table_opt.as_ref().map(|table| table.def.name == SYSTEM_CONFIG_TABLE).unwrap_or(false))
        .ok_or(RemDbError::TableNotFound)?;
    
    let table = db.get_table_mut(table_id)?;
    
    // 获取当前时间戳
    let now = crate::platform::get_timestamp_us();
    let _timestamp_value = Value {
        time: crate::types::db_timestamp {
            value: now as i64,
            tz_offset: 0,
            precision: 6, // 微秒级
            flags: 0,
        }
    };
    
    // 默认配置项
    let default_configs = [
        ("vector_compression_enabled", "false", "全局向量压缩开关"),
        ("vector_compression_scheme", "none", "向量压缩方案：none=不压缩, float16=float16, zstd=ZSTD"),
        ("vector_compression_level", "3", "压缩级别（1-9）"),
        ("max_query_memory_mb", "512", "查询内存限制（MB）"),
        ("query_timeout_ms", "30000", "查询超时时间（毫秒）"),
    ];
    
    for (key, value, desc) in default_configs {
        // 构建记录数据
        let mut record_data = [0u8; 64 + 256 + 128 + 8 + 8]; // 总字段大小
        let mut offset = 0;
        
        // 写入config_key
        memset(record_data.as_mut_ptr().add(offset), 0, 64);
        let key_bytes = key.as_bytes();
        memcpy(record_data.as_mut_ptr().add(offset), key_bytes.as_ptr(), key_bytes.len());
        offset += 64;
        
        // 写入config_value
        memset(record_data.as_mut_ptr().add(offset), 0, 256);
        let value_bytes = value.as_bytes();
        memcpy(record_data.as_mut_ptr().add(offset), value_bytes.as_ptr(), value_bytes.len());
        offset += 256;
        
        // 写入description
        memset(record_data.as_mut_ptr().add(offset), 0, 128);
        let desc_bytes = desc.as_bytes();
        memcpy(record_data.as_mut_ptr().add(offset), desc_bytes.as_ptr(), desc_bytes.len());
        offset += 128;
        
        // 写入updated_at
        memcpy(record_data.as_mut_ptr().add(offset), &now as *const u64 as *const u8, 8);
        offset += 8;
        
        // 写入created_at
        memcpy(record_data.as_mut_ptr().add(offset), &now as *const u64 as *const u8, 8);
        
        // 插入记录
        table.insert(record_data.as_ptr())?;
    }
    
    Ok(())
}

/// 加载配置缓存
pub unsafe fn load_config_cache(db: &crate::RemDb) -> Result<()> {
    // 查找系统表
    let table_id = db.tables.iter()
        .position(|table_opt| table_opt.as_ref().map(|table| table.def.name == SYSTEM_CONFIG_TABLE).unwrap_or(false))
        .ok_or(RemDbError::TableNotFound)?;
    
    let table = db.get_table(table_id)?;
    
    // 初始化默认配置
    let mut enabled = false;
    let mut scheme = COMPRESSION_NONE;
    let mut level = 3u8;
    let mut max_query_memory_mb = 512u32;
    let mut query_timeout_ms = 30000u32;
    
    // 扫描系统表获取配置
    let mut cursor = table.scan_ref();
    while let Some(record) = cursor.next() {
        // 获取config_key
        let config_key = record.get_str(0).unwrap_or("");
        
        match config_key {
            "vector_compression_enabled" => {
                let value = record.get_str(1).unwrap_or("false");
                enabled = value == "true";
            },
            "vector_compression_scheme" => {
                let value = record.get_str(1).unwrap_or("none");
                scheme = match value {
                    "none" => COMPRESSION_NONE,
                    "float16" => COMPRESSION_FLOAT16,
                    "zstd" => COMPRESSION_ZSTD,
                    _ => COMPRESSION_NONE,
                };
            },
            "vector_compression_level" => {
                let value = record.get_str(1).unwrap_or("3");
                level = value.parse().unwrap_or(3);
            },
            "max_query_memory_mb" => {
                let value = record.get_str(1).unwrap_or("512");
                max_query_memory_mb = value.parse().unwrap_or(512);
            },
            "query_timeout_ms" => {
                let value = record.get_str(1).unwrap_or("30000");
                query_timeout_ms = value.parse().unwrap_or(30000);
            },
            _ => {},
        }
    }
    
    // 更新配置缓存
    CONFIG_CACHE = Some(ConfigCache {
        vector_compression_enabled: enabled,
        vector_compression_scheme: scheme,
        vector_compression_level: level,
        max_query_memory_mb,
        query_timeout_ms,
    });
    
    Ok(())
}

/// 获取当前向量压缩配置
pub fn get_vector_compression_config() -> ConfigCache {
    unsafe {
        // 如果缓存已初始化，返回缓存的配置，否则返回默认配置
        CONFIG_CACHE.as_ref().cloned().unwrap_or_else(|| {
            // 返回默认配置的副本，而不是引用
            ConfigCache {
                vector_compression_enabled: false,
                vector_compression_scheme: COMPRESSION_NONE,
                vector_compression_level: 3,
                max_query_memory_mb: 512,
                query_timeout_ms: 30000,
            }
        })
    }
}

/// 获取查询资源配置
pub fn get_query_resource_config() -> (u32, u32) {
    unsafe {
        // 如果缓存已初始化，返回缓存的配置，否则返回默认配置
        let config = CONFIG_CACHE.as_ref().cloned().unwrap_or_else(|| {
            // 返回默认配置的副本，而不是引用
            ConfigCache {
                vector_compression_enabled: false,
                vector_compression_scheme: COMPRESSION_NONE,
                vector_compression_level: 3,
                max_query_memory_mb: 512,
                query_timeout_ms: 30000,
            }
        });
        
        (config.max_query_memory_mb, config.query_timeout_ms)
    }
}

/// 刷新配置缓存
pub unsafe fn refresh_config_cache() -> Result<()> {
    if let Some(db) = crate::get_global_db() {
        load_config_cache(db)?;
    }
    Ok(())
}

/// 检查系统表是否为系统表
pub fn is_system_table(table_name: &str) -> bool {
    table_name.starts_with("__remdb_system")
}

/// 获取向量字段大小（考虑压缩）
pub fn get_vector_field_size(dimension: u16) -> usize {
    let config = get_vector_compression_config();
    
    match config.vector_compression_scheme {
        COMPRESSION_NONE => dimension as usize * 4, // float32: 4字节/维度
        COMPRESSION_FLOAT16 => dimension as usize * 2, // float16: 2字节/维度
        COMPRESSION_ZSTD => dimension as usize * 4 + 4, // ZSTD: 原始大小 + 4字节压缩大小
        _ => dimension as usize * 4, // 默认不压缩
    }
}