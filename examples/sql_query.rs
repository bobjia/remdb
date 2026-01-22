//! SQL查询示例
//!
//! 该示例展示了如何使用remdb的SQL查询功能。

// 引入alloc模块
extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use core::alloc::Layout;
use core::ptr::NonNull;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32), // 32字节定长字符串
        age: i8,
        active: bool,
        created_at: u64
    }
);

// 定义数据库配置
remdb::database!(
    TEST_DB,
    tables: [users]
);

fn main() {
    unsafe {
        // 获取数据库配置
        let config = &TEST_DB;

        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台抽象层
        // 使用一个简单的平台实现，所有文件操作都返回成功
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
                // 返回一个非空指针作为有效的FileHandle
                Ok(1 as *const u8)
            }
            fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_write(
                &self,
                _handle: platform::FileHandle,
                _buffer: *const u8,
                size: usize,
            ) -> platform::FileResult<usize> {
                Ok(size)
            }
            fn file_read(
                &self,
                _handle: platform::FileHandle,
                _buffer: *mut u8,
                _size: usize,
            ) -> platform::FileResult<usize> {
                // 对于读取操作，返回0表示文件为空，这样会创建新的日志头
                Ok(0)
            }
            fn file_seek(
                &self,
                _handle: platform::FileHandle,
                _offset: i64,
                _whence: platform::SeekWhence,
            ) -> platform::FileResult<u64> {
                Ok(0)
            }
            fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                Ok(0)
            }
            fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                0
            }
        }
        static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
        platform::init_platform(&DUMMY_PLATFORM);

        // 初始化全局数据库
        let db = init_global_db(config).unwrap();

        // 插入测试数据
        #[repr(C)]
        struct UserRecord {
            id: i32,
            name: [u8; 32],
            age: i8,
            active: bool,
            created_at: u64,
        }

        // 准备测试数据
        let test_users = [
            (1, "Alice", 25, true, 1620000000000),
            (2, "Bob", 30, true, 1620000001000),
            (3, "Charlie", 35, false, 1620000002000),
            (4, "David", 22, true, 1620000003000),
            (5, "Eve", 28, false, 1620000004000),
        ];

        for (id, name, age, active, created_at) in test_users {
            // 构建记录数据
            let mut record = UserRecord {
                id,
                name: [0u8; 32],
                age,
                active,
                created_at,
            };

            // 复制名字到记录
            let name_bytes = name.as_bytes();
            record.name[..name_bytes.len()].copy_from_slice(name_bytes);

            // 插入记录
            let insert_id = db
                .get_table_mut(0)
                .unwrap()
                .insert(&record as *const _ as *const u8)
                .unwrap();
            println!("插入用户 {} 成功，记录ID: {}", name, insert_id);
        }

        // 执行SQL查询
        println!("\n=== SQL查询示例 ===");

        // 1. 查询所有用户
        println!("\n1. 查询所有用户:");
        let result = db.sql_query("SELECT * FROM users").unwrap();
        println!("{}", result.to_string());

        // 2. 查询指定列
        println!("\n2. 查询指定列:");
        let result = db.sql_query("SELECT name, age FROM users").unwrap();
        println!("{}", result.to_string());

        // 3. 查询带条件
        println!("\n3. 查询带条件 (age > 25):");
        let result = db.sql_query("SELECT * FROM users WHERE age > 25").unwrap();
        println!("{}", result.to_string());

        // 4. 查询带条件和排序
        println!("\n4. 查询带条件和排序 (active = true, 按年龄降序):");
        let result = db
            .sql_query("SELECT * FROM users WHERE active = true ORDER BY age DESC")
            .unwrap();
        println!("{}", result.to_string());

        // 5. 查询带LIMIT
        println!("\n5. 查询带LIMIT (前2条记录):");
        let result = db
            .sql_query("SELECT * FROM users ORDER BY id ASC LIMIT 2")
            .unwrap();
        println!("{}", result.to_string());

        // 6. 使用迭代器访问结果
        println!("\n6. 使用迭代器访问结果:");
        let result = db
            .sql_query("SELECT name, active FROM users WHERE age < 30")
            .unwrap();
        for row in result.iter() {
            // 从结果行中获取字段值
            let name = row.get(0).unwrap();
            let active = row.get(1).unwrap();

            // 转换为合适的类型
            let name_str = String::from_utf8_lossy(&name.value.string)
                .trim_end_matches(char::from(0))
                .to_string();
            let active_val = active.value.bool;

            println!("用户名: {}, 活跃: {}", name_str, active_val);
        }

        // 7. 执行SQL INSERT语句
        println!("\n=== SQL INSERT示例 ===");

        // 7.1 插入一条新记录
        println!("\n7.1 插入一条新记录:");
        let result = db
            .sql_query("INSERT INTO users VALUES (6, 'Frank', 33, true, 1620000005000)")
            .unwrap();
        println!("插入结果: {}", result.to_string());

        // 7.2 插入多条记录
        println!("\n7.2 插入多条记录:");
        let result = db.sql_query("INSERT INTO users VALUES (7, 'Grace', 27, true, 1620000006000), (8, 'Henry', 31, false, 1620000007000)").unwrap();
        println!("插入结果: {}", result.to_string());

        // 7.3 插入指定列
        println!("\n7.3 插入指定列:");
        let result = db
            .sql_query("INSERT INTO users (id, name, age, active) VALUES (9, 'Ivy', 29, true)")
            .unwrap();
        println!("插入结果: {}", result.to_string());

        // 7.4 验证插入结果
        println!("\n7.4 验证插入结果:");
        let result = db.sql_query("SELECT * FROM users ORDER BY id ASC").unwrap();
        println!("{}", result.to_string());

        // 8. 执行SQL DELETE语句
        println!("\n=== SQL DELETE示例 ===");

        // 8.1 删除符合条件的记录
        println!("\n8.1 删除符合条件的记录 (age > 30):");
        let result = db.sql_query("DELETE FROM users WHERE age > 30").unwrap();
        println!("删除结果: {}", result.to_string());

        // 8.2 验证删除结果
        println!("\n8.2 验证删除结果:");
        let result = db.sql_query("SELECT * FROM users ORDER BY id ASC").unwrap();
        println!("{}", result.to_string());

        // 8.3 删除所有记录
        println!("\n8.3 删除所有记录:");
        let result = db.sql_query("DELETE FROM users").unwrap();
        println!("删除结果: {}", result.to_string());

        // 8.4 验证所有记录已删除
        println!("\n8.4 验证所有记录已删除:");
        let result = db.sql_query("SELECT * FROM users").unwrap();
        println!("{}", result.to_string());

        // 9. 使用新的专用方法示例
        println!("\n=== 新专用方法示例 ===");

        // 9.1 使用insert_record插入记录
        println!("\n9.1 使用insert_record插入记录:");
        let columns = &["id", "name", "age", "active", "created_at"];
        let values = &["1", "Alice", "25", "true", "1620000000000"];
        let affected_rows = db.insert_record("users", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);

        // 9.2 使用insert_record插入多条记录
        println!("\n9.2 使用insert_record插入多条记录:");
        let values2 = &["2", "Bob", "30", "true", "1620000001000"];
        let values3 = &["3", "Charlie", "35", "false", "1620000002000"];
        let affected_rows2 = db.insert_record("users", columns, values2).unwrap();
        let affected_rows3 = db.insert_record("users", columns, values3).unwrap();
        println!(
            "插入记录成功，影响行数: {} 和 {}",
            affected_rows2, affected_rows3
        );

        // 9.3 使用execute_query查询记录
        println!("\n9.3 使用execute_query查询记录:");
        let result = db
            .execute_query("users", &["name", "age"], Some("age > 25"), None)
            .unwrap();
        println!("{}", result.to_string());

        // 9.4 使用update_record更新记录
        println!("\n9.4 使用update_record更新记录:");
        let affected_rows = db
            .update_record("users", "age = 31", Some("name = 'Bob'"))
            .unwrap();
        println!("更新记录成功，影响行数: {}", affected_rows);

        // 9.5 使用delete_record删除记录
        println!("\n9.5 使用delete_record删除记录:");
        let affected_rows = db.delete_record("users", Some("id = 3")).unwrap();
        println!("删除记录成功，影响行数: {}", affected_rows);

        // 9.6 使用execute_query查询剩余记录
        println!("\n9.6 使用execute_query查询剩余记录:");
        let result = db.execute_query("users", &["*"], None, None).unwrap();
        println!("{}", result.to_string());

        println!("\n=== SQL示例完成 ===");
    }
}
