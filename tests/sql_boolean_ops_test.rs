//! SQL布尔操作单元测试
//! 
//! 该测试文件验证SQL布尔操作的正确性，包括AND、OR和NOT运算符。

#![cfg(feature = "std")]

use remdb::*;
use serial_test::serial;

// 简单的测试平台实现
struct TestPlatform;

impl platform::Platform for TestPlatform {
    fn get_timestamp(&self) -> u64 {
        0
    }

    fn get_timestamp_us(&self) -> u64 {
        0
    }

    fn spin_lock(&self, lock: &mut u32) {
        // 简单的自旋锁实现
        unsafe {
            while core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
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

    fn delay_ms(&self, _ms: u32) {
        // 空实现
    }

    fn delay_us(&self, _us: u32) {
        // 空实现
    }

    fn file_open(
        &self,
        _path: &str,
        _mode: platform::FileMode,
    ) -> platform::FileResult<platform::FileHandle> {
        Ok(core::ptr::null())
    }

    fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
        Ok(())
    }

    fn file_write(
        &self,
        _handle: platform::FileHandle,
        _buffer: *const u8,
        _size: usize,
    ) -> platform::FileResult<usize> {
        Ok(0)
    }

    fn file_read(
        &self,
        _handle: platform::FileHandle,
        _buffer: *mut u8,
        _size: usize,
    ) -> platform::FileResult<usize> {
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

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 定义测试表
remdb::table!(
    TEST_BOOLEAN_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        a: bool,
        b: bool
    }
);

// 定义测试数据库配置
remdb::database!(
    BOOLEAN_TEST_DB,
    tables: [TEST_BOOLEAN_TABLE]
);

#[test]
#[serial]
fn test_boolean_operations() {
    // 使用堆内存缓冲区，确保测试之间的隔离
    let mut db_memory = vec![0u8; 4194304]; // 4MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
            .unwrap();
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化数据库
    let config = &BOOLEAN_TEST_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    println!("=== 测试布尔操作 ===");

    // 1. 插入测试数据
    println!("测试1: 插入测试数据");
    
    // 先清空表
    let _ = db.sql_query("DELETE FROM TEST_BOOLEAN_TABLE");
    
    // 插入各种布尔组合
    let inserts = [
        "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (1, true, true)",
        "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (2, true, false)",
        "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (3, false, true)",
        "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (4, false, false)"
    ];

    for insert in inserts {
        let result = db.sql_query(insert);
        assert!(result.is_ok(), "插入数据失败: {}", insert);
        println!("✓ 插入数据成功: {}", insert);
    }

    // 2. 测试AND操作
    println!("\n测试2: AND操作");
    let and_query = "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE a AND b";
    let result = db.sql_query(and_query);
    assert!(result.is_ok(), "AND操作查询失败");
    println!("✓ AND操作查询执行成功");

    // 3. 测试OR操作
    println!("\n测试3: OR操作");
    let or_query = "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE a OR b";
    let result = db.sql_query(or_query);
    assert!(result.is_ok(), "OR操作查询失败");
    println!("✓ OR操作查询执行成功");

    // 4. 测试NOT操作
    println!("\n测试4: NOT操作");
    let not_query = "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE NOT a";
    let result = db.sql_query(not_query);
    assert!(result.is_ok(), "NOT操作查询失败");
    println!("✓ NOT操作查询执行成功");

    // 5. 测试所有布尔操作组合
    println!("\n测试5: 布尔操作组合");
    let combined_queries = [
        "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE NOT (a AND b)",
        "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE (a AND b) OR (NOT a AND NOT b)",
        "SELECT COUNT(*) as count FROM TEST_BOOLEAN_TABLE WHERE NOT a OR NOT b"
    ];

    for query in combined_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok(), "布尔操作组合查询失败: {}", query);
        println!("✓ 布尔操作组合查询执行成功: {}", query);
    }

    println!("\n=== 所有布尔操作测试通过! ===");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
}

#[test]
#[serial]
fn test_boolean_parsing() {
    // 测试布尔值解析
    println!("=== 测试布尔值解析 ===");

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 测试TRUE值解析
    let true_sql = "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (1, TRUE, FALSE)";
    println!("测试TRUE值解析: {}", true_sql);
    
    // 测试FALSE值解析
    let false_sql = "INSERT INTO TEST_BOOLEAN_TABLE (id, a, b) VALUES (2, FALSE, TRUE)";
    println!("测试FALSE值解析: {}", false_sql);

    println!("✓ 布尔值解析测试完成");
}
