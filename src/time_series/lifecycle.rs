use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

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
    /// 是否正在运行（原子布尔值，Arc包装）
    running: Arc<AtomicBool>,
    /// 清理闭包，线程安全
    cleanup_callback: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl LifecycleManager {
    /// 创建新的生命周期管理器
    pub fn new(retention_period: Duration) -> Self {
        Self {
            retention_period,
            cleanup_interval: Duration::from_secs(5 * 60), // 5分钟
            running: Arc::new(AtomicBool::new(false)),
            cleanup_callback: None,
        }
    }

    /// 设置清理间隔
    pub fn set_cleanup_interval(&mut self, interval: Duration) {
        self.cleanup_interval = interval;
    }

    /// 设置清理闭包
    pub fn set_cleanup_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.cleanup_callback = Some(Arc::new(callback));
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

        #[cfg(feature = "std")]
        {
            // 克隆所需数据
            let cleanup_interval = self.cleanup_interval;
            let running = self.running.clone();
            let cleanup = self.cleanup_callback.clone();

            std::thread::spawn(move || {
                while running.load(Ordering::SeqCst) {
                    // 执行清理逻辑
                    if let Some(callback) = cleanup.as_ref() {
                        callback();
                    }

                    // 休眠指定间隔
                    std::thread::sleep(cleanup_interval);
                }
            });
        }
    }

    /// 停止清理任务
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// 执行清理逻辑
    pub fn cleanup(&self) {
        if let Some(callback) = &self.cleanup_callback {
            callback();
        }
    }
}
