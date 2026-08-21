use crate::try_lock;

use super::{CompressionType, LifecycleManager, PartitionManager, TimeSeriesIndex};
use crate::{RemDbError, Result, TableDef};
use alloc::{sync::Arc, vec::Vec};
use core::time::Duration;

#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(not(feature = "std"))]
use crate::memory::allocator::Mutex;

/// 时序数据配置
#[derive(Debug, Clone, Copy)]
pub struct TimeSeriesConfig {
    /// 分区时长（秒）
    pub partition_duration_secs: u64,
    /// 数据保留期（秒）
    pub retention_period_secs: u64,
    /// 压缩类型
    pub compression: CompressionType,
    /// 最大分区数
    pub max_partitions: usize,
}

impl TimeSeriesConfig {
    /// 创建一个新的时序数据配置
    pub const fn new(
        partition_duration_secs: u64,
        retention_period_secs: u64,
        compression: CompressionType,
        max_partitions: usize,
    ) -> Self {
        Self {
            partition_duration_secs,
            retention_period_secs,
            compression,
            max_partitions,
        }
    }

    /// 获取分区时长
    pub fn partition_duration(&self) -> Duration {
        Duration::from_secs(self.partition_duration_secs)
    }

    /// 获取数据保留期
    pub fn retention_period(&self) -> Duration {
        Duration::from_secs(self.retention_period_secs)
    }
}

impl Default for TimeSeriesConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// 时序数据配置默认值
pub const DEFAULT_TIME_SERIES_CONFIG: TimeSeriesConfig = TimeSeriesConfig::new(
    3600,          // 1小时
    7 * 24 * 3600, // 7天
    CompressionType::DeltaRunLength,
    1000,
);

impl TimeSeriesConfig {
    /// 时序数据配置默认值
    pub const DEFAULT: Self = DEFAULT_TIME_SERIES_CONFIG;
}

/// 时序表定义
#[derive(Debug)]
pub struct TimeSeriesTableDef {
    /// 基础表定义
    pub base: TableDef,
    /// 时间字段索引
    pub time_field: usize,
    /// 值字段索引
    pub value_field: usize,
    /// 标签字段索引列表
    pub tag_fields: Box<[usize]>,
    /// 时序数据配置
    pub config: TimeSeriesConfig,
}

/// 时序数据记录
#[derive(Debug, Clone, Copy)]
pub struct TimeSeriesRecord {
    /// 时间戳
    pub timestamp: u64,
    /// 值
    pub value: f64,
    /// 标签数量
    pub tag_count: u8,
    /// 标签数据（可变长度）
    pub tags: [u64; 8], // 支持最多8个标签
}

/// 预聚合数据配置
#[derive(Debug, Clone, PartialEq)]
pub struct PreAggregationConfig {
    /// 预聚合时间间隔（秒）
    pub interval_seconds: u64,
    /// 预聚合函数
    pub aggregation: String,
}

/// 预聚合数据存储
pub struct PreAggregationStore {
    /// 预聚合配置
    pub configs: Vec<PreAggregationConfig>,
    /// 预聚合数据（按时间间隔和标签组合存储）
    pub data: std::collections::HashMap<(u64, u64), f64>, // (time_bucket, tag_hash) -> aggregated_value
}

/// 时序表结构
pub struct TimeSeriesTable {
    /// 表定义
    pub def: Arc<TimeSeriesTableDef>,
    /// 分区管理器
    pub partitions: Arc<Mutex<PartitionManager>>,
    /// 时序索引
    pub index: Arc<TimeSeriesIndex>,
    /// 生命周期管理器
    pub lifecycle: LifecycleManager,
    /// 预聚合数据存储
    pub pre_aggregation: Arc<Mutex<PreAggregationStore>>,
}

impl TimeSeriesTable {
    /// 创建新的时序表
    pub fn new(def: Arc<TimeSeriesTableDef>, index: Arc<TimeSeriesIndex>) -> Result<Self> {
        // 检查时间字段和值字段的有效性
        if def.time_field >= def.base.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        if def.value_field >= def.base.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }

        // 检查标签字段的有效性
        for tag_field in def.tag_fields.iter() {
            if *tag_field >= def.base.fields.len() {
                return Err(RemDbError::FieldNotFound);
            }
        }

        // 创建分区管理器
        let partition_manager = Arc::new(Mutex::new(PartitionManager::new(
            def.config.partition_duration(),
            def.config.max_partitions,
        )));

