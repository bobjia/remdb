#[cfg(feature = "ha")]
pub use crate::ha::HAConfig;
pub use crate::time_series::TimeSeriesConfig;
use crate::types::TableDef;
use core::mem::size_of;

/// Model Worker 配置
#[cfg(feature = "model-runtime")]
#[derive(Clone, Debug)]
pub struct ModelWorkerConfig {
    /// 是否启用 Model Worker
    pub enabled: bool,
    /// 分配给 Worker 的 CPU 核心数
    pub cpu_cores: usize,
    /// 内存限制（MB）
    pub memory_limit_mb: usize,
    /// 最大模型数量
    pub max_models: usize,
    /// 请求超时时间（毫秒）
    pub request_timeout_ms: u64,
    /// Worker 崩溃时是否自动重启
    pub restart_on_failure: bool,
    /// 最大重启尝试次数
    pub max_restart_attempts: u32,
}

#[cfg(feature = "model-runtime")]
impl Default for ModelWorkerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cpu_cores: 2,
            memory_limit_mb: 2048,
            max_models: 10,
            request_timeout_ms: 5000,
            restart_on_failure: true,
            max_restart_attempts: 3,
        }
    }
}

#[cfg(feature = "model-runtime")]
impl ModelWorkerConfig {
    pub fn validate(&self) -> bool {
        if self.cpu_cores == 0 || self.cpu_cores > 64 {
            return false;
        }
        if self.memory_limit_mb < 256 || self.memory_limit_mb > 65536 {
            return false;
        }
        if self.max_models == 0 || self.max_models > 100 {
            return false;
        }
        if self.request_timeout_ms < 100 || self.request_timeout_ms > 60000 {
            return false;
        }
        if self.max_restart_attempts > 10 {
            return false;
        }
        true
    }
}

/// 默认内存分配器实现
pub struct DefaultMemoryAllocator;

impl MemoryAllocator for DefaultMemoryAllocator {
    fn allocate(&self, size: usize) -> Option<core::ptr::NonNull<u8>> {
        // 实际分配内存
        #[cfg(feature = "std")]
        {
            // 使用with_capacity + resize确保capacity == size
            let mut vec = Vec::with_capacity(size);
            vec.resize(size, 0);
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
                // When recreating Vec for deallocation, len should match original allocation size
                // because we initialized all bytes with vec![0u8; size]
                let vec = Vec::from_raw_parts(ptr.as_ptr(), size, size);
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

/// WAL压缩类型
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WALCompressionType {
    /// 不压缩
    None,
    /// LZ4压缩
    LZ4,
    /// ZSTD压缩
    ZSTD,
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
    /// 恢复时最大连续无效记录数，达到此值后停止恢复
    pub max_consecutive_invalid: u32,
    /// 恢复时跳过预分配空间的阈值（连续无效记录数）
    pub skip_threshold: u32,
    /// 恢复时跳过的块大小（字节）
    pub skip_block_size: usize,
    /// 恢复时最大跳过尝试次数
    pub max_skip_attempts: u32,
    /// WAL压缩类型
    pub compression_type: WALCompressionType,
    /// 压缩级别（1-9）
    pub compression_level: u8,
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

    /// Model Worker配置
    #[cfg(feature = "model-runtime")]
    pub model_worker_config: ModelWorkerConfig,
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

    if config.wal_config.compression_level < 1 || config.wal_config.compression_level > 9 {
        return false;
    }

    #[cfg(not(feature = "wal-compression-lz4"))]
    {
        if matches!(config.wal_config.compression_type, WALCompressionType::LZ4) {
            return false;
        }
    }

    #[cfg(not(feature = "wal-compression-zstd"))]
    {
        if matches!(config.wal_config.compression_type, WALCompressionType::ZSTD) {
            return false;
        }
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

    // 检查Model Worker配置
    #[cfg(feature = "model-runtime")]
    {
        if !config.model_worker_config.validate() {
            return false;
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
        for &pk_index in &table.primary_key {
            if pk_index >= table.fields.len() {
                return false;
            }
        }

        // 检查辅助索引（如果有）
        if let Some(secondary_index) = &table.secondary_index {
            for &index in secondary_index {
                if index >= table.fields.len() {
                    return false;
                }
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
                // 对于复合主键，使用第一个主键字段的大小
                let primary_key_field = &table.fields[table.primary_key[0]];
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
                // 对于复合主键，使用第一个主键字段的大小
                let primary_key_field = &table.fields[table.primary_key[0]];
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
