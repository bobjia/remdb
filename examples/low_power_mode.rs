// 低功耗模式示例
extern crate alloc;

use remdb::types::RecordHeader;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 262144] = [0u8; 262144]; // 256KB 内存缓冲区

// 定义表结构
remdb::table!(
    TEST_TABLE,
    100, // 减小最大记录数到100，降低内存需求
    primary_key: id,
    fields: {
        id: i32,
        name: str(32),
        value: f64,
        timestamp: u64
    }
);

// 定义数据库配置，支持低功耗模式
remdb::database!(
    TEST_DB,
    tables: [
        TEST_TABLE
    ],
    low_power: true,
    low_power_max_records: 100
);

// 定义测试数据结构
#[derive(Clone, Copy)]
#[repr(C)]
struct TestRecord {
    id: i32,
    name: [u8; 32],
    value: f64,
    timestamp: u64,
}

// 手动计算表所需的总内存大小
// 记录大小：id(4字节) + name(32字节) + value(8字节) + timestamp(8字节) = 52字节
const RECORD_SIZE: usize = 4 + 32 + 8 + 8;
const TABLE_DATA_SIZE: usize = RECORD_SIZE * TEST_TABLE.max_records;
const STATUS_ARRAY_SIZE: usize = core::mem::size_of::<RecordHeader>() * TEST_TABLE.max_records;
const FREE_SLOTS_SIZE: usize = core::mem::size_of::<usize>() * TEST_TABLE.max_records;
const TABLE_MEM_SIZE: usize = TABLE_DATA_SIZE + STATUS_ARRAY_SIZE + FREE_SLOTS_SIZE;

