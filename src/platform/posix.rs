#![cfg(feature = "posix")]

use core::ptr;
use std::io::{Read, Write};
use super::Platform;

/// POSIX平台实现
pub struct PosixPlatform;

impl Platform for PosixPlatform {
    /// 获取当前时间戳（毫秒）
    fn get_timestamp(&self) -> u64 {
        use core::time::Duration;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
        
        now.as_millis() as u64
    }
    
    /// 获取当前时间戳（微秒）
    fn get_timestamp_us(&self) -> u64 {
        use core::time::Duration;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
        
        now.as_micros() as u64
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
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    
    /// 延迟（微秒）
    fn delay_us(&self, us: u32) {
        std::thread::sleep(std::time::Duration::from_micros(us as u64));
    }
    
    /// 打开文件
    fn file_open(&self, path: &str, mode: super::FileMode) -> super::FileResult<super::FileHandle> {
        use std::fs::OpenOptions;
        
        // 创建Box包装的File，将其指针转换为FileHandle
        let mut options = OpenOptions::new();
        
        match mode {
            super::FileMode::Read => {
                options.read(true);
            },
            super::FileMode::Write => {
                options.write(true).create(true).truncate(true);
            },
            super::FileMode::ReadWrite => {
                options.read(true).write(true).create(true);
            },
            super::FileMode::Append => {
                options.write(true).create(true).append(true);
            },
        }
        
        match options.open(path) {
            Ok(file) => {
                // 分配Box存储File，返回其指针作为FileHandle
                let boxed_file = Box::new(file);
                Ok(Box::into_raw(boxed_file) as super::FileHandle)
            },
            Err(_) => Err(()),
        }
    }
    
    /// 关闭文件
    fn file_close(&self, handle: super::FileHandle) -> super::FileResult<()> {
        // 将FileHandle转换回Box<File>并释放
        unsafe {
            let _ = Box::from_raw(handle as *mut std::fs::File);
        }
        Ok(())
    }
    
    /// 写入文件
    fn file_write(&self, handle: super::FileHandle, buf: &[u8]) -> super::FileResult<usize> {
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            match file.write(buf) {
                Ok(bytes_written) => {
                    // 写入后立即刷新，确保数据写入磁盘
                    file.flush().map_err(|_| ())?;
                    Ok(bytes_written)
                },
                Err(_) => Err(()),
            }
        }
    }
    
    /// 读取文件
    fn file_read(&self, handle: super::FileHandle, buf: &mut [u8]) -> super::FileResult<usize> {
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            match file.read(buf) {
                Ok(bytes_read) => Ok(bytes_read),
                Err(_) => Err(()),
            }
        }
    }
    
    /// 文件定位
    fn file_seek(&self, handle: super::FileHandle, offset: i64, whence: super::SeekWhence) -> super::FileResult<u64> {
        use std::io::Seek;
        
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            
            let seek_from = match whence {
                super::SeekWhence::SeekSet => std::io::SeekFrom::Start(offset as u64),
                super::SeekWhence::SeekCur => std::io::SeekFrom::Current(offset),
                super::SeekWhence::SeekEnd => std::io::SeekFrom::End(offset),
            };
            
            match file.seek(seek_from) {
                Ok(new_pos) => Ok(new_pos),
                Err(_) => Err(()),
            }
        }
    }
    
    /// 删除文件
    fn file_remove(&self, path: &str) -> super::FileResult<()> {
        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
    
    /// 获取文件大小
    fn file_size(&self, path: &str) -> super::FileResult<usize> {
        use std::fs::metadata;
        
        match metadata(path) {
            Ok(metadata) => Ok(metadata.len() as usize),
            Err(_) => Err(()),
        }
    }
    
    /// 计算CRC32校验和
    fn crc32(&self, data: *const u8, size: usize) -> u32 {
        // 空数据的CRC32值为0xFFFFFFFF ^ 0xFFFFFFFF = 0
        if size == 0 || data.is_null() {
            return 0;
        }
        

    fn crc32(&self, data: &[u8]) -> u32 {
        // CRC32标准多项式：0xEDB88320
        const CRC32_POLY: u32 = 0xEDB88320;
        
        // 预计算CRC32表
        let mut crc_table = [0u32; 256];
        for i in 0..256 {
            let mut crc = i as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ CRC32_POLY;
                } else {
                    crc >>= 1;
                }
            }
            crc_table[i] = crc;
        }
        
        // 计算CRC32值
        let mut crc = 0xFFFFFFFFu32;
        // 安全创建切片：data非空且size>0
        let data_slice = unsafe { core::slice::from_raw_parts(data, size) };
        
        for &byte in data_slice {

        for &byte in data {
            let index = ((crc ^ byte as u32) & 0xFF) as usize;
            crc = (crc >> 8) ^ crc_table[index];
        }
        
        crc ^ 0xFFFFFFFFu32
    }
}

/// 获取POSIX平台实例
pub fn get_posix_platform() -> &'static dyn Platform {
    &PosixPlatform
}