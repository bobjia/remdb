#![cfg_attr(not(feature = "std"), no_std)]

use core::time::Duration;
use alloc::{sync::Arc, vec::Vec};
use std::sync::Mutex;

use super::TimeSeriesRecord;

/// 分区统计信息
#[derive(Debug, Clone, Default)]
pub struct PartitionStats {
    /// 记录数
    pub record_count: usize,
    /// 压缩前大小（字节）
    pub uncompressed_size: usize,
    /// 压缩后大小（字节）
    pub compressed_size: usize,
    /// 最后访问时间
    pub last_access_time: u64,
}

/// 时间分区结构
#[derive(Debug)]
pub struct TimeSeriesPartition {
    /// 分区开始时间
    pub start_time: u64,
    /// 分区结束时间
    pub end_time: u64,
    /// 记录列表
    pub records: Vec<TimeSeriesRecord>,
    /// 是否已压缩
    pub compressed: bool,
    /// 分区统计信息
    pub stats: PartitionStats,
}

impl TimeSeriesPartition {
    /// 创建新的分区
    pub fn new(start_time: u64, end_time: u64) -> Self {
        Self {
            start_time,
            end_time,
            records: Vec::new(),
            compressed: false,
            stats: PartitionStats::default(),
        }
    }
    
    /// 计算分区大小
    pub fn calculate_size(&self) -> usize {
        self.records.len() * core::mem::size_of::<TimeSeriesRecord>()
    }
    
    /// 清空分区
    pub fn clear(&mut self) {
        self.records.clear();
        self.stats.record_count = 0;
        self.stats.uncompressed_size = 0;
        self.stats.compressed_size = 0;
    }
}

/// 分区管理器
pub struct PartitionManager {
    /// 分区列表
    partitions: Vec<Arc<Mutex<TimeSeriesPartition>>>,
    /// 分区时长（秒）
    partition_duration: u64,
    /// 最大分区数
    max_partitions: usize,
}

impl PartitionManager {
    /// 创建新的分区管理器
    pub fn new(partition_duration: Duration, max_partitions: usize) -> Self {
        Self {
            partitions: Vec::new(),
            partition_duration: partition_duration.as_secs(),
            max_partitions,
        }
    }
    
    /// 获取或创建分区
    pub fn get_or_create_partition(&mut self, timestamp: u64) -> Arc<Mutex<TimeSeriesPartition>> {
        let partition_key = timestamp / self.partition_duration;
        let start_time = partition_key * self.partition_duration;
        let end_time = start_time + self.partition_duration;
        
        // 检查是否已存在该分区
        for partition in &self.partitions {
            let p = partition.lock().unwrap();
            if p.start_time == start_time {
                return partition.clone();
            }
        }
        
        // 创建新分区
        let new_partition = Arc::new(Mutex::new(TimeSeriesPartition::new(start_time, end_time)));
        
        // 添加到分区列表
        self.partitions.push(new_partition.clone());
        
        // 如果分区数超过最大值，删除最旧的分区
        if self.partitions.len() > self.max_partitions {
            self.partitions.remove(0);
        }
        
        new_partition
    }
    
    /// 获取指定时间范围内的所有分区
    pub fn get_partitions_in_range(&self, start_time: u64, end_time: u64) -> Vec<Arc<Mutex<TimeSeriesPartition>>> {
        let mut result = Vec::new();
        
        for partition in &self.partitions {
            let p = partition.lock().unwrap();
            if p.start_time <= end_time && p.end_time >= start_time {
                result.push(partition.clone());
            }
        }
        
        result
    }
    
    /// 获取最旧的分区
    pub fn get_oldest_partition(&self) -> Option<Arc<Mutex<TimeSeriesPartition>>> {
        self.partitions.first().cloned()
    }
    
    /// 获取最新的分区
    pub fn get_newest_partition(&self) -> Option<Arc<Mutex<TimeSeriesPartition>>> {
        self.partitions.last().cloned()
    }
    
    /// 清理过期分区
    pub fn cleanup_expired_partitions(&mut self, current_time: u64, retention_period: Duration) {
        let expire_time = current_time - retention_period.as_secs();
        
        self.partitions.retain(|partition| {
            let p = partition.lock().unwrap();
            p.end_time > expire_time
        });
    }
    
    /// 获取分区数量
    pub fn get_partition_count(&self) -> usize {
        self.partitions.len()
    }
    
    /// 获取指定时间戳所在的分区
    pub fn get_partition(&self, timestamp: u64) -> Option<Arc<Mutex<TimeSeriesPartition>>> {
        let partition_key = timestamp / self.partition_duration;
        let start_time = partition_key * self.partition_duration;
        
        for partition in &self.partitions {
            let p = partition.lock().unwrap();
            if p.start_time == start_time {
                return Some(partition.clone());
            }
        }
        
        None
    }
    
    /// 压缩所有可压缩的分区
    pub fn compress_all_partitions(&self) {
        for partition in &self.partitions {
            let mut p = partition.lock().unwrap();
            if !p.compressed {
                // 压缩分区逻辑（需要与压缩模块集成）
                p.compressed = true;
            }
        }
    }
}
