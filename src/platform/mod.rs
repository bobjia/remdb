// 使用条件编译，在std环境下使用std::sync::OnceLock，在no_std环境下使用自定义实现
#[cfg(feature = "std")]
use std::sync::OnceLock;

// no_std环境下的简单OnceLock实现
#[cfg(not(feature = "std"))]
pub struct OnceLock<T> {
    data: core::cell::UnsafeCell<Option<T>>,
    initialized: core::sync::atomic::AtomicBool,
}

// 为OnceLock添加Sync trait实现
#[cfg(not(feature = "std"))]
unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}

// 为OnceLock添加Send trait实现
#[cfg(not(feature = "std"))]
unsafe impl<T: Send> Send for OnceLock<T> {}

#[cfg(not(feature = "std"))]
impl<T> OnceLock<T> {
    pub const fn new() -> Self {
        OnceLock {
            data: core::cell::UnsafeCell::new(None),
            initialized: core::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.initialized.load(core::sync::atomic::Ordering::Acquire) {
            unsafe { (*self.data.get()).as_ref() }
        } else {
            None
        }
    }

    pub fn set(&self, value: T) -> core::result::Result<(), T> {
        if self
            .initialized
            .swap(true, core::sync::atomic::Ordering::AcqRel)
        {
            Err(value)
        } else {
            unsafe {
                *self.data.get() = Some(value);
            }
            Ok(())
        }
    }
}

/// 文件操作结果类型
pub type FileResult<T> = core::result::Result<T, ()>;

/// 平台抽象层接口
pub trait Platform: Send + Sync {
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

    /// 打开文件
    fn file_open(&self, path: &str, mode: FileMode) -> FileResult<FileHandle>;

    /// 关闭文件
    fn file_close(&self, handle: FileHandle) -> FileResult<()>;

    /// 写入文件
    fn file_write(&self, handle: FileHandle, buffer: *const u8, size: usize) -> FileResult<usize>;

    /// 读取文件
    fn file_read(&self, handle: FileHandle, buffer: *mut u8, size: usize) -> FileResult<usize>;

    /// 文件定位
    fn file_seek(&self, handle: FileHandle, offset: i64, whence: SeekWhence) -> FileResult<u64>;

    /// 删除文件
    fn file_remove(&self, path: &str) -> FileResult<()>;

    /// 获取文件大小
    fn file_size(&self, path: &str) -> FileResult<usize>;

    /// 计算CRC32校验和
    fn crc32(&self, data: *const u8, size: usize) -> u32;
}

/// 文件模式
#[derive(Copy, Clone)]
pub enum FileMode {
    /// 只读模式
    Read,
    /// 只写模式，创建文件（如果不存在）
    Write,
    /// 读写模式，创建文件（如果不存在）
    ReadWrite,
    /// 只写模式，追加到文件末尾
    Append,
}

/// 文件句柄类型 - 使用*const u8作为通用句柄类型，可以容纳任何指针
pub type FileHandle = *const u8;

/// 文件定位起始位置
#[derive(Copy, Clone)]
pub enum SeekWhence {
    /// 从文件开头
    SeekSet,
    /// 从当前位置
    SeekCur,
    /// 从文件末尾
    SeekEnd,
}

/// 全局平台实例
pub static PLATFORM: OnceLock<&'static dyn Platform> = OnceLock::new();

/// 初始化平台抽象层
pub fn init_platform(platform: &'static dyn Platform) {
    PLATFORM.set(platform).ok();
}

/// 重置平台抽象层（仅用于测试）
#[cfg(test)]
pub fn reset_platform() {
    // 使用unsafe代码重置OnceLock，仅在测试中使用
    unsafe {
        // 重置initialized标志
        #[cfg(not(feature = "std"))]
        {
            let platform_ptr = &PLATFORM as *const OnceLock<&'static dyn Platform> as *mut OnceLock<&'static dyn Platform>;
            (*platform_ptr).initialized.store(false, core::sync::atomic::Ordering::Release);
            // 清空数据
            *(*platform_ptr).data.get() = None;
        }
        // 在std环境下，我们无法直接重置OnceLock，所以不做任何操作
        // 测试代码应该适应这一限制
    }
}

/// 获取当前时间戳（毫秒）
pub fn get_timestamp() -> u64 {
    if let Some(platform) = PLATFORM.get() {
        platform.get_timestamp()
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        0
    }
}

/// 获取当前时间戳（微秒）
pub fn get_timestamp_us() -> u64 {
    if let Some(platform) = PLATFORM.get() {
        platform.get_timestamp_us()
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        0
    }
}

/// 自旋锁实现
pub fn spin_lock(lock: &mut u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.spin_lock(lock)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 自旋锁释放
pub fn spin_unlock(lock: &mut u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.spin_unlock(lock)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 内存屏障 - 编译器屏障
pub fn compiler_barrier() {
    if let Some(platform) = PLATFORM.get() {
        platform.compiler_barrier()
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 内存屏障 - 读写屏障
pub fn full_memory_barrier() {
    if let Some(platform) = PLATFORM.get() {
        platform.full_memory_barrier()
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 内存拷贝（安全版本）
pub fn memcpy(dest: *mut u8, src: *const u8, size: usize) {
    if let Some(platform) = PLATFORM.get() {
        platform.memcpy(dest, src, size)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 内存设置
pub fn memset(dest: *mut u8, value: u8, size: usize) {
    if let Some(platform) = PLATFORM.get() {
        platform.memset(dest, value, size)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 延迟（毫秒）
pub fn delay_ms(ms: u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.delay_ms(ms)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 延迟（微秒）
pub fn delay_us(us: u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.delay_us(us)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
    }
}

/// 打开文件
pub fn file_open(path: &str, mode: FileMode) -> FileResult<FileHandle> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_open(path, mode)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 关闭文件
pub fn file_close(handle: FileHandle) -> FileResult<()> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_close(handle)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 写入文件
pub fn file_write(handle: FileHandle, buffer: *const u8, size: usize) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_write(handle, buffer, size)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 读取文件
pub fn file_read(handle: FileHandle, buffer: *mut u8, size: usize) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_read(handle, buffer, size)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 文件定位
pub fn file_seek(handle: FileHandle, offset: i64, whence: SeekWhence) -> FileResult<u64> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_seek(handle, offset, whence)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 删除文件
pub fn file_remove(path: &str) -> FileResult<()> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_remove(path)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 获取文件大小
pub fn file_size(path: &str) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_size(path)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        Err(())
    }
}

/// 计算CRC32校验和
pub fn crc32(data: *const u8, size: usize) -> u32 {
    if let Some(platform) = PLATFORM.get() {
        platform.crc32(data, size)
    } else {
        #[cfg(feature = "log")]
        crate::log::error!("Platform not initialized");
        0
    }
}

// 重新导出子模块
pub mod baremetal;
pub mod posix;
