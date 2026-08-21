#![cfg(feature = "baremetal")]

use super::Platform;
use super::{FileMode, FileHandle, SeekWhence, FileResult};
use core::ptr;

/// 裸机平台实现
pub struct BareMetalPlatform;

impl Platform for BareMetalPlatform {
    /// 获取当前时间戳（毫秒）
    fn get_timestamp(&self) -> u64 {
        // 裸机环境下需要用户提供时钟实现
        // 这里使用一个简单的计数器，实际应用中应该替换为硬件时钟
        static mut COUNTER: u64 = 0;
        unsafe {
            COUNTER += 1;
            COUNTER
        }
    }
    
    /// 获取当前时间戳（微秒）
    fn get_timestamp_us(&self) -> u64 {
        // 裸机环境下需要用户提供时钟实现
        static mut COUNTER_US: u64 = 0;
        unsafe {
            COUNTER_US += 1;
            COUNTER_US
        }
    }
    
    /// 内存拷贝（安全版本）
    fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
        let len = core::cmp::min(dest.len(), src.len());
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dest.as_mut_ptr(), len);
        }
    }
    
    /// 内存设置
    fn memset(&self, dest: &mut [u8], value: u8) {
        unsafe {
            ptr::write_bytes(dest.as_mut_ptr(), value, dest.len());
        }
    }
    
    /// 延迟（毫秒）
    fn delay_ms(&self, ms: u32) {
        // 简单的忙等待延迟
        // 实际应用中应该使用硬件定时器
        let delay_cycles = ms as u32 * 1000;
        
        unsafe {
            let mut i = 0;
            while i < delay_cycles {
                core::hint::spin_loop();
                i += 1;
            }
        }
    }
    
    /// 延迟（微秒）
    fn delay_us(&self, us: u32) {
        // 简单的忙等待延迟
        let delay_cycles = us;
        
        unsafe {
            let mut i = 0;
            while i < delay_cycles {
                core::hint::spin_loop();
                i += 1;
            }
        }
    }
    
    /// 打开文件 - 裸机环境下返回错误
    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> {
        Err(())
    }
    
    /// 关闭文件 - 裸机环境下返回错误
    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Err(())
    }
    
    /// 写入文件 - 裸机环境下返回错误
    fn file_write(&self, _handle: FileHandle, _buf: &[u8]) -> FileResult<usize> {
        Err(())
    }
    
    /// 读取文件 - 裸机环境下返回错误
    fn file_read(&self, _handle: FileHandle, _buf: &mut [u8]) -> FileResult<usize> {
        Err(())
    }
    
    /// 文件定位 - 裸机环境下返回错误
    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
        Err(())
    }
    
    /// 删除文件 - 裸机环境下返回错误
    fn file_remove(&self, _path: &str) -> FileResult<()> {
        Err(())
    }
    
    /// 获取文件大小 - 裸机环境下返回错误
    fn file_size(&self, _path: &str) -> FileResult<usize> {
        Err(())
    }
    
    /// 计算CRC32校验和 - 裸机环境下返回0
    fn crc32(&self, _data: &[u8]) -> u32 {
        0
    }
}

/// 获取裸机平台实例
pub fn get_baremetal_platform() -> &'static dyn Platform {
    &BareMetalPlatform
}