#[cfg(feature = "ha")]
pub use crate::ha::HAConfig;
pub use crate::time_series::TimeSeriesConfig;
use crate::types::TableDef;
use core::mem::size_of;

/// 默认内存分配器实现
pub struct DefaultMemoryAllocator;

impl MemoryAllocator for DefaultMemoryAllocator {
    fn allocate(&self, size: usize) -> Option<core::ptr::NonNull<u8>> {
        // 实际分配内存
        #[cfg(feature = "std")]
        {
            let mut vec = vec![0u8; size];
            let ptr = vec.as_mut_ptr();
            // 释放vec对内存的所有权，但不释放内存本身
            std::mem::forget(vec);
            Some(unsafe { core::ptr::NonNull::new_unchecked(ptr) })
        }
        #[cfg(not(feature = "std"))]
        {
            // 非std环境下返回None
            None
        }
    }

    fn deallocate(&self, ptr: core::ptr::NonNull<u8>, size: usize) {
        // 释放内存
        #[cfg(feature = "std")]
        {
            unsafe {
                let slice = core::slice::from_raw_parts_mut(ptr.as_ptr(), size);
                let vec = Vec::from_raw_parts(slice.as_mut_ptr(), 0, size);
                drop(vec);
            }
        }
        // 非std环境下不做任何操作
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

/// WAL日志配置
pub struct WALConfig {
    /// 日志文件路径
    pub log_path: &'static str,
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

/// 数据库全局配置
pub struct DbConfig {
    /// 表定义列表
    pub tables: Vec<TableDef>,
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
    /// WAL日志配置
    pub wal_config: WALConfig,
    /// 时序数据默认配置
    pub time_series_defaults: TimeSeriesConfig,

    /// PubSub配置
    #[cfg(feature = "pubsub")]
    pub pubsub_config: Option<crate::pubsub::PubSubConfig>,

    /// HA配置
    #[cfg(feature = "ha")]
    pub ha_config: Option<HAConfig>,
}

/// 编译时配置检查
pub fn validate_config(config: &DbConfig) -> bool {
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
    if config.wal_config.checkpoint_interval_ms > 3600000 {
        // 最大1小时
        return false;
    }

    if config.wal_config.log_file_size_limit < 1024 * 1024 {
        // 最小1MB
        return false;
    }

    if config.wal_config.log_prealloc_size > config.wal_config.log_file_size_limit {
        return false;
    }

    if config.wal_config.log_segment_size < 1024 * 1024 {
        // 最小1MB
        return false;
    }

    if config.wal_config.retained_checkpoints > 10 {
        return false;
    }

    // 检查HA配置
    #[cfg(feature = "ha")]
    {
        if let Some(ha_config) = &config.ha_config {
            if ha_config.heartbeat_interval_ms < 100 {
                // 最小100ms
                return false;
            }

            if ha_config.heartbeat_interval_ms > 60000 {
                // 最大60秒
                return false;
            }

            if ha_config.failure_detection_ms < ha_config.heartbeat_interval_ms {
                // 故障检测时间必须大于等于心跳间隔
                return false;
            }

            if ha_config.failure_detection_ms > 300000 {
                // 最大5分钟
                return false;
            }

            if ha_config.sync_timeout_ms < 100 {
                // 最小100ms
                return false;
            }

            if ha_config.sync_timeout_ms > 10000 {
                // 最大10秒
                return false;
            }
        }
    }

    // 检查每个表
    for table in &config.tables {
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
    }

    true
}

/// 计算表的内存占用
pub fn table_memory_usage(table: &TableDef) -> usize {
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
            }
            // B-Tree索引
            crate::types::IndexType::BTree => {
                // B-Tree节点大小
                const BTREE_NODE_SIZE: usize = 1 + 1 + (64 * 4) + ((size_of::<usize>() * 5) / 8);
                // 假设每个节点平均使用50%的空间，每个节点平均2个键
                let max_nodes = table.max_records / 2;
                max_nodes * BTREE_NODE_SIZE
            }
            // T-Tree索引
            crate::types::IndexType::TTree => {
                // T-Tree节点大小
                const TTREE_NODE_SIZE: usize = 1 + (64 * 3) + (size_of::<usize>() * 3);
                // 假设每个节点平均使用50%的空间，每个节点平均2个键
                let max_nodes = table.max_records / 2;
                max_nodes * TTREE_NODE_SIZE
            }
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
pub fn total_memory_usage(config: &DbConfig) -> usize {
    let mut total = 0;
    let mut i = 0;
    while i < config.tables.len() {
        total += table_memory_usage(&config.tables[i]);
        i += 1;
    }
    total
}
