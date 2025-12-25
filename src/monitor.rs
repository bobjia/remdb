#![cfg_attr(not(feature = "std"), no_std)]

use core::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use alloc::string::String;

/// 数据库健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 不健康
    Unhealthy,
}

/// 数据库监控指标
#[derive(Debug)]
pub struct DbMetrics {
    /// 总内存（字节）
    pub total_memory: usize,
    /// 已使用内存（字节）
    pub used_memory: AtomicUsize,
    /// 读取操作计数
    pub read_ops: AtomicU64,
    /// 写入操作计数
    pub write_ops: AtomicU64,
    /// 删除操作计数
    pub delete_ops: AtomicU64,
    /// 更新操作计数
    pub update_ops: AtomicU64,
    /// 缓存命中次数
    pub cache_hits: AtomicU64,
    /// 缓存未命中次数
    pub cache_misses: AtomicU64,
    /// 索引查找次数
    pub index_lookups: AtomicU64,
    /// 索引插入次数
    pub index_inserts: AtomicU64,
    /// 索引删除次数
    pub index_deletes: AtomicU64,
    /// 事务总数
    pub transactions: AtomicU64,
    /// 已提交事务数
    pub committed_transactions: AtomicU64,
    /// 已回滚事务数
    pub rolled_back_transactions: AtomicU64,
}

/// 数据库监控指标快照
#[derive(Debug, Clone, Copy)]
pub struct DbMetricsSnapshot {
    /// 总内存（字节）
    pub total_memory: usize,
    /// 已使用内存（字节）
    pub used_memory: usize,
    /// 读取操作计数
    pub read_ops: u64,
    /// 写入操作计数
    pub write_ops: u64,
    /// 删除操作计数
    pub delete_ops: u64,
    /// 更新操作计数
    pub update_ops: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 索引查找次数
    pub index_lookups: u64,
    /// 索引插入次数
    pub index_inserts: u64,
    /// 索引删除次数
    pub index_deletes: u64,
    /// 事务总数
    pub transactions: u64,
    /// 已提交事务数
    pub committed_transactions: u64,
    /// 已回滚事务数
    pub rolled_back_transactions: u64,
    /// 缓存命中率（百分比）
    pub cache_hit_rate: f64,
}

/// 健康检查结果
#[derive(Debug)]
pub struct HealthCheckResult {
    /// 健康状态
    pub status: HealthStatus,
    /// 健康检查时间戳
    pub timestamp: u64,
    /// 指标快照
    pub metrics: DbMetricsSnapshot,
    /// 详细信息
    pub details: String,
}

impl DbMetrics {
    /// 创建新的监控指标实例
    pub fn new(total_memory: usize) -> Self {
        DbMetrics {
            total_memory,
            used_memory: AtomicUsize::new(0),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            delete_ops: AtomicU64::new(0),
            update_ops: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            index_lookups: AtomicU64::new(0),
            index_inserts: AtomicU64::new(0),
            index_deletes: AtomicU64::new(0),
            transactions: AtomicU64::new(0),
            committed_transactions: AtomicU64::new(0),
            rolled_back_transactions: AtomicU64::new(0),
        }
    }

    /// 创建指标快照
    pub fn snapshot(&self) -> DbMetricsSnapshot {
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        let cache_total = cache_hits + cache_misses;
        let cache_hit_rate = if cache_total > 0 {
            (cache_hits as f64 / cache_total as f64) * 100.0
        } else {
            0.0
        };

        DbMetricsSnapshot {
            total_memory: self.total_memory,
            used_memory: self.used_memory.load(Ordering::Relaxed),
            read_ops: self.read_ops.load(Ordering::Relaxed),
            write_ops: self.write_ops.load(Ordering::Relaxed),
            delete_ops: self.delete_ops.load(Ordering::Relaxed),
            update_ops: self.update_ops.load(Ordering::Relaxed),
            cache_hits,
            cache_misses,
            index_lookups: self.index_lookups.load(Ordering::Relaxed),
            index_inserts: self.index_inserts.load(Ordering::Relaxed),
            index_deletes: self.index_deletes.load(Ordering::Relaxed),
            transactions: self.transactions.load(Ordering::Relaxed),
            committed_transactions: self.committed_transactions.load(Ordering::Relaxed),
            rolled_back_transactions: self.rolled_back_transactions.load(Ordering::Relaxed),
            cache_hit_rate,
        }
    }

