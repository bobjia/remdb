/// 平台抽象层接口
pub trait Platform {
    /// 获取当前时间戳（毫秒）
    fn get_timestamp(&self) -> u64;
    
    /// 获取当前时间戳（微秒）
    fn get_timestamp_us(&self) -> u64;
    
    /// 自旋锁实现
    fn spin_lock(&self, lock: &mut u32);
    
    /// 自旋锁释放
    fn spin_unlock(&self, lock: &mut u32);
    
    /// 内存屏障 - 编译器屏障
    fn compiler_barrier(&self);
    
    /// 内存屏障 - 读写屏障
    fn full_memory_barrier(&self);
    
    /// 内存拷贝（安全版本）
    fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize);
    
    /// 内存设置
    fn memset(&self, dest: *mut u8, value: u8, size: usize);
    
    /// 延迟（毫秒）
    fn delay_ms(&self, ms: u32);
    
    /// 延迟（微秒）
    fn delay_us(&self, us: u32);
}

/// 全局平台实例
pub static mut PLATFORM: Option<&'static dyn Platform> = None;

/// 初始化平台抽象层
pub unsafe fn init_platform(platform: &'static dyn Platform) {
    PLATFORM = Some(platform);
}

/// 获取当前时间戳（毫秒）
pub fn get_timestamp() -> u64 {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.get_timestamp()
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 获取当前时间戳（微秒）
pub fn get_timestamp_us() -> u64 {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.get_timestamp_us()
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 自旋锁实现
pub fn spin_lock(lock: &mut u32) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.spin_lock(lock)
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 自旋锁释放
pub fn spin_unlock(lock: &mut u32) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.spin_unlock(lock)
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 内存屏障 - 编译器屏障
pub fn compiler_barrier() {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.compiler_barrier()
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 内存屏障 - 读写屏障
pub fn full_memory_barrier() {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.full_memory_barrier()
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 内存拷贝（安全版本）
pub fn memcpy(dest: *mut u8, src: *const u8, size: usize) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.memcpy(dest, src, size)
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 内存设置
pub fn memset(dest: *mut u8, value: u8, size: usize) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.memset(dest, value, size)
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 延迟（毫秒）
pub fn delay_ms(ms: u32) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.delay_ms(ms)
        } else {
            panic!("Platform not initialized")
        }
    }
}

/// 延迟（微秒）
pub fn delay_us(us: u32) {
    unsafe {
        if let Some(platform) = PLATFORM {
            platform.delay_us(us)
        } else {
            panic!("Platform not initialized")
        }
    }
}

// 重新导出子模块
pub mod posix;
pub mod baremetal;
