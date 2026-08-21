#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(not(feature = "std"))]
pub struct OnceLock<T> {
    data: core::cell::UnsafeCell<Option<T>>,
    initialized: core::sync::atomic::AtomicBool,
}

#[cfg(not(feature = "std"))]
unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}

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
            unsafe {
                (*self.data.get()).as_ref()
            }
        } else {
            None
        }
    }
    
    pub fn set(&self, value: T) -> core::result::Result<(), T> {
        if self.initialized.swap(true, core::sync::atomic::Ordering::AcqRel) {
            Err(value)
        } else {
            unsafe {
                *self.data.get() = Some(value);
            }
            Ok(())
        }
    }
    
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        match self.get() {
            Some(v) => v,
            None => {
                let _ = self.set(f());
                unsafe {
                    (*self.data.get()).as_ref().unwrap()
                }
            }
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
    
    /// 内存拷贝（安全版本）——从 src 拷贝到 dest，拷贝长度为 min(dest.len(), src.len())
    fn memcpy(&self, dest: &mut [u8], src: &[u8]);
    
    /// 内存设置 —— 将 dest 的所有字节设为 value
    fn memset(&self, dest: &mut [u8], value: u8);
    
    /// 延迟（毫秒）
    fn delay_ms(&self, ms: u32);
    
    /// 延迟（微秒）
    fn delay_us(&self, us: u32);
    
    /// 打开文件
    fn file_open(&self, path: &str, mode: FileMode) -> FileResult<FileHandle>;
    
    /// 关闭文件
    fn file_close(&self, handle: FileHandle) -> FileResult<()>;
    
    /// 写入文件
    fn file_write(&self, handle: FileHandle, buf: &[u8]) -> FileResult<usize>;
    
    /// 读取文件
    fn file_read(&self, handle: FileHandle, buf: &mut [u8]) -> FileResult<usize>;
    
    /// 文件定位
    fn file_seek(&self, handle: FileHandle, offset: i64, whence: SeekWhence) -> FileResult<u64>;
    
    /// 删除文件
    fn file_remove(&self, path: &str) -> FileResult<()>;
    
    /// 获取文件大小
    fn file_size(&self, path: &str) -> FileResult<usize>;
    
    /// 计算CRC32校验和
    fn crc32(&self, data: &[u8]) -> u32;
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

/// 文件句柄类型 - 使用 usize 作为通用句柄类型
pub type FileHandle = usize;

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

/// 获取当前时间戳（毫秒）
pub fn get_timestamp() -> u64 {
    if let Some(platform) = PLATFORM.get() {
        platform.get_timestamp()
    } else {
        panic!("Platform not initialized")
    }
}

/// 获取当前时间戳（微秒）
pub fn get_timestamp_us() -> u64 {
    if let Some(platform) = PLATFORM.get() {
        platform.get_timestamp_us()
    } else {
        panic!("Platform not initialized")
    }
}

/// 内存拷贝（安全版本）——从 src 拷贝到 dest
pub fn memcpy(dest: &mut [u8], src: &[u8]) {
    if let Some(platform) = PLATFORM.get() {
        platform.memcpy(dest, src)
    } else {
        panic!("Platform not initialized")
    }
}

/// 内存设置
pub fn memset(dest: &mut [u8], value: u8) {
    if let Some(platform) = PLATFORM.get() {
        platform.memset(dest, value)
    } else {
        panic!("Platform not initialized")
    }
}

/// 延迟（毫秒）
pub fn delay_ms(ms: u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.delay_ms(ms)
    } else {
        panic!("Platform not initialized")
    }
}

/// 延迟（微秒）
pub fn delay_us(us: u32) {
    if let Some(platform) = PLATFORM.get() {
        platform.delay_us(us)
    } else {
        panic!("Platform not initialized")
    }
}

/// 打开文件
pub fn file_open(path: &str, mode: FileMode) -> FileResult<FileHandle> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_open(path, mode)
    } else {
        panic!("Platform not initialized")
    }
}

/// 关闭文件
pub fn file_close(handle: FileHandle) -> FileResult<()> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_close(handle)
    } else {
        panic!("Platform not initialized")
    }
}

/// 写入文件
pub fn file_write(handle: FileHandle, buf: &[u8]) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_write(handle, buf)
    } else {
        panic!("Platform not initialized")
    }
}

/// 读取文件
pub fn file_read(handle: FileHandle, buf: &mut [u8]) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_read(handle, buf)
    } else {
        panic!("Platform not initialized")
    }
}

/// 文件定位
pub fn file_seek(handle: FileHandle, offset: i64, whence: SeekWhence) -> FileResult<u64> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_seek(handle, offset, whence)
    } else {
        panic!("Platform not initialized")
    }
}

/// 删除文件
pub fn file_remove(path: &str) -> FileResult<()> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_remove(path)
    } else {
        panic!("Platform not initialized")
    }
}

/// 获取文件大小
pub fn file_size(path: &str) -> FileResult<usize> {
    if let Some(platform) = PLATFORM.get() {
        platform.file_size(path)
    } else {
        panic!("Platform not initialized")
    }
}

/// 计算CRC32校验和
pub fn crc32(data: &[u8]) -> u32 {
    if let Some(platform) = PLATFORM.get() {
        platform.crc32(data)
    } else {
        panic!("Platform not initialized")
    }
}

// 重新导出子模块
pub mod posix;
pub mod baremetal;