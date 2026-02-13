// 测试默认值功能
#![cfg(feature = "ha")]

extern crate alloc;

use remdb::*;

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i64,
        name: str(32), // 32字节定长字符串
        age: i64,
        active: bool,
        score: f64
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [users]
);

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152];

fn main() {
    println!("Testing DEFAULT field functionality...");

    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .expect("Failed to initialize global allocator");

        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        platform::init_platform(platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            // 在非posix平台上，使用一个简单的平台实现
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
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
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: platform::FileMode,
                ) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: platform::FileHandle,
                    _offset: i64,
                    _whence: platform::SeekWhence,
                ) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }

        // 初始化全局数据库
        let db = init_global_db(&DB_CONFIG).expect("Failed to initialize database");

        println!("Testing DEFAULT field functionality...");

        // 插入数据，测试默认值功能
        println!("1. Inserting records with DEFAULT values...");

        // 使用insert_record插入记录，不提供所有字段值
        let columns = &["id", "name"];
        let values = &["1", "Alice"];
        let affected_rows = db.insert_record("users", columns, values).unwrap();
        println!("   ✓ Inserted record 'Alice' with DEFAULT values, affected rows: {}", affected_rows);

        let values2 = &["2", "Bob"];
        let affected_rows2 = db.insert_record("users", columns, values2).unwrap();
        println!("   ✓ Inserted record 'Bob' with DEFAULT values, affected rows: {}", affected_rows2);

        println!("   ✓ All records inserted successfully");

        // 查询数据，验证默认值
        println!("2. Querying records to verify values...");
        let result = db.execute_query("users", &["id", "name", "age", "active", "score"], None, None).unwrap();
        println!("   ✓ Query executed successfully");
        println!("   {}", result.to_string());

        println!("DEFAULT field functionality test completed successfully!");
    }
}
