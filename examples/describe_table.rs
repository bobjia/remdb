#![allow(unsafe_code)]
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
        memory::init_global_allocator(
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
            fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
                let len = dest.len().min(src.len());
                dest[..len].copy_from_slice(&src[..len]);
            }
            fn memset(&self, dest: &mut [u8], value: u8) {
                dest.fill(value);
            }
            fn delay_ms(&self, _ms: u32) {
            }
            fn delay_us(&self, _us: u32) {
            }
            fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
                // 返回一个有效的FileHandle
                Ok(1)
            }
            fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_write(&self, _handle: platform::FileHandle, _buf: &[u8]) -> platform::FileResult<usize> {
                // 模拟写入成功，返回写入的字节数
                Ok(_buf.len())
            }
            fn file_read(&self, _handle: platform::FileHandle, _buf: &mut [u8]) -> platform::FileResult<usize> {
                // 模拟读取成功，返回0表示文件为空
                Ok(0)
            }
            fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
                // 模拟seek成功，返回当前位置0
                Ok(0)
            }
            fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                Ok(0)
            }
            fn crc32(&self, _data: &[u8]) -> u32 {
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
        
        // 测试新专用方法
        println!("\n--- Testing New Dedicated Methods ---");
        
        // 使用insert_record插入记录
        println!("\n1. 使用insert_record插入记录:");
        let columns = &["id", "name", "age", "active"];
        let values = &["1", "John Doe", "30", "true"];
        match db.insert_record("TEST_TABLE", columns, values) {
            Ok(affected_rows) => {
                println!("✅ 插入记录成功，影响行数: {}", affected_rows);
            },
            Err(e) => {
                println!("❌ 插入记录失败: {:?}", e);
            }
        }
        
        // 使用execute_query查询记录
        println!("\n2. 使用execute_query查询记录:");
        match db.execute_query("TEST_TABLE", &["id", "name", "age", "active"], None, None) {
            Ok(result_set) => {
                println!("✅ 查询记录成功:");
                println!("{}", result_set.to_string());
            },
            Err(e) => {
                println!("❌ 查询记录失败: {:?}", e);
            }
        }
        
        // 使用update_record更新记录
        println!("\n3. 使用update_record更新记录:");
        match db.update_record("TEST_TABLE", "age = 31, active = false", Some("id = 1")) {
            Ok(affected_rows) => {
                println!("✅ 更新记录成功，影响行数: {}", affected_rows);
                
                // 查询验证
                if let Ok(result_set) = db.execute_query("TEST_TABLE", &["id", "name", "age", "active"], None, None) {
                    println!("✅ 更新后查询结果:");
                    println!("{}", result_set.to_string());
                }
            },
            Err(e) => {
                println!("❌ 更新记录失败: {:?}", e);
            }
        }
        
        // 使用delete_record删除记录
        println!("\n4. 使用delete_record删除记录:");
        match db.delete_record("TEST_TABLE", Some("id = 1")) {
            Ok(affected_rows) => {
                println!("✅ 删除记录成功，影响行数: {}", affected_rows);
                
                // 查询验证
                if let Ok(result_set) = db.execute_query("TEST_TABLE", &["id", "name", "age", "active"], None, None) {
                    println!("✅ 删除后查询结果:");
                    println!("{}", result_set.to_string());
                }
            },
            Err(e) => {
                println!("❌ 删除记录失败: {:?}", e);
            }
        }
    }
}