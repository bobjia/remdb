extern crate alloc;

use core::ptr::NonNull;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// 定义表结构
remdb::table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        name: str(32), // 32字节定长字符串
        age: i8,
        active: bool
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [TEST_TABLE]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;
        
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        // 使用一个简单的平台实现
        struct DummyPlatform;
        impl platform::Platform for DummyPlatform {
            fn get_timestamp(&self) -> u64 {
                0
            }
            fn get_timestamp_us(&self) -> u64 {
                0
            }
            fn spin_lock(&self, _lock: &mut u32) {
            }
            fn spin_unlock(&self, _lock: &mut u32) {
            }
            fn compiler_barrier(&self) {
            }
            fn full_memory_barrier(&self) {
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
            fn delay_ms(&self, _ms: u32) {
            }
            fn delay_us(&self, _us: u32) {
            }
            fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
                Err(())
            }
            fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                Err(())
            }
            fn file_write(&self, _handle: platform::FileHandle, _buffer: *const u8, _size: usize) -> platform::FileResult<usize> {
                Err(())
            }
            fn file_read(&self, _handle: platform::FileHandle, _buffer: *mut u8, _size: usize) -> platform::FileResult<usize> {
                Err(())
            }
            fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
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
        
        // 初始化全局数据库
        let db = init_global_db(config).unwrap();
        
        println!("Database initialized successfully.");
        println!("\n--- Testing DESCRIBE TABLE ---");
        
        // 测试 DESCRIBE TABLE 命令
        let result = db.sql_query("DESCRIBE TABLE TEST_TABLE");
        match result {
            Ok(result_set) => {
                println!("\nDESCRIBE TABLE TEST_TABLE result:");
                println!("{}", result_set.to_string());
            },
            Err(e) => {
                println!("Error executing DESCRIBE TABLE: {:?}", e);
            }
        }
        
        // 测试简写形式 DESCRIBE users
        println!("\n--- Testing DESCRIBE TEST_TABLE (short form) ---");
        let result = db.sql_query("DESCRIBE TEST_TABLE");
        match result {
            Ok(result_set) => {
                println!("\nDESCRIBE TEST_TABLE result:");
                println!("{}", result_set.to_string());
            },
            Err(e) => {
                println!("Error executing DESCRIBE TEST_TABLE: {:?}", e);
            }
        }
    }
}