    /// 重置所有指标
    pub fn reset(&self) {
        self.used_memory.store(0, Ordering::Relaxed);
        self.read_ops.store(0, Ordering::Relaxed);
        self.write_ops.store(0, Ordering::Relaxed);
        self.delete_ops.store(0, Ordering::Relaxed);
        self.update_ops.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.index_lookups.store(0, Ordering::Relaxed);
        self.index_inserts.store(0, Ordering::Relaxed);
        self.index_deletes.store(0, Ordering::Relaxed);
        self.transactions.store(0, Ordering::Relaxed);
        self.committed_transactions.store(0, Ordering::Relaxed);
        self.rolled_back_transactions.store(0, Ordering::Relaxed);
    }

    /// 增加读取操作计数
    pub fn inc_read_ops(&self) {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加写入操作计数
    pub fn inc_write_ops(&self) {
        self.write_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加删除操作计数
    pub fn inc_delete_ops(&self) {
        self.delete_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加更新操作计数
    pub fn inc_update_ops(&self) {
        self.update_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加缓存命中次数
    pub fn inc_cache_hits(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加缓存未命中次数
    pub fn inc_cache_misses(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加索引查找次数
    pub fn inc_index_lookups(&self) {
        self.index_lookups.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加索引插入次数
    pub fn inc_index_inserts(&self) {
        self.index_inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加索引删除次数
    pub fn inc_index_deletes(&self) {
        self.index_deletes.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加事务计数
    pub fn inc_transactions(&self) {
        self.transactions.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加已提交事务计数
    pub fn inc_committed_transactions(&self) {
        self.committed_transactions.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加已回滚事务计数
    pub fn inc_rolled_back_transactions(&self) {
        self.rolled_back_transactions.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新已使用内存
    pub fn set_used_memory(&self, memory: usize) {
        self.used_memory.store(memory, Ordering::Relaxed);
    }

    /// 增加已使用内存
    pub fn add_used_memory(&self, memory: usize) {
        self.used_memory.fetch_add(memory, Ordering::Relaxed);
    }

    /// 减少已使用内存
    pub fn sub_used_memory(&self, memory: usize) {
        self.used_memory.fetch_sub(memory, Ordering::Relaxed);
    }
}

impl DbMetricsSnapshot {
    /// 将指标转换为文本格式
    pub fn to_text(&self) -> String {
        let mut text = alloc::string::String::new();
        text.push_str("===== 数据库监控指标 =====\n");
        text.push_str(&format!("内存使用: {}/{} 字节\n", self.used_memory, self.total_memory));
        text.push_str(&format!("操作计数: 读={}, 写={}, 删除={}, 更新={}\n", 
                               self.read_ops, self.write_ops, self.delete_ops, self.update_ops));
        text.push_str(&format!("缓存统计: 命中={}, 未命中={}, 命中率={:.2}%\n", 
                               self.cache_hits, self.cache_misses, self.cache_hit_rate));
        text.push_str(&format!("索引操作: 查找={}, 插入={}, 删除={}\n", 
                               self.index_lookups, self.index_inserts, self.index_deletes));
        text.push_str(&format!("事务统计: 总数={}, 已提交={}, 已回滚={}\n", 
                               self.transactions, self.committed_transactions, self.rolled_back_transactions));
        text.push_str("========================\n");
        text
    }
}

impl HealthCheckResult {
    /// 创建新的健康检查结果
    pub fn new(status: HealthStatus, metrics: DbMetricsSnapshot, details: String) -> Self {
        HealthCheckResult {
            status,
            timestamp: crate::platform::get_timestamp(),
            metrics,
            details,
        }
    }

    /// 将健康检查结果转换为文本格式
    pub fn to_text(&self) -> String {
        let mut text = alloc::string::String::new();
        text.push_str(&format!("===== 健康检查结果 =====\n"));
        text.push_str(&format!("时间戳: {}\n", self.timestamp));
        text.push_str(&format!("状态: {:?}\n", self.status));
        text.push_str(&format!("详细信息: {}\n", self.details));
        text.push_str(&self.metrics.to_text());
        text.push_str("========================\n");
        text
    }
}