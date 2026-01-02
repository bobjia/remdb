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
    unsafe {
        crate::transaction::TX_MANAGER.clear_log_manager();
    }
    
    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("remdb.log");
    
    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();
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
    file.read_to_string(&mut contents).expect("Failed to read DDL file");
    
    // 打印调试信息
    println!("Generated DDL:
{}", contents);
    
    // 验证文件内容
    assert!(contents.contains("CREATE TABLE test_table"), "DDL file should contain CREATE TABLE statement");
    assert!(contents.contains("PRIMARY KEY"), "DDL file should contain primary key definition");
    assert!(contents.contains("name"), "DDL file should contain name field definition");
    assert!(contents.contains("CREATE INDEX"), "DDL file should contain CREATE INDEX statement");
    
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
    unsafe {
        crate::transaction::TX_MANAGER.clear_log_manager();
    }
    
    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("remdb.log");
    
    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();
    }
    
    // 使用init_global_db函数初始化数据库
    let db = init_global_db(&TEST_DB).unwrap();
    
    // 获取表
    let table_id = 0;
    let table = db.get_table_mut(table_id).unwrap();
    
    // 添加调试信息
    println!("DEBUG: TEST_TABLE.record_size = {}", TEST_TABLE.record_size);
    println!("DEBUG: Number of fields = {}", TEST_TABLE.fields.len());
    for (i, field) in TEST_TABLE.fields.iter().enumerate() {
        println!("DEBUG: Field {} - name: {}, offset: {}, size: {}", 
                 i, field.name, field.offset, field.size);
    }
    
    // 插入测试数据，使用动态内存分配避免栈溢出
    let mut record1 = Vec::with_capacity(TEST_TABLE.record_size);
    record1.resize(TEST_TABLE.record_size, 0);
    println!("DEBUG: record1 len = {}, capacity = {}", record1.len(), record1.capacity());
    
    // 直接使用具体的偏移量值，避免运行时计算错误
    unsafe {
        // id: 1 at offset 0
        let id_ptr = record1.as_mut_ptr() as *mut i32;
        *id_ptr = 1;
        println!("DEBUG: Set id = 1");
        
        // name: "Test User" at offset 4 (i32是4字节)
        let name_ptr = record1.as_mut_ptr().add(4) as *mut u8;
        let name = "Test User";
        for (i, &c) in name.as_bytes().iter().enumerate() {
            if i < 32 { // name field size is 32
                *name_ptr.add(i) = c;
            }
        }
        println!("DEBUG: Set name = Test User");
        
        // age: 25 at offset 36 (4 + 32)
        let age_ptr = record1.as_mut_ptr().add(36) as *mut i8;
        *age_ptr = 25;
        println!("DEBUG: Set age = 25");
        
        // active: true at offset 37 (36 + 1)
        let active_ptr = record1.as_mut_ptr().add(37) as *mut u8;
        *active_ptr = 1;
        println!("DEBUG: Set active = true");
        
        // created_at: 1234567890 at offset 40 (37 + 1 + 2 padding for alignment)
        let created_at_ptr = record1.as_mut_ptr().add(40) as *mut u64;
        *created_at_ptr = 1234567890;
        println!("DEBUG: Set created_at = 1234567890");
    }
    
    // 插入记录
    let _ = table.insert(record1.as_ptr());
    
    // 导出数据到文件
    let data_path = "test_data.sql";
    let result = db.export_data(data_path);
    assert!(result.is_ok(), "Failed to export data");
    
    // 读取并验证导出的数据文件
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(data_path).expect("Failed to open data file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Failed to read data file");
    
    // 打印调试信息
    println!("Generated data:
{}", contents);
    
    // 验证文件内容
    assert!(contents.contains("INSERT INTO test_table"), "Data file should contain INSERT statement");
    assert!(contents.contains("1"), "Data file should contain correct record values");
    
    // 清理测试文件
    std::fs::remove_file(data_path).expect("Failed to remove data file");
    
    // 重置全局数据库 - 这会自动触发Drop trait，释放所有表内存
    reset_global_db();
}

#[test]
#[serial]
fn test_export_empty_table() {
    // 重置事务管理器状态，避免测试之间的状态污染
    unsafe {
        crate::transaction::TX_MANAGER.clear_log_manager();
    }
    
    // 在测试开始前，删除可能存在的日志文件，避免影响后续测试
    use std::fs::remove_file;
    let _ = remove_file("remdb.log");
    
    // 初始化全局内存分配器，使用静态内存缓冲区
    unsafe {
        crate::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();
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
    file.read_to_string(&mut contents).expect("Failed to read empty data file");
    
    // 验证数据文件为空（允许空白字符）
    assert!(contents.trim().is_empty(), "Data file for empty table should be empty");
    
    // 清理测试文件
    std::fs::remove_file(ddl_path).expect("Failed to remove empty DDL file");
    std::fs::remove_file(data_path).expect("Failed to remove empty data file");
    
    // 重置全局数据库
    reset_global_db();
    
    // 重置全局分配器
    unsafe {
        crate::memory::allocator::reset_global_allocator().unwrap();
    }
}