#![allow(static_mut_refs)]
//! 批量插入示例
//!
//! 该示例展示如何使用 RemDB 的批量插入功能：
//! - 使用 batch_insert_record API
//! - 批量插入性能对比
//! - 批量插入事务处理

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::{RemDb, Result};

static mut DB_MEMORY: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024];

static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 16 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,

        model_worker_config: Default::default(),
    }));

    let mut db = RemDb::new(config);
    db.init()?;

    println!("=== 批量插入示例 ===\n");

    // 1. 创建测试表
    println!("1. 创建测试表");
    db.sql_query(
        "CREATE TABLE products (id INT32 PRIMARY KEY, name TEXT, price REAL, stock INT32)",
    )?;
    println!("   创建表: products");

    // 2. 单条插入 vs 批量插入对比
    println!("\n2. 单条插入 (100 条记录)");

    let start = std::time::Instant::now();
    for i in 1..=100 {
        let sql = format!(
            "INSERT INTO products VALUES ({}, 'Product_{}', {}, {})",
            i,
            i,
            i as f64 * 10.0,
            i * 10
        );
        db.sql_query(&sql)?;
    }
    let single_duration = start.elapsed();
    println!("   单条插入 100 条记录耗时: {:?}", single_duration);

    // 清空表
    db.sql_query("DELETE FROM products")?;

    // 3. 使用 SQL 批量插入
    println!("\n3. SQL 批量插入 (100 条记录)");

    let start = std::time::Instant::now();
    let mut values_str = String::new();
    for i in 1..=100 {
        if i > 1 {
            values_str.push_str(", ");
        }
        values_str.push_str(&format!(
            "({}, 'Product_{}', {}, {})",
            i,
            i,
            i as f64 * 10.0,
            i * 10
        ));
    }
    db.sql_query(&format!("INSERT INTO products VALUES {}", values_str))?;
    let sql_batch_duration = start.elapsed();
    println!("   SQL 批量插入 100 条记录耗时: {:?}", sql_batch_duration);

    // 清空表
    db.sql_query("DELETE FROM products")?;

    // 4. 使用 batch_insert_record API
    println!("\n4. 使用 batch_insert_record API");

    let columns: &[&str] = &["id", "name", "price", "stock"];
    let batch_size = 100;

    // 创建数据数组
    let mut id_strings: Vec<String> = Vec::with_capacity(batch_size);
    let mut name_strings: Vec<String> = Vec::with_capacity(batch_size);
    let mut price_strings: Vec<String> = Vec::with_capacity(batch_size);
    let mut stock_strings: Vec<String> = Vec::with_capacity(batch_size);

    for i in 1..=batch_size {
        id_strings.push(i.to_string());
        name_strings.push(format!("Product_{}", i));
        price_strings.push((i as f64 * 10.0).to_string());
        stock_strings.push((i * 10).to_string());
    }

    // 构建记录数组
    let mut records: Vec<Vec<&str>> = Vec::with_capacity(batch_size);
    for i in 0..batch_size {
        records.push(vec![
            id_strings[i].as_str(),
            name_strings[i].as_str(),
            price_strings[i].as_str(),
            stock_strings[i].as_str(),
        ]);
    }

    // 转换为 &[&[&str]]
    let records_ref: Vec<&[&str]> = records.iter().map(|v| v.as_slice()).collect();

    let start = std::time::Instant::now();
    let affected_rows = db.batch_insert_record("products", columns, &records_ref)?;
    let api_batch_duration = start.elapsed();
    println!(
        "   batch_insert_record 插入 {} 条记录耗时: {:?}",
        affected_rows, api_batch_duration
    );

    // 5. 验证插入结果
    println!("\n5. 验证插入结果");
    let result = db.sql_query("SELECT COUNT(*) as count FROM products")?;
    println!("   表中记录数:");
    println!("{}", result.to_string());

    // 6. 查询部分数据
    println!("\n6. 查询前 10 条数据");
    let result = db.sql_query("SELECT * FROM products ORDER BY id LIMIT 10")?;
    println!("{}", result.to_string());

    // 7. 大批量插入测试
    println!("\n7. 大批量插入测试 (1000 条记录)");

    // 创建新表用于大批量测试
    db.sql_query("CREATE TABLE large_data (id INT32 PRIMARY KEY, value TEXT, timestamp INT64)")?;

    let batch_size = 1000;
    let columns: &[&str] = &["id", "value", "timestamp"];

    let mut id_strings: Vec<String> = Vec::with_capacity(batch_size);
    let mut value_strings: Vec<String> = Vec::with_capacity(batch_size);
    let mut ts_strings: Vec<String> = Vec::with_capacity(batch_size);

    for i in 1..=batch_size {
        id_strings.push(i.to_string());
        value_strings.push(format!("Value_{}", i));
        ts_strings.push((i as i64 * 1000).to_string());
    }

    let mut records: Vec<Vec<&str>> = Vec::with_capacity(batch_size);
    for i in 0..batch_size {
        records.push(vec![
            id_strings[i].as_str(),
            value_strings[i].as_str(),
            ts_strings[i].as_str(),
        ]);
    }

    let records_ref: Vec<&[&str]> = records.iter().map(|v| v.as_slice()).collect();

    let start = std::time::Instant::now();
    let affected_rows = db.batch_insert_record("large_data", columns, &records_ref)?;
    let large_batch_duration = start.elapsed();
    println!(
        "   批量插入 {} 条记录耗时: {:?}",
        affected_rows, large_batch_duration
    );

    // 8. 统计信息
    println!("\n8. 插入性能统计");
    println!("   单条插入 100 条: {:?}", single_duration);
    println!("   SQL 批量插入 100 条: {:?}", sql_batch_duration);
    println!("   API 批量插入 100 条: {:?}", api_batch_duration);
    println!("   API 批量插入 1000 条: {:?}", large_batch_duration);

    if sql_batch_duration.as_micros() > 0 {
        let speedup = single_duration.as_micros() as f64 / sql_batch_duration.as_micros() as f64;
        println!("   SQL 批量插入相比单条插入加速: {:.2}x", speedup);
    }

    // 9. 事务中的批量插入
    println!("\n9. 事务中的批量插入");

    db.sql_query("CREATE TABLE transactional_data (id INT32 PRIMARY KEY, data TEXT)")?;

    // 开始事务
    db.sql_query("BEGIN TRANSACTION")?;

    let columns: &[&str] = &["id", "data"];
    let mut id_strings: Vec<String> = Vec::new();
    let mut data_strings: Vec<String> = Vec::new();

    for i in 1..=50 {
        id_strings.push(i.to_string());
        data_strings.push(format!("Data_{}", i));
    }

    let mut records: Vec<Vec<&str>> = Vec::new();
    for i in 0..50 {
        records.push(vec![id_strings[i].as_str(), data_strings[i].as_str()]);
    }

    let records_ref: Vec<&[&str]> = records.iter().map(|v| v.as_slice()).collect();

    db.batch_insert_record("transactional_data", columns, &records_ref)?;
    println!("   在事务中批量插入 50 条记录");

    // 提交事务
    db.sql_query("COMMIT")?;
    println!("   提交事务");

    let result = db.sql_query("SELECT COUNT(*) as count FROM transactional_data")?;
    println!("   事务提交后的记录数:");
    println!("{}", result.to_string());

    println!("\n=== 批量插入示例完成 ===");
    Ok(())
}
