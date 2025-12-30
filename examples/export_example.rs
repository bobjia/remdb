// 导出功能示例
// 使用标准库
#![cfg(feature = "std")]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

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

fn main() {
    println!("=== RemDB 导出功能示例 ===");
    
    // 1. 初始化内存分配器
    println!("1. 初始化内存分配器...");
    let mut memory = [0u8; 65536];
    crate::memory::allocator::init_global_allocator(memory.as_mut_ptr(), memory.len()).unwrap();
    println!("   内存分配器初始化成功");
    
    // 2. 初始化数据库
    println!("2. 初始化数据库...");
    let db = init_global_db(&TEST_DB).unwrap();
    println!("   数据库初始化成功");
    
    // 3. 插入测试数据
    println!("3. 插入测试数据...");
    let table_id = 0;
    let mut table = db.get_table_mut(table_id).unwrap();
    
    // 准备测试记录
    let mut record1 = [0u8; 4 + 32 + 1 + 1 + 8]; // id(4) + name(32) + age(1) + active(1) + created_at(8)
    
    unsafe {
        // id: 1
        let id_ptr = record1.as_mut_ptr() as *mut i32;
        *id_ptr = 1;
        
        // name: "Alice"
        let name = "Alice";
        let name_ptr = record1.as_mut_ptr().add(4) as *mut u8;
        for (i, &c) in name.as_bytes().iter().enumerate() {
            *name_ptr.add(i) = c;
        }
        
        // age: 25
        let age_ptr = record1.as_mut_ptr().add(4 + 32) as *mut i8;
        *age_ptr = 25;
        
        // active: true
        let active_ptr = record1.as_mut_ptr().add(4 + 32 + 1) as *mut u8;
        *active_ptr = 1;
        
        // created_at: 1234567890
        let created_at_ptr = record1.as_mut_ptr().add(4 + 32 + 1 + 1) as *mut u64;
        *created_at_ptr = 1234567890;
    }
    
    // 插入记录
    let _ = table.insert(record1.as_ptr());
    println!("   插入记录成功: id=1, name=Alice");
    
    // 4. 导出DDL
    println!("4. 导出DDL到文件...");
    let ddl_path = "test_schema.ddl";
    let result = db.export_ddl(ddl_path);
    if result.is_ok() {
        println!("   DDL导出成功: {}", ddl_path);
    } else {
        println!("   DDL导出失败: {:?}", result.err());
        return;
    }
    
    // 5. 导出数据
    println!("5. 导出数据到文件...");
    let data_path = "test_data.sql";
    let result = db.export_data(data_path);
    if result.is_ok() {
        println!("   数据导出成功: {}", data_path);
    } else {
        println!("   数据导出失败: {:?}", result.err());
        return;
    }
    
    // 6. 显示导出结果
    println!("\n=== 导出结果 ===");
    
    // 显示DDL文件内容
    println!("\nDDL文件内容 ({}):", ddl_path);
    let ddl_content = std::fs::read_to_string(ddl_path).unwrap();
    println!("{}", ddl_content);
    
    // 显示数据文件内容
    println!("\n数据文件内容 ({}):", data_path);
    let data_content = std::fs::read_to_string(data_path).unwrap();
    println!("{}", data_content);
    
    // 7. 清理
    println!("\n=== 清理 ===");
    std::fs::remove_file(ddl_path).unwrap();
    std::fs::remove_file(data_path).unwrap();
    reset_global_db();
    println!("   清理完成");
    
    println!("\n=== 示例结束 ===");
}