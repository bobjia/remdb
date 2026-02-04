extern crate alloc;

use remdb::*;

// 定义内存缓冲区（增大到2MB以容纳系统表和用户表）
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152];

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
    DB_CONFIG,
    tables: [users]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;

        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

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
        let db = init_global_db(config).unwrap();

        // 创建测试记录
        let mut record_data = [0u8; 44]; // 计算记录大小：i32(4) + str(32) + i8(1) + bool(1) + u64(8) = 46字节（对齐到8字节）

        // 设置字段值
        let id: i32 = 1;
        let name = "test_user";
        let age: i8 = 30;
        let active = true;
        let created_at: u64 = 1234567890;

        // 手动填充记录数据（实际应用中应该使用更安全的方式）
        core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);

        core::ptr::copy_nonoverlapping(name.as_ptr(), record_data.as_mut_ptr().add(4), name.len());

        core::ptr::write(record_data.as_mut_ptr().add(36) as *mut i8, age);
        core::ptr::write(record_data.as_mut_ptr().add(37) as *mut bool, active);
        core::ptr::copy_nonoverlapping(
            &created_at as *const u64 as *const u8,
            record_data.as_mut_ptr().add(40),
            8,
        );

        // 插入记录
        let table_mut = db.get_table_mut(0).unwrap();
        let record_id = table_mut.insert(record_data.as_ptr()).unwrap();

        println!("Inserted record with ID: {}", record_id);

        // 获取记录
        let mut result_data = [0u8; 44];
        table_mut
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();

        // 读取字段值（简化示例）
        let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
        let result_name = core::str::from_utf8(&result_data[4..36])
            .unwrap()
            .trim_end_matches(char::from(0));
        let result_age = core::ptr::read(result_data.as_ptr().add(36) as *const i8);
        let result_active = core::ptr::read(result_data.as_ptr().add(37) as *const bool);
        let result_created_at = core::ptr::read(result_data.as_ptr().add(40) as *const u64);

        println!(
            "Retrieved record: ID={}, Name={}, Age={}, Active={}, CreatedAt={}",
            result_id, result_name, result_age, result_active, result_created_at
        );

        // 删除记录
        table_mut.delete(record_id).unwrap();
        println!("Deleted record with ID: {}", record_id);

        // 测试事务
        // 创建事务缓冲区（使用默认初始化）
        let mut tx_buffer = transaction::Transaction::default();

        let mut log_buffer = [transaction::LogItem::default(); 10];

        let tx = transaction::begin(
            transaction::TransactionType::ReadWrite,
            transaction::IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10,
        )
        .unwrap();

        println!("Started transaction with ID: {}", tx.as_ref().id);

        // 在事务中插入记录
        let tx_record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        println!("Inserted record in transaction with ID: {}", tx_record_id);

        // 提交事务
        transaction::commit().unwrap();
        println!("Committed transaction");

        // 删除记录
        table_mut.delete(tx_record_id).unwrap();

        // 新专用方法示例
        println!("\n=== 新专用方法示例 ===");

        // 使用insert_record插入记录
        println!("\n使用insert_record插入记录:");
        let columns = &["id", "name", "age", "active", "created_at"];
        let values = &["2", "new_user", "25", "true", "1234567890"];
        let affected_rows = db.insert_record("users", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);

        // 使用execute_query查询记录
        println!("\n使用execute_query查询记录:");
        let result = db
            .execute_query("users", &["id", "name", "age"], None, None)
            .unwrap();
        println!("查询结果: {}", result.to_string());

        // 使用update_record更新记录
        println!("\n使用update_record更新记录:");
        let affected_rows = db
            .update_record("users", "age = 26", Some("name = 'new_user'"))
            .unwrap();
        println!("更新记录成功，影响行数: {}", affected_rows);

        // 使用execute_query查询更新后的记录
        let result = db
            .execute_query("users", &["id", "name", "age"], None, None)
            .unwrap();
        println!("更新后查询结果: {}", result.to_string());

        // 使用delete_record删除记录
        println!("\n使用delete_record删除记录:");
        let affected_rows = db.delete_record("users", Some("id = 2")).unwrap();
        println!("删除记录成功，影响行数: {}", affected_rows);

        println!("\nBasic usage example completed successfully!");
    }
}
