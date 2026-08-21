#![allow(unsafe_code)]
extern crate alloc;

use remdb::{database, table, Result};
use remdb::types::RecordHeader;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 262144] = [0u8; 262144]; // 256KB 内存缓冲区

// 定义测试表
table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: u64,
        name: str(20),
        value: u32
    }
);

// 定义数据库
database!(
    TEST_DB,
    tables: [TEST_TABLE]
);



fn main() -> Result<()> 
{
    // 初始化平台
    // 使用一个简单的平台实现
    struct SimplePlatform;
    
    impl remdb::platform::Platform for SimplePlatform {
        fn get_timestamp(&self) -> u64 {
            0
        }
        
        fn get_timestamp_us(&self) -> u64 {
            0
        }
        
        fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
            let len = dest.len().min(src.len());
            dest[..len].copy_from_slice(&src[..len]);
        }
        
        fn memset(&self, dest: &mut [u8], value: u8) {
            dest.fill(value);
        }
        
        fn delay_ms(&self, ms: u32) {
            // 简单的忙等待延迟
            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < ms as u128 {
                core::hint::spin_loop();
            }
        }
        
        fn delay_us(&self, us: u32) {
            // 简单的忙等待延迟
            let start = std::time::Instant::now();
            while start.elapsed().as_micros() < us as u128 {
                core::hint::spin_loop();
            }
        }
        
        fn file_open(&self, path: &str, mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
            // 使用std::fs::File实现文件操作
            use std::fs::OpenOptions;
            
            let file = match mode {
                remdb::platform::FileMode::Read => OpenOptions::new().read(true).open(path),
                remdb::platform::FileMode::Write => OpenOptions::new().write(true).create(true).truncate(true).open(path),
                remdb::platform::FileMode::ReadWrite => OpenOptions::new().read(true).write(true).create(true).open(path),
                remdb::platform::FileMode::Append => OpenOptions::new().append(true).create(true).open(path),
            };
            
            match file {
                Ok(file) => {
                    let file_ptr = Box::into_raw(Box::new(file)) as remdb::platform::FileHandle;
                    Ok(file_ptr)
                },
                Err(_) => Err(()),
            }
        }
        
        fn file_close(&self, handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
            // 使用std::fs::File实现文件关闭
            let _file = unsafe { Box::from_raw(handle as *mut std::fs::File) };
            Ok(())
        }
        
        fn file_write(&self, handle: remdb::platform::FileHandle, buf: &[u8]) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件写入
            use std::io::Write;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            
            match file.write(buf) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }
        
        fn file_read(&self, handle: remdb::platform::FileHandle, buf: &mut [u8]) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件读取
            use std::io::Read;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            
            match file.read(buf) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }
        
        fn file_seek(&self, handle: remdb::platform::FileHandle, offset: i64, whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
            // 使用std::fs::File实现文件定位
            use std::io::Seek;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let seek_from = match whence {
                remdb::platform::SeekWhence::SeekSet => std::io::SeekFrom::Start(offset as u64),
                remdb::platform::SeekWhence::SeekCur => std::io::SeekFrom::Current(offset),
                remdb::platform::SeekWhence::SeekEnd => std::io::SeekFrom::End(offset),
            };
            
            match file.seek(seek_from) {
                Ok(pos) => Ok(pos),
                Err(_) => Err(()),
            }
        }
        
        fn file_remove(&self, path: &str) -> remdb::platform::FileResult<()> {
            // 使用std::fs::remove_file实现文件删除
            use std::fs::remove_file;
            
            match remove_file(path) {
                Ok(_) => Ok(()),
                Err(_) => Err(()),
            }
        }
        
        fn file_size(&self, path: &str) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::metadata实现文件大小获取
            use std::fs::metadata;
            
            match metadata(path) {
                Ok(meta) => Ok(meta.len() as usize),
                Err(_) => Err(()),
            }
        }
        
        fn crc32(&self, data: &[u8]) -> u32 {
            // 简单的XOR校验和实现
            let mut checksum = 0u32;
            for &byte in data {
                checksum ^= byte as u32;
            }
            checksum
        }
    }
    
    static SIMPLE_PLATFORM: SimplePlatform = SimplePlatform;
    
    unsafe {
        // 初始化内存分配器
        let _ = remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台
        remdb::platform::init_platform(&SIMPLE_PLATFORM);
        // 初始化数据库
        let db = remdb::init_global_db(&TEST_DB)?;
        
        // 插入测试数据
        println!("插入测试数据...");
        println!("TEST_TABLE.fields.len() = {}", TEST_TABLE.fields.len());
        for i in 0..10 {
            // 创建对齐的记录数据
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
            let mut record = AlignedRecord([0; 32]);
            
            // 获取表引用
            let table = db.get_table_mut(0)?;
            
            // 设置id字段为唯一值
            let id_value = remdb::Value::U64(i as u64);
            table.set_field(&mut record.0, 0, &id_value)?;
            
            let name = format!("item_{}", i);
            let name_value = remdb::Value::String({ 
                let mut s = [0u8; 64];
                // 填充name，剩余空间用0填充
                for (j, c) in name.as_bytes().iter().enumerate() {
                    if j < s.len() {
                        s[j] = *c;
                    } else {
                        break;
                    }
                }
                s
            } );
            table.set_field(&mut record.0, 1, &name_value)?;
            
            let value_value = remdb::Value::U32((i * 100) as u32);
            table.set_field(&mut record.0, 2, &value_value)?;
            
            // 插入记录
            let record_id = table.insert(&record.0)?;
            println!("插入记录ID: {}", record_id);
        }
        
        // 保存快照
        println!("保存快照到 snapshot.remd...");
        db.save_snapshot("snapshot.remd")?;
        println!("快照保存成功");
        
        Ok(())
    }
}