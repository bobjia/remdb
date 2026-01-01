use core::mem::size_of;
use crate::types::TableDef;

/// 默认内存分配器实现
pub struct DefaultMemoryAllocator;

impl MemoryAllocator for DefaultMemoryAllocator {
    fn allocate(&self, _size: usize) -> Option<core::ptr::NonNull<u8>> {
        // 默认实现，返回None表示分配失败
        None
    }
    
    fn deallocate(&self, _ptr: core::ptr::NonNull<u8>, _size: usize) {
        // 默认实现，不做任何操作
    }
}

/// 内存分配器接口
pub trait MemoryAllocator: Sync {
    /// 分配内存
    fn allocate(&self, size: usize) -> Option<core::ptr::NonNull<u8>>;
    
    /// 释放内存
    fn deallocate(&self, ptr: core::ptr::NonNull<u8>, size: usize);
}

/// 日志模式
#[derive(Copy, Clone, PartialEq)]
pub enum LogMode {
    /// 同步模式：事务提交时立即写入日志
    Sync,
    /// 异步模式：日志先写入缓冲区，后台批量写入
    Async,
}

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
    /// 单表的最大记录数（用于动态创建表）
    pub default_max_records: usize,
    /// 内存分配器
    pub memory_allocator: &'static dyn MemoryAllocator,
    /// 日志模式（同步/异步）
    pub log_mode: LogMode,
    /// 检查点间隔（毫秒，默认60秒）
    pub checkpoint_interval_ms: u64,
    /// 日志文件大小限制（字节，默认16MB）
    pub log_file_size_limit: usize,
    /// 日志预分配大小（字节）
    pub log_prealloc_size: usize,
    /// 日志分段大小（字节，默认16MB）
    pub log_segment_size: usize,
    /// 保留的检查点数量
    pub retained_checkpoints: usize,
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
    
    // 检查默认最大记录数
    if config.default_max_records > 500000 {
        return false;
    }
    
    // 检查WAL和检查点配置
    if config.checkpoint_interval_ms > 3600000 { // 最大1小时
        return false;
    }
    
    if config.log_file_size_limit < 1024 * 1024 { // 最小1MB
        return false;
    }
    
    if config.log_prealloc_size > config.log_file_size_limit {
        return false;
    }
    
    if config.log_segment_size < 1024 * 1024 { // 最小1MB
        return false;
    }
    
    if config.retained_checkpoints > 10 {
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
        match table.secondary_index_type {
            // 有序数组索引
            crate::types::IndexType::SortedArray => {
                let primary_key_field = &table.fields[table.primary_key];
                table.max_records * (primary_key_field.size + size_of::<u16>())
            },
            // B-Tree索引
            crate::types::IndexType::BTree => {
                // B-Tree节点大小
                const BTREE_NODE_SIZE: usize = 1 + 1 + (64 * 4) + ((size_of::<usize>() * 5) / 8);
                // 假设每个节点平均使用50%的空间，每个节点平均2个键
                let max_nodes = table.max_records / 2;
                max_nodes * BTREE_NODE_SIZE
            },
            // T-Tree索引
            crate::types::IndexType::TTree => {
                // T-Tree节点大小
                const TTREE_NODE_SIZE: usize = 1 + (64 * 3) + (size_of::<usize>() * 3);
                // 假设每个节点平均使用50%的空间，每个节点平均2个键
                let max_nodes = table.max_records / 2;
                max_nodes * TTREE_NODE_SIZE
            },
            // 其他索引类型（默认）
            _ => {
                let primary_key_field = &table.fields[table.primary_key];
                table.max_records * (primary_key_field.size + size_of::<u16>())
            }
        }
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
