// 导出功能测试
// 使用标准库
#![cfg(feature = "std")]

// 确保测试顺序执行，避免全局状态冲突
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use remdb::*;
use serial_test::serial;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024]; // 1MB内存，用于所有测试用例

// 定义表结构
remdb::table!(
    TEST_TABLE,
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
    tables: [TEST_TABLE]
);

#[test]
#[serial]
fn test_export_ddl() {
    // 重置事务管理器状态，避免测试之间的状态污染
    crate::transaction::reset_log_manager();

    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("./wal");

    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }

    // 使用init_global_db函数初始化数据库
    let db = init_global_db(&TEST_DB).unwrap();

    // 导出DDL到文件
    let ddl_path = "test_ddl.sql";
    let result = db.export_ddl(ddl_path);
    assert!(result.is_ok(), "Failed to export DDL");

    // 读取并验证导出的DDL文件
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(ddl_path).expect("Failed to open DDL file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read DDL file");

    // 打印调试信息
    println!(
        "Generated DDL:
{}",
        contents
    );

    // 验证文件内容
    assert!(
        contents.contains("CREATE TABLE test_table"),
        "DDL file should contain CREATE TABLE statement"
    );
    assert!(
        contents.contains("PRIMARY KEY"),
        "DDL file should contain primary key definition"
    );
    assert!(
        contents.contains("name"),
        "DDL file should contain name field definition"
    );
    assert!(
        contents.contains("CREATE INDEX"),
        "DDL file should contain CREATE INDEX statement"
    );

    // 清理测试文件
    std::fs::remove_file(ddl_path).expect("Failed to remove DDL file");

    // 重置全局数据库 - 这会释放所有表及其内存
    reset_global_db();

    // 重置全局分配器
    crate::memory::allocator::reset_global_allocator().unwrap();
}

#[test]
#[serial]
fn test_export_data() {
    // 重置事务管理器状态，避免测试之间的状态污染
    crate::transaction::reset_log_manager();

    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("./wal");

    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }

    // 使用init_global_db函数初始化数据库
    let db = init_global_db(&TEST_DB).unwrap();

    // 使用SQL INSERT语句插入测试数据，避免直接访问表
    let result = db.sql_query("INSERT INTO TEST_TABLE (id, name, age, active, created_at) VALUES (1, 'Test User', 25, true, 1234567890)");
    assert!(result.is_ok(), "插入测试数据应该成功");

    // 导出数据到文件
    let data_path = "test_data.sql";
    let result = db.export_data(data_path);
    assert!(result.is_ok(), "Failed to export data");

    // 读取并验证导出的数据文件
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(data_path).expect("Failed to open data file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read data file");

    // 打印调试信息
    println!(
        "Generated data:
{}",
        contents
    );

    // 验证文件内容
    assert!(
        contents.contains("INSERT INTO test_table"),
        "Data file should contain INSERT statement"
    );
    assert!(
        contents.contains("1"),
        "Data file should contain correct record values"
    );

    // 清理测试文件
    std::fs::remove_file(data_path).expect("Failed to remove data file");

    // 重置全局数据库 - 这会自动触发Drop trait，释放所有表内存
    reset_global_db();

    // 重置全局内存分配器
    crate::memory::allocator::reset_global_allocator().unwrap();
}

#[test]
#[serial]
fn test_export_empty_table() {
    // 重置事务管理器状态，避免测试之间的状态污染
    crate::transaction::reset_log_manager();

    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("./wal");

    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }

    // 使用init_global_db函数初始化数据库
    let db = init_global_db(&TEST_DB).unwrap();

    // 导出DDL到文件
    let ddl_path = "test_empty_ddl.sql";
    let result = db.export_ddl(ddl_path);
    assert!(result.is_ok(), "Failed to export DDL for empty table");

    // 导出数据到文件
    let data_path = "test_empty_data.sql";
    let result = db.export_data(data_path);
    assert!(result.is_ok(), "Failed to export data for empty table");

    // 读取并验证导出的数据文件（应为空）
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(data_path).expect("Failed to open empty data file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read empty data file");

    // 验证数据文件为空（允许空白字符）
    assert!(
        contents.trim().is_empty(),
        "Data file for empty table should be empty"
    );

    // 清理测试文件
    std::fs::remove_file(ddl_path).expect("Failed to remove empty DDL file");
    std::fs::remove_file(data_path).expect("Failed to remove empty data file");

    // 重置全局数据库
    reset_global_db();

    // 重置全局分配器
    crate::memory::allocator::reset_global_allocator().unwrap();
}
