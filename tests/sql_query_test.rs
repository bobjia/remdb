//! SQL查询单元测试
//! 
//! 该测试文件验证SQL查询功能的正确性。

#![cfg(feature = "std")]

use remdb::*;
use std::sync::Arc;

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
                .compare_exchange(0, 1, 
                                 core::sync::atomic::Ordering::Acquire,
                                 core::sync::atomic::Ordering::Relaxed)
                .is_err() {
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
    
    fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
        Ok(core::ptr::null())
    }
    
    fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
        Ok(())
    }
    
    fn file_write(&self, _handle: platform::FileHandle, _buffer: *const u8, _size: usize) -> platform::FileResult<usize> {
        Ok(0)
    }
    
    fn file_read(&self, _handle: platform::FileHandle, _buffer: *mut u8, _size: usize) -> platform::FileResult<usize> {
        Ok(0)
    }
    
    fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
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

// 定义测试表结构
remdb::table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32),
        age: i8,
        active: bool,
        created_at: u64
    }
);

// 定义测试数据库配置
remdb::database!(
    TEST_DB,
    tables: [TEST_TABLE]
);

#[cfg_attr(any(test, feature = "std"), test)]
fn test_sql_query() {
    // 使用静态内存缓冲区，确保它不会在函数返回时被释放
    static mut DB_MEMORY: [u8; 131072] = [0u8; 131072];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).unwrap();
    }
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化数据库
    let config = &TEST_DB;
    let db = unsafe {
        init_global_db(config).unwrap()
    };
    
    // 测试各种SQL语法
    println!("=== 测试各种SQL语法 ===");
    
    // 1. 测试有效SQL语法
    let valid_queries = [
        "SELECT * FROM TEST_TABLE",
        "SELECT name, age FROM TEST_TABLE",
        "SELECT * FROM TEST_TABLE WHERE id = 1",
        "SELECT * FROM TEST_TABLE WHERE age > 25 AND active = true",
        "SELECT * FROM TEST_TABLE ORDER BY name ASC",
        "SELECT * FROM TEST_TABLE ORDER BY age DESC",
        "SELECT * FROM TEST_TABLE LIMIT 5",
        "SELECT * FROM TEST_TABLE WHERE active = false LIMIT 2",
    ];
    
    for query in valid_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok(), "查询 '{}' 应该成功执行", query);
    }
    
    // 2. 测试无效SQL语法
    let invalid_queries = [
        "SELECT", // 缺少FROM子句
        "SELECT *", // 缺少FROM子句
        "SELECT * FROM", // 缺少表名
        "SELECT * FROM WHERE id = 1", // 缺少表名
        "SELECT * FROM TEST_TABLE WHERE", // 缺少条件
        "SELECT * FROM TEST_TABLE WHERE id", // 缺少比较运算符和值
        "SELECT * FROM TEST_TABLE ORDER BY", // 缺少排序列
        "SELECT * FROM TEST_TABLE LIMIT", // 缺少LIMIT值
    ];
    
    for query in invalid_queries {
        let result = db.sql_query(query);
        assert!(result.is_err(), "查询 '{}' 应该失败", query);
    }

    println!("=== 插入测试数据和查询 ===");

    // 插入测试数据
    // 确保TestRecord的内存布局与table!宏生成的字段偏移量匹配
    // 使用精确的#[repr(C)]布局，确保字段顺序和大小与table!宏定义一致
    // 添加2字节填充，因为created_at需要8字节对齐
    #[repr(C)]
    struct TestRecord {
        id: i32,          // 4字节
        name: [u8; 32],   // 32字节
        age: i8,          // 1字节
        active: u8,       // 1字节（bool在C中通常是1字节）
        _padding: [u8; 2], // 2字节填充，确保created_at字段8字节对齐
        created_at: u64,  // 8字节
    }
    
    // 准备测试数据
    let test_data = [
        (1, "Alice", 25, true, 1620000000000),
        (2, "Bob", 30, true, 1620000001000),
        (3, "Charlie", 35, false, 1620000002000),
        (4, "David", 22, true, 1620000003000),
        (5, "Eve", 28, false, 1620000004000),
    ];
    
    for (id, name, age, active, created_at) in test_data {
        let mut record = TestRecord {
            id,
            name: [0u8; 32],
            age,
            active: if active { 1 } else { 0 }, // 将bool转换为u8
            _padding: [0u8; 2], // 初始化填充字段为0
            created_at,
        };
        
        let name_bytes = name.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);
        
        let insert_id = unsafe {
            db.get_table_mut(0).unwrap().insert(&record as *const _ as *const u8).unwrap()
        };
        // insert返回的是槽位索引，不是记录的id字段值
        assert!(insert_id < config.tables[0].max_records);
    }
    
    // 测试SQL查询
    
    // 调试信息：打印表名和字段名
    let table = unsafe { db.get_table_mut(0).unwrap() };
    println!("表名: {}", table.def.name);
    println!("记录大小: {}", table.def.record_size);
    for (i, field) in table.def.fields.iter().enumerate() {
        println!("字段 {}: 名称={}, 大小={}, 偏移={}", i, field.name, field.size, field.offset);
    }
    
    // 调试：手动遍历表记录
    println!("=== 手动遍历表记录 ===");
    let mut count = 0;
    unsafe {
        table.iterate(|id, record_ptr| {
            println!("记录 {} (ID: {}) 存在", count, id);
            count += 1;
            true
        }).unwrap();
    }
    println!("找到 {} 条记录", count);
    
    // 1. 测试基本SELECT查询
    let result = db.sql_query("SELECT * FROM TEST_TABLE").unwrap();
    println!("查询结果行数: {}", result.row_count());
    assert_eq!(result.row_count(), 5);
    assert_eq!(result.column_count(), 5);
    
    // 2. 测试SELECT特定列
    let result = db.sql_query("SELECT name, age FROM TEST_TABLE").unwrap();
    assert_eq!(result.row_count(), 5);
    assert_eq!(result.column_count(), 2);
    
    // 3. 测试SELECT带WHERE条件
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE age > 25").unwrap();
    assert_eq!(result.row_count(), 3);
    
    // 4. 测试SELECT带WHERE条件和ORDER BY
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE active = true ORDER BY age ASC").unwrap();
    assert_eq!(result.row_count(), 3);
    
    // 5. 测试SELECT带LIMIT
    let result = db.sql_query("SELECT * FROM TEST_TABLE LIMIT 2").unwrap();
    assert_eq!(result.row_count(), 2);
    
    // 6. 测试SELECT带WHERE条件和LIMIT
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE active = false LIMIT 1").unwrap();
    assert_eq!(result.row_count(), 1);
    
    // 7. 测试无效表名
    let result = db.sql_query("SELECT * FROM invalid_table");
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::TableNotFound));
    }
    
    // 8. 测试无效字段名
    let result = db.sql_query("SELECT invalid_field FROM TEST_TABLE");
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::FieldNotFound));
    }
    
    // 9. 测试SQL INSERT语句
    println!("=== 测试SQL INSERT语句 ===");
    
    // 先清空表，为INSERT测试做准备
    let table = unsafe { db.get_table_mut(0).unwrap() };
    let mut count = 0;
    let mut to_delete = Vec::new();
    unsafe {
        table.iterate(|id, _| {
            to_delete.push(id);
            count += 1;
            true
        }).unwrap();
        
        for id in to_delete {
            table.delete(id).unwrap();
        }
    }
    
    // 测试合法INSERT
    let result = db.sql_query("INSERT INTO TEST_TABLE (id, name, age, active, created_at) VALUES (1, 'TestUser', 30, true, 1620000000000)");
    assert!(result.is_ok(), "合法INSERT应该成功");
    
    // 测试重复主键INSERT，应该返回DuplicateKey（通过ConstraintsConflicts映射）
    let result = db.sql_query("INSERT INTO TEST_TABLE (id, name, age, active, created_at) VALUES (1, 'DuplicateUser', 25, false, 1620000001000)");
    assert!(result.is_err(), "重复主键INSERT应该失败");
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::DuplicateKey), 
               "重复主键应该返回DuplicateKey错误，实际返回: {:?}", err);
    }
    
    // 测试插入多个记录，其中包含重复主键
    let result = db.sql_query("INSERT INTO TEST_TABLE (id, name, age, active, created_at) VALUES \
                              (2, 'User2', 20, true, 1620000002000), \
                              (3, 'User3', 25, false, 1620000003000), \
                              (1, 'AnotherDuplicate', 40, true, 1620000004000)");
    assert!(result.is_err(), "包含重复主键的批量INSERT应该失败");
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::DuplicateKey), 
               "批量INSERT中的重复主键应该返回DuplicateKey错误");
    }
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
}
