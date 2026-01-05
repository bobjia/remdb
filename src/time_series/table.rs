#![cfg_attr(not(feature = "std"), no_std)]

use core::time::Duration;
use alloc::{sync::Arc, vec::Vec};
use crate::{TableDef, Result, RemDbError};
use super::{CompressionType, TimeSeriesIndex, TimeSeriesPartition, PartitionManager, LifecycleManager};

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
        max_partitions: usize
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
    3600, // 1小时
    7 * 24 * 3600, // 7天
    CompressionType::DeltaRunLength,
    1000
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
    pub tag_fields: &'static [usize],
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

/// 时序表结构
pub struct TimeSeriesTable {
    /// 表定义
    pub def: Arc<TimeSeriesTableDef>,
    /// 分区管理器
    pub partitions: PartitionManager,
    /// 时序索引
    pub index: Arc<TimeSeriesIndex>,
    /// 生命周期管理器
    pub lifecycle: LifecycleManager,
}

impl TimeSeriesTable {
    /// 创建新的时序表
    pub fn new(
        def: Arc<TimeSeriesTableDef>,
        index: Arc<TimeSeriesIndex>
    ) -> Result<Self> {
        // 检查时间字段和值字段的有效性
        if def.time_field >= def.base.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }
        
        if def.value_field >= def.base.fields.len() {
            return Err(RemDbError::FieldNotFound);
        }
        
        // 检查标签字段的有效性
        for &tag_field in def.tag_fields {
            if tag_field >= def.base.fields.len() {
                return Err(RemDbError::FieldNotFound);
            }
        }
        
        // 创建分区管理器
        let partition_manager = PartitionManager::new(
            def.config.partition_duration(),
            def.config.max_partitions
        );
        
        // 创建生命周期管理器
        let lifecycle_manager = LifecycleManager::new(
            def.config.retention_period()
        );
        
        Ok(Self {
            def,
            partitions: partition_manager,
            index,
            lifecycle: lifecycle_manager,
        })
    }
    
    /// 批量写入时序数据
    pub unsafe fn batch_write(
        &mut self,
        records: *const TimeSeriesRecord,
        count: usize
    ) -> Result<usize> {
        if records.is_null() || count == 0 {
            return Err(RemDbError::ConfigError);
        }
        
        let mut inserted = 0;
        
        // 遍历所有记录，写入到对应的分区
        for i in 0..count {
            let record = *records.add(i);
            
            // 获取或创建分区
            let partition = self.partitions.get_or_create_partition(record.timestamp);
            
            // 写入记录到分区
            let mut partition_guard = partition.lock().unwrap();
            partition_guard.records.push(record);
            partition_guard.stats.record_count += 1;
            
            // 更新索引
            self.index.insert(record.timestamp, inserted as usize);
            
            inserted += 1;
        }
        
        Ok(inserted)
    }
    
    /// 事务化批量写入时序数据
    /// 确保一批数据要么全部成功插入并立即可见，要么全部回滚
    pub fn write_timeseries_batch(
        &mut self,
        data_points: &[TimeSeriesRecord]
    ) -> Result<usize> {
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
            let partition = self.partitions.get_or_create_partition(record.timestamp);
            
            // 写入记录到分区
            let mut partition_guard = partition.lock().unwrap();
            partition_guard.records.push(*record);
            partition_guard.stats.record_count = partition_guard.records.len();
            
            // 更新索引
            self.index.insert(record.timestamp, inserted as usize);
            
            // 记录事务日志
            unsafe {
                // 获取当前事务
                if let Some(mut tx_ptr) = crate::transaction::get_current_tx() {
                    let tx_mut = tx_ptr.as_mut();
                    
                    // 添加日志项
                    let data_size = core::mem::size_of::<TimeSeriesRecord>();
                    tx_mut.add_log_item(
                        crate::transaction::LogOperation::TimeSeriesInsert,
                        table_id,
                        i as u16, // 使用索引作为record_id
                        core::ptr::null(), // 旧数据为null
                        record as *const _ as *const u8, // 新数据指针
                        data_size
                    )?;
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
        end_time: u64
    ) -> Result<Vec<TimeSeriesRecord>> {
        // 获取所有相关分区
        let relevant_partitions = self.partitions.get_partitions_in_range(start_time, end_time);
        
        let mut results = Vec::new();
        
        // 遍历所有相关分区，查询符合条件的记录
        for partition in relevant_partitions {
            let partition_guard = partition.lock().unwrap();
            
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
