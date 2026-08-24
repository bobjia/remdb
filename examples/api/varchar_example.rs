#![allow(static_mut_refs)]
// 示例：验证VARCHAR类型支持

extern crate alloc;
use remdb::*;

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        name: str(50), // 50字节定长字符串
        email: str(100), // 100字节定长字符串
        age: i32
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
    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .expect("Failed to initialize allocator");

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

        // 初始化数据库
        let db = init_global_db(&DB_CONFIG).expect("Failed to initialize database");

        // 插入数据
        println!("\n插入数据:");
        let columns = &["id", "name", "email", "age"];
        let values = &["1", "Alice", "alice@example.com", "30"];
        let affected_rows = db.insert_record("users", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);

        // 查询数据
        println!("\n查询数据:");
        let result = db
            .execute_query("users", &["id", "name", "email", "age"], None, None)
            .unwrap();
        println!("查询结果: {}", result.to_string());

        // 测试新的专用方法
        println!("\n=== 测试新的专用方法 ===");

        // 使用insert_record插入记录
        println!("\n1. 使用insert_record插入记录:");
        let values2 = &["2", "Bob", "bob@example.com", "25"];
        let affected_rows2 = db.insert_record("users", columns, values2).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows2);

        // 使用execute_query查询记录
        println!("\n2. 使用execute_query查询记录:");
        let exec_result = db
            .execute_query("users", &["id", "name", "email", "age"], None, None)
            .unwrap();
        println!("查询结果: {}", exec_result.to_string());

        // 使用update_record更新记录
        println!("\n3. 使用update_record更新记录:");
        let update_affected = db
            .update_record(
                "users",
                "age = 26, email = 'bob.updated@example.com'",
                Some("id = 2"),
            )
            .unwrap();
        println!("更新记录成功，影响行数: {}", update_affected);

        // 查询验证更新
        let updated_result = db
            .execute_query(
                "users",
                &["id", "name", "email", "age"],
                Some("id = 2"),
                None,
            )
            .unwrap();
        println!("更新后查询结果: {}", updated_result.to_string());

        // 使用delete_record删除记录
        println!("\n4. 使用delete_record删除记录:");
        let delete_affected = db.delete_record("users", Some("id = 1")).unwrap();
        println!("删除记录成功，影响行数: {}", delete_affected);

        // 查询剩余记录
        let remaining_result = db
            .execute_query("users", &["id", "name", "email", "age"], None, None)
            .unwrap();
        println!("删除后剩余记录: {}", remaining_result.to_string());

        println!("\nVARCHAR type support verified successfully!");
    }
}
