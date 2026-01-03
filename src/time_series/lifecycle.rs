#![cfg_attr(not(feature = "std"), no_std)]

use core::time::Duration;
use core::sync::atomic::{AtomicBool, Ordering};


/// 数据保留策略
#[derive(Debug, Clone)]
pub enum RetentionPolicy {
    /// 按时间保留
    TimeBased(Duration),
    /// 按记录数保留
    CountBased(usize),
    /// 混合策略
    Hybrid(Duration, usize),
}

/// 生命周期管理器
pub struct LifecycleManager {
    /// 数据保留期
    retention_period: Duration,
    /// 清理间隔
    cleanup_interval: Duration,
    /// 是否正在运行（原子布尔值）
    running: AtomicBool,
}

impl LifecycleManager {
    /// 创建新的生命周期管理器
    pub fn new(retention_period: Duration) -> Self {
        Self {
            retention_period,
            cleanup_interval: Duration::from_secs(5 * 60), // 5分钟
            running: AtomicBool::new(false),
        }
    }
    
    /// 设置清理间隔
    pub fn set_cleanup_interval(&mut self, interval: Duration) {
        self.cleanup_interval = interval;
    }
    
    /// 获取当前时间戳（秒）
    pub fn get_current_timestamp() -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }
        #[cfg(not(feature = "std"))]
        {
            // 非标准库环境下，返回0或其他合适的默认值
            0
        }
    }
    
    /// 检查数据是否过期
    pub fn is_expired(&self, timestamp: u64) -> bool {
        let now = Self::get_current_timestamp();
        let expire_time = now - self.retention_period.as_secs();
        timestamp < expire_time
    }
    
    /// 启动清理任务
    pub fn start(&self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        
        self.running.store(true, Ordering::SeqCst);
        
        // 注意：暂时注释掉清理线程，因为存在线程安全问题
        // 后续需要重新设计清理机制
        /*
        #[cfg(feature = "std")] 
        {
            // 创建清理任务的克隆副本
            let retention_period = self.retention_period;
            let cleanup_interval = self.cleanup_interval;
            let running = &self.running;
            
            std::thread::spawn(move || {
                while running.load(Ordering::SeqCst) {
                    // 执行清理逻辑
                    // 注意：这里需要调整清理逻辑，因为无法直接访问self
                    
                    // 休眠指定间隔
                    std::thread::sleep(cleanup_interval);
                }
            });
        }
        */
    }
    
    /// 停止清理任务
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
    
    /// 执行清理逻辑（需要被子模块实现）
    pub fn cleanup(&self) {
        // 默认实现，具体清理逻辑由使用方实现
    }
}
