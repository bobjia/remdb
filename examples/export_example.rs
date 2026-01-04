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
    
    // 使用insert_record插入记录
    let columns = &["id", "name", "age", "active", "created_at"];
    let values = &["1", "Alice", "25", "true", "1234567890"];
    let affected_rows = db.insert_record("TEST_TABLE", columns, values).unwrap();
    println!("   插入记录成功: id=1, name=Alice, 影响行数: {}", affected_rows);
    
    // 4. 创建时序表
    println!("4. 创建时序表...");
    
    // 直接使用API创建时序表，绕过SQL解析器的限制
    
    // 创建第一个时序表：delta-delta压缩，30天TTL
    let mut ts_config1 = crate::time_series::TimeSeriesConfig::DEFAULT;
    ts_config1.compression = crate::time_series::CompressionType::DeltaDelta;
    ts_config1.retention_period_secs = 30 * 24 * 3600; // 30天
    
    let result1 = db.create_time_series_table(
        "test_ts1",
        "ts",
        "value",
        &["tag1", "tag2"],
        Some(ts_config1)
    );
    
    if result1.is_ok() {
        println!("   创建时序表成功: test_ts1");
    } else {
        println!("   创建时序表失败: test_ts1");
    }
    
    // 创建第二个时序表：delta压缩，7天TTL
    let mut ts_config2 = crate::time_series::TimeSeriesConfig::DEFAULT;
    ts_config2.compression = crate::time_series::CompressionType::Delta;
    ts_config2.retention_period_secs = 7 * 24 * 3600; // 7天
    
    let result2 = db.create_time_series_table(
        "test_ts2",
        "timestamp",
        "temperature",
        &["location"],
        Some(ts_config2)
    );
    
    if result2.is_ok() {
        println!("   创建时序表成功: test_ts2");
    } else {
        println!("   创建时序表失败: test_ts2");
    }
    
    // 5. 导出DDL
    println!("5. 导出DDL到文件...");
    let ddl_path = "test_schema.ddl";
    let result = db.export_ddl(ddl_path);
    if result.is_ok() {
        println!("   DDL导出成功: {}", ddl_path);
    } else {
        println!("   DDL导出失败: {:?}", result.err());
        return;
    }
    
    // 6. 导出数据
    println!("6. 导出数据到文件...");
    let data_path = "test_data.sql";
    let result = db.export_data(data_path);
    if result.is_ok() {
        println!("   数据导出成功: {}", data_path);
    } else {
        println!("   数据导出失败: {:?}", result.err());
        return;
    }
    
    // 7. 显示导出结果
    println!("\n=== 导出结果 ===");
    
    // 显示DDL文件内容
    println!("\nDDL文件内容 ({}):", ddl_path);
    let ddl_content = std::fs::read_to_string(ddl_path).unwrap();
    println!("{}", ddl_content);
    
    // 显示数据文件内容
    println!("\n数据文件内容 ({}):", data_path);
    let data_content = std::fs::read_to_string(data_path).unwrap();
    println!("{}", data_content);
    
    // 8. 清理
    println!("\n=== 清理 ===");
    std::fs::remove_file(ddl_path).unwrap();
    std::fs::remove_file(data_path).unwrap();
    reset_global_db();
    println!("   清理完成");
    
    println!("\n=== 示例结束 ===");
}