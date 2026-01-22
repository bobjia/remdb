extern crate alloc;

use remdb::types::RecordHeader;
use remdb::{database, table, Result};

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

fn main() -> Result<()> {
    // 初始化平台
    // 由于posix模块被特性门控，我们使用一个简单的平台实现
    struct SimplePlatform;

    impl remdb::platform::Platform for SimplePlatform {
        fn get_timestamp(&self) -> u64 {
            0
        }

        fn get_timestamp_us(&self) -> u64 {
            0
        }

        fn spin_lock(&self, lock: &mut u32) {
            // 简单的自旋锁实现
            while unsafe {
                core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                    .compare_exchange(
                        0,
                        1,
                        core::sync::atomic::Ordering::Acquire,
                        core::sync::atomic::Ordering::Relaxed,
                    )
                    .is_err()
            } {
                core::hint::spin_loop();
            }
        }

        fn spin_unlock(&self, lock: &mut u32) {
            unsafe {
                core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                    .store(0, core::sync::atomic::Ordering::Release);
            }
        }

        fn compiler_barrier(&self) {
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        }

        fn full_memory_barrier(&self) {
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }

        fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
            unsafe {
                core::ptr::copy_nonoverlapping(src, dest, size);
            }
        }

        fn memset(&self, dest: *mut u8, value: u8, size: usize) {
            unsafe {
                core::ptr::write_bytes(dest, value, size);
            }
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

        fn file_open(
            &self,
            path: &str,
            mode: remdb::platform::FileMode,
        ) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
            // 使用std::fs::File实现文件操作
            use std::fs::OpenOptions;

            let file = match mode {
                remdb::platform::FileMode::Read => OpenOptions::new().read(true).open(path),
                remdb::platform::FileMode::Write => OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path),
                remdb::platform::FileMode::ReadWrite => OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path),
                remdb::platform::FileMode::Append => {
                    OpenOptions::new().append(true).create(true).open(path)
                }
            };

            match file {
                Ok(file) => {
                    let file_ptr = Box::into_raw(Box::new(file)) as remdb::platform::FileHandle;
                    Ok(file_ptr)
                }
                Err(_) => Err(()),
            }
        }

        fn file_close(
            &self,
            handle: remdb::platform::FileHandle,
        ) -> remdb::platform::FileResult<()> {
            // 使用std::fs::File实现文件关闭
            let _file = unsafe { Box::from_raw(handle as *mut std::fs::File) };
            Ok(())
        }

        fn file_write(
            &self,
            handle: remdb::platform::FileHandle,
            buffer: *const u8,
            size: usize,
        ) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件写入
            use std::io::Write;

            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let slice = unsafe { std::slice::from_raw_parts(buffer, size) };

            match file.write(slice) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }

        fn file_read(
            &self,
            handle: remdb::platform::FileHandle,
            buffer: *mut u8,
            size: usize,
        ) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件读取
            use std::io::Read;

            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let slice = unsafe { std::slice::from_raw_parts_mut(buffer, size) };

            match file.read(slice) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }

        fn file_seek(
            &self,
            handle: remdb::platform::FileHandle,
            offset: i64,
            whence: remdb::platform::SeekWhence,
        ) -> remdb::platform::FileResult<u64> {
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

        fn crc32(&self, data: *const u8, size: usize) -> u32 {
            // 简单的XOR校验和实现
            let slice = unsafe { std::slice::from_raw_parts(data, size) };
            let mut checksum = 0u32;
            for &byte in slice {
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
            DB_MEMORY.len(),
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
            let id_value = remdb::Value { u64: i as u64 };
            table.set_field(record.0.as_mut_ptr(), 0, &id_value)?;

            let name = format!("item_{}", i);
            let name_value = remdb::Value {
                string: {
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
                },
            };
            table.set_field(record.0.as_mut_ptr(), 1, &name_value)?;

            let value_value = remdb::Value {
                u32: (i * 100) as u32,
            };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?;

            // 插入记录
            let record_id = table.insert(record.0.as_ptr())?;
            println!("插入记录ID: {}", record_id);
        }

        // 保存快照
        println!("保存快照到 snapshot.remd...");
        db.save_snapshot("snapshot.remd")?;
        println!("快照保存成功");

        Ok(())
    }
}