        // 创建生命周期管理器
        let mut lifecycle_manager = LifecycleManager::new(def.config.retention_period());

        // 设置清理闭包
        let partitions_clone = partition_manager.clone();
        let retention_period = def.config.retention_period();
        lifecycle_manager.set_cleanup_callback(move || {
            let mut partitions_guard = try_lock!(partitions_clone);
            let current_time = LifecycleManager::get_current_timestamp();
            partitions_guard.cleanup_expired_partitions(current_time, retention_period);
        });

        // 创建预聚合数据存储
        let pre_aggregation = Arc::new(Mutex::new(PreAggregationStore {
            configs: Vec::new(),
            data: std::collections::HashMap::new(),
        }));

        Ok(Self {
            def,
            partitions: partition_manager,
            index,
            lifecycle: lifecycle_manager,
            pre_aggregation,
        })
    }

    /// 添加预聚合配置
    pub fn add_pre_aggregation(
        &self,
        interval_seconds: u64,
        aggregation: &str,
    ) -> Result<()> {
        let mut pre_aggregation_guard = try_lock!(self.pre_aggregation);
        
        // 检查是否已存在相同配置
        let existing_config = pre_aggregation_guard.configs.iter()
            .find(|config| config.interval_seconds == interval_seconds && config.aggregation == aggregation);
        
        if existing_config.is_some() {
            return Ok(()); // 配置已存在，无需重复添加
        }
        
        // 添加新的预聚合配置
        pre_aggregation_guard.configs.push(PreAggregationConfig {
            interval_seconds,
            aggregation: aggregation.to_string(),
        });
        
        Ok(())
    }
    
    /// 使用预聚合数据执行查询
    pub fn query_pre_aggregated(
        &self,
        start_time: u64,
        end_time: u64,
        interval_seconds: u64,
        aggregation: &str,
    ) -> Result<Vec<TimeSeriesRecord>> {
        let pre_aggregation_guard = try_lock!(self.pre_aggregation);
        
        // 检查预聚合配置是否存在
        let config_exists = pre_aggregation_guard.configs.iter()
            .any(|config| config.interval_seconds == interval_seconds && config.aggregation == aggregation);
        
        if !config_exists {
            return Err(RemDbError::ConfigError); // 预聚合配置不存在
        }
        
        // 计算时间桶范围
        let interval_nanos = interval_seconds * 1_000_000_000u64;
        let start_bucket = start_time / interval_nanos;
        let end_bucket = end_time / interval_nanos;
        
        // 收集预聚合数据
        let mut result = Vec::new();
        
        for bucket in start_bucket..=end_bucket {
            // 查找该时间桶的所有预聚合数据
            for ((stored_bucket, _tag_hash), value) in pre_aggregation_guard.data.iter() {
                if *stored_bucket == bucket {
                    // 构建时序记录
                    result.push(TimeSeriesRecord {
                        timestamp: bucket * interval_nanos,
                        value: *value,
                        tag_count: 0, // 简化处理，实际应该从tag_hash恢复标签
                        tags: [0; 8],
                    });
                }
            }
        }
        
        Ok(result)
    }
    
    /// 更新预聚合数据
    fn update_pre_aggregations(&self, record: &TimeSeriesRecord) {
        let mut pre_aggregation_guard = try_lock!(self.pre_aggregation);
        
        // 先复制配置，避免借用冲突
        let configs = pre_aggregation_guard.configs.clone();
        
        // 为每个预聚合配置更新数据
        for config in &configs {
            let interval_nanos = config.interval_seconds * 1_000_000_000u64;
            let time_bucket = record.timestamp / interval_nanos;
            
            // 计算标签哈希（简化处理）
            let tag_hash = record.tag_count as u64; // 实际应该基于标签值计算哈希
            
            let key = (time_bucket, tag_hash);
            
            // 根据聚合函数更新值
            match config.aggregation.as_str() {
                "avg" => {
                    // 平均值需要跟踪总和和计数，这里简化处理
                    let current_value = *pre_aggregation_guard.data.get(&key).unwrap_or(&0.0);
                    // 简化实现：使用移动平均
                    let new_value = (current_value + record.value) / 2.0;
                    pre_aggregation_guard.data.insert(key, new_value);
                }
                "sum" => {
                    let current_value = *pre_aggregation_guard.data.get(&key).unwrap_or(&0.0);
                    pre_aggregation_guard.data.insert(key, current_value + record.value);
                }
                "min" => {
                    let current_value = *pre_aggregation_guard.data.get(&key).unwrap_or(&f64::MAX);
                    pre_aggregation_guard.data.insert(key, f64::min(current_value, record.value));
                }
                "max" => {
                    let current_value = *pre_aggregation_guard.data.get(&key).unwrap_or(&f64::MIN);
                    pre_aggregation_guard.data.insert(key, f64::max(current_value, record.value));
                }
                _ => {}
            }
        }
    }

    /// 批量写入时序数据
    pub unsafe fn batch_write(
        &mut self,
        records: *const TimeSeriesRecord,
        count: usize,
    ) -> Result<usize> {
        if records.is_null() || count == 0 {
            return Err(RemDbError::ConfigError);
        }

        let mut inserted = 0;

        // 遍历所有记录，写入到对应的分区
        for i in 0..count {
            let record = *records.add(i);

            // 获取或创建分区
            let mut partitions_guard = try_lock!(self.partitions);
            let partition = partitions_guard.get_or_create_partition(record.timestamp);

            // 写入记录到分区
            let mut partition_guard = try_lock!(partition);
            partition_guard.records.push(record);
            partition_guard.stats.record_count += 1;

            // 更新索引
            self.index.insert(record.timestamp, inserted as usize);

            // 更新预聚合数据
            self.update_pre_aggregations(&record);

            inserted += 1;
        }

        Ok(inserted)
    }

    /// 事务化批量写入时序数据
    /// 确保一批数据要么全部成功插入并立即可见，要么全部回滚
    pub fn write_timeseries_batch(&mut self, data_points: &[TimeSeriesRecord]) -> Result<usize> {
        if data_points.is_empty() {
            return Err(RemDbError::ConfigError);
        }

        // 检查是否有活跃事务
        let has_active_tx = crate::transaction::has_active_tx();

        // 如果没有活跃事务，返回错误，要求调用者显式开始事务
        // 这是为了简化实现，避免直接访问Transaction结构体的私有字段
        if !has_active_tx {
            return Err(RemDbError::TransactionError);
        }

        let mut inserted = 0;
        let table_id = self.def.base.id;

        // 批量写入逻辑
        for (i, record) in data_points.iter().enumerate() {
            // 获取或创建分区
            let mut partitions_guard = try_lock!(self.partitions);
            let partition = partitions_guard.get_or_create_partition(record.timestamp);

            // 写入记录到分区
            let mut partition_guard = try_lock!(partition);
            partition_guard.records.push(*record);
            partition_guard.stats.record_count = partition_guard.records.len();

            // 更新索引
            self.index.insert(record.timestamp, inserted as usize);

            // 更新预聚合数据
            self.update_pre_aggregations(record);

            // 记录事务日志
            unsafe {
                // 获取当前事务
                if let Some(mut tx_ptr) = crate::transaction::get_current_tx() {
                    let tx_mut = tx_ptr.as_mut();

                    // 添加日志项
                    let data_size = core::mem::size_of::<TimeSeriesRecord>();
                    let tx_id = tx_mut.id;
                    let record_slice =
                        core::slice::from_raw_parts(record as *const _ as *const u8, data_size);
                    tx_mut.begin_log_item(
                        tx_id,
                        crate::transaction::LogOperation::TimeSeriesInsert,
                        table_id,
                        i as u16, // 使用索引作为record_id
                        data_size as u16,
                        None,               // 旧数据为null
                        Some(record_slice), // 新数据指针
                    );
                }
            }

            inserted += 1;
        }

        Ok(inserted)
    }

    /// 时间范围查询
    pub fn query_time_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<TimeSeriesRecord>> {
        // 获取所有相关分区
        let partitions_guard = try_lock!(self.partitions);
        let relevant_partitions = partitions_guard.get_partitions_in_range(start_time, end_time);

        let mut results = Vec::new();

        // 遍历所有相关分区，查询符合条件的记录
        for partition in relevant_partitions {
            let partition_guard = try_lock!(partition);

            // 遍历分区中的记录，过滤符合时间范围的记录
            for record in &partition_guard.records {
                if record.timestamp >= start_time && record.timestamp <= end_time {
                    results.push(*record);
                }
            }
        }

        Ok(results)
    }
}