fn main() {
    unsafe {
        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台
        #[cfg(feature = "posix")]
        remdb::platform::init_platform(remdb::platform::posix::get_posix_platform());

        #[cfg(not(feature = "posix"))]
        {
            // 使用一个简单的平台实现，只提供必要的方法
            struct MinimalPlatform;
            impl remdb::platform::Platform for MinimalPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {
                    // 简单的自旋锁实现
                    unsafe {
                        while core::sync::atomic::AtomicU32::from_ptr(_lock as *mut u32)
                            .compare_exchange(
                                0,
                                1,
                                core::sync::atomic::Ordering::Acquire,
                                core::sync::atomic::Ordering::Relaxed,
                            )
                            .is_err()
                        {
                            core::hint::spin_loop();
                        }
                    }
                }
                fn spin_unlock(&self, _lock: &mut u32) {
                    unsafe {
                        core::sync::atomic::AtomicU32::from_ptr(_lock as *mut u32)
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
                fn memset(&self, dest: *mut u8, val: u8, size: usize) {
                    unsafe {
                        core::ptr::write_bytes(dest, val, size);
                    }
                }
                fn delay_ms(&self, _ms: u32) {
                    // 空实现
                }
                fn delay_us(&self, _us: u32) {
                    // 空实现
                }
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: remdb::platform::FileMode,
                ) -> std::result::Result<*const u8, ()> {
                    Err(())
                }
                fn file_close(&self, _handle: *const u8) -> std::result::Result<(), ()> {
                    Ok(())
                }
                fn file_write(
                    &self,
                    _handle: *const u8,
                    _buf: *const u8,
                    _size: usize,
                ) -> std::result::Result<usize, ()> {
                    Ok(0)
                }
                fn file_read(
                    &self,
                    _handle: *const u8,
                    _buf: *mut u8,
                    _size: usize,
                ) -> std::result::Result<usize, ()> {
                    Ok(0)
                }
                fn file_seek(
                    &self,
                    _handle: *const u8,
                    _offset: i64,
                    _whence: remdb::platform::SeekWhence,
                ) -> std::result::Result<u64, ()> {
                    Ok(0)
                }
                fn file_remove(&self, _path: &str) -> std::result::Result<(), ()> {
                    Ok(())
                }
                fn file_size(&self, _path: &str) -> std::result::Result<usize, ()> {
                    Ok(0)
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static MINIMAL_PLATFORM: MinimalPlatform = MinimalPlatform;
            remdb::platform::init_platform(&MINIMAL_PLATFORM);
        }

        // 初始化数据库
        let db = remdb::init_global_db(&TEST_DB).unwrap();

        println!(
            "数据库初始化成功，支持低功耗模式: {}",
            TEST_DB.low_power_mode_supported
        );
        println!(
            "低功耗模式下的最大记录数: {:?}",
            TEST_DB.low_power_max_records
        );

        // 插入测试数据
        let mut records = [TestRecord {
            id: 0,
            name: [b'a'; 32],
            value: 0.0,
            timestamp: 0,
        }; 50];

        for i in 0..50 {
            records[i].id = i as i32;
            records[i].value = i as f64;
            records[i].timestamp = i as u64;
        }

        // 进入低功耗模式
        println!("进入低功耗模式...");
        db.enter_low_power_mode().unwrap();
        println!("当前低功耗模式状态: {}", db.is_low_power_mode());

        // 插入记录（正常情况）
        println!("开始插入50条记录...");
        for i in 0..50 {
            match db
                .get_table_mut(0)
                .unwrap()
                .insert(&records[i] as *const TestRecord as *const u8)
            {
                Ok(id) => println!("插入成功，记录ID: {}", id),
                Err(e) => println!("插入失败，错误: {:?}", e),
            }
        }

        println!("当前记录数: {}", db.get_table(0).unwrap().record_count());

        // 退出低功耗模式
        println!("退出低功耗模式...");
        db.exit_low_power_mode().unwrap();
        println!("当前低功耗模式状态: {}", db.is_low_power_mode());

        // 测试新的专用方法
        println!("\n=== 测试新的专用方法 ===");

        // 1. 使用insert_record插入记录（正常模式）
        println!("\n1. 在正常模式下使用insert_record插入记录:");
        let columns = &["id", "name", "value", "timestamp"];
        let values = &["51", "test_insert", "123.45", "1234567890"];
        let affected_rows = db.insert_record("TEST_TABLE", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);

        // 2. 使用execute_query查询记录
        println!("\n2. 使用execute_query查询记录:");
        let result = db
            .execute_query(
                "TEST_TABLE",
                &["id", "name", "value", "timestamp"],
                Some("id = 51"),
                None,
            )
            .unwrap();
        println!("查询结果: {}", result.to_string());

        // 3. 再次进入低功耗模式，测试新方法
        println!("\n3. 再次进入低功耗模式，测试新方法:");
        db.enter_low_power_mode().unwrap();
        println!("当前低功耗模式状态: {}", db.is_low_power_mode());

        // 4. 在低功耗模式下使用update_record更新记录
        println!("\n4. 在低功耗模式下使用update_record更新记录:");
        let update_affected = db
            .update_record(
                "TEST_TABLE",
                "value = 543.21, timestamp = 9876543210",
                Some("id = 51"),
            )
            .unwrap();
        println!("更新记录成功，影响行数: {}", update_affected);

        // 查询验证更新
        let updated_result = db
            .execute_query(
                "TEST_TABLE",
                &["id", "name", "value", "timestamp"],
                Some("id = 51"),
                None,
            )
            .unwrap();
        println!("更新后查询结果: {}", updated_result.to_string());

        // 5. 在低功耗模式下使用delete_record删除记录
        println!("\n5. 在低功耗模式下使用delete_record删除记录:");
        let delete_affected = db.delete_record("TEST_TABLE", Some("id = 51")).unwrap();
        println!("删除记录成功，影响行数: {}", delete_affected);

        // 查询验证删除
        let delete_result = db
            .execute_query(
                "TEST_TABLE",
                &["id", "name", "value", "timestamp"],
                Some("id = 51"),
                None,
            )
            .unwrap();
        println!("删除后查询结果: {}", delete_result.to_string());

        // 退出低功耗模式
        db.exit_low_power_mode().unwrap();
        println!("当前低功耗模式状态: {}", db.is_low_power_mode());

        println!("示例程序执行完成");
        println!("所有测试通过!");
    }
}
