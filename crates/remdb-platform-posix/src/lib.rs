// POSIX platform implementation for remdb
// This crate contains inherently unsafe POSIX syscall wrappers.
// #![allow(unsafe_code)] is required because POSIX file I/O and
// memory operations are inherently unsafe at the syscall level.
// SAFETY: Each unsafe block is documented with its safety invariants.
// Callers must ensure buffer validity and handle lifetimes correctly.
#![allow(unsafe_code)]

use core::ptr;
use std::io::{Read, Write};
use remdb::platform::Platform;

/// POSIX平台实现
pub struct PosixPlatform;

impl Platform for PosixPlatform {
    /// 获取当前时间戳（毫秒）
    fn get_timestamp(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
        
        now.as_millis() as u64
    }
    
    /// 获取当前时间戳（微秒）
    fn get_timestamp_us(&self) -> u64 {
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
    fn file_open(&self, path: &str, mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
        use std::fs::OpenOptions;
        
        let mut options = OpenOptions::new();
        
        match mode {
            remdb::platform::FileMode::Read => {
                options.read(true);
            },
            remdb::platform::FileMode::Write => {
                options.write(true).create(true).truncate(true);
            },
            remdb::platform::FileMode::ReadWrite => {
                options.read(true).write(true).create(true);
            },
            remdb::platform::FileMode::Append => {
                options.write(true).create(true).append(true);
            },
        }
        
        match options.open(path) {
            Ok(file) => {
                let boxed_file = Box::new(file);
                Ok(Box::into_raw(boxed_file) as remdb::platform::FileHandle)
            },
            Err(_) => Err(()),
        }
    }
    
    /// 关闭文件
    fn file_close(&self, handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
        unsafe {
            let _ = Box::from_raw(handle as *mut std::fs::File);
        }
        Ok(())
    }
    
    /// 写入文件
    fn file_write(&self, handle: remdb::platform::FileHandle, buf: &[u8]) -> remdb::platform::FileResult<usize> {
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            match file.write(buf) {
                Ok(bytes_written) => {
                    file.flush().map_err(|_| ())?;
                    Ok(bytes_written)
                },
                Err(_) => Err(()),
            }
        }
    }
    
    /// 读取文件
    fn file_read(&self, handle: remdb::platform::FileHandle, buf: &mut [u8]) -> remdb::platform::FileResult<usize> {
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            match file.read(buf) {
                Ok(bytes_read) => Ok(bytes_read),
                Err(_) => Err(()),
            }
        }
    }
    
    /// 文件定位
    fn file_seek(&self, handle: remdb::platform::FileHandle, offset: i64, whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
        use std::io::Seek;
        
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            
            let seek_from = match whence {
                remdb::platform::SeekWhence::SeekSet => std::io::SeekFrom::Start(offset as u64),
                remdb::platform::SeekWhence::SeekCur => std::io::SeekFrom::Current(offset),
                remdb::platform::SeekWhence::SeekEnd => std::io::SeekFrom::End(offset),
            };
            
            match file.seek(seek_from) {
                Ok(new_pos) => Ok(new_pos),
                Err(_) => Err(()),
            }
        }
    }
    
    /// 删除文件
    fn file_remove(&self, path: &str) -> remdb::platform::FileResult<()> {
        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
    
    /// 获取文件大小
    fn file_size(&self, path: &str) -> remdb::platform::FileResult<usize> {
        use std::fs::metadata;
        
        match metadata(path) {
            Ok(metadata) => Ok(metadata.len() as usize),
            Err(_) => Err(()),
        }
    }
    
    /// 计算CRC32校验和
    fn crc32(&self, data: &[u8]) -> u32 {
        const CRC32_POLY: u32 = 0xEDB88320;
        
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
        
        let mut crc = 0xFFFFFFFFu32;
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