use core::mem::size_of;
use crate::types::TableDef;

/// 数据库全局配置
pub struct DbConfig {
    /// 表定义列表
    pub tables: &'static [TableDef],
    /// 总内存大小
    pub total_memory: usize,
    /// 支持低功耗模式
    pub low_power_mode_supported: bool,
    /// 低功耗模式下的最大记录数（可选）
    pub low_power_max_records: Option<usize>,
}



/// 编译时配置检查
pub const fn validate_config(config: &DbConfig) -> bool {
    // 检查表数量
    if config.tables.len() > 32 {
        return false;
    }
    
    // 检查低功耗模式配置
    if let Some(low_power_max) = config.low_power_max_records {
        if low_power_max > 100000 {
            return false;
        }
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
