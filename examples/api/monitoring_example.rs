//! 监控和健康检查示例
//!
//! 该示例展示如何使用 RemDB 的监控功能：
//! - 获取数据库指标
//! - 健康检查
//! - 重置指标
//! - 指标快照

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::{RemDb, Result};

static mut DB_MEMORY: [u8; 4 * 1024 * 1024] = [0; 4 * 1024 * 1024];

static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 4 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
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
    }));

    let mut db = RemDb::new(config);
    db.init()?;

    println!("=== 监控和健康检查示例 ===\n");

    // 1. 初始健康检查
    println!("1. 初始健康检查");
    let health = db.health_check();
    println!("   健康状态: {:?}", health.status);
    println!("   详细信息: {}", health.details);

    // 2. 获取初始指标
    println!("\n2. 初始指标");
    let metrics = db.get_metrics();
    println!("   总内存: {} bytes", metrics.total_memory);
    let snapshot = metrics.snapshot();
    println!("   已用内存: {} bytes", snapshot.used_memory);
    println!("   读操作数: {}", snapshot.read_ops);
    println!("   写操作数: {}", snapshot.write_ops);
    println!("   删除操作数: {}", snapshot.delete_ops);
    println!("   更新操作数: {}", snapshot.update_ops);
    println!("   事务数: {}", snapshot.transactions);
    println!("   已提交事务: {}", snapshot.committed_transactions);
    println!("   回滚事务: {}", snapshot.rolled_back_transactions);

    // 3. 创建表并插入数据
    println!("\n3. 执行一些操作以产生指标");
    db.sql_query("CREATE TABLE test_table (id INT32 PRIMARY KEY, name TEXT, value REAL)")?;
    println!("   创建表: test_table");
    
    for i in 1..=10 {
        db.sql_query(&format!("INSERT INTO test_table VALUES ({}, 'Name_{}', {})", i, i, i as f64 * 10.0))?;
    }
    println!("   插入 10 条记录");
    
    db.sql_query("SELECT * FROM test_table")?;
    println!("   执行查询");
    
    db.sql_query("UPDATE test_table SET value = 999.99 WHERE id = 1")?;
    println!("   更新 1 条记录");
    
    db.sql_query("DELETE FROM test_table WHERE id = 10")?;
    println!("   删除 1 条记录");

    // 4. 获取更新后的指标
    println!("\n4. 操作后的指标");
    let metrics = db.get_metrics();
    let snapshot = metrics.snapshot();
    println!("   读操作数: {}", snapshot.read_ops);
    println!("   写操作数: {}", snapshot.write_ops);
    println!("   删除操作数: {}", snapshot.delete_ops);
    println!("   更新操作数: {}", snapshot.update_ops);
    println!("   缓存命中: {}", snapshot.cache_hits);
    println!("   缓存未命中: {}", snapshot.cache_misses);
    println!("   索引查找: {}", snapshot.index_lookups);
    println!("   索引插入: {}", snapshot.index_inserts);

    // 5. 指标快照
    println!("\n5. 指标快照");
    let snapshot = db.metrics_snapshot();
    println!("   总内存: {} bytes", snapshot.total_memory);
    println!("   已用内存: {} bytes", snapshot.used_memory);
    println!("   缓存命中率: {:.2}%", snapshot.cache_hit_rate);
    println!("   读操作数: {}", snapshot.read_ops);
    println!("   写操作数: {}", snapshot.write_ops);

    // 6. 导出指标
    println!("\n6. 导出指标 (文本格式)");
    let metrics_str = snapshot.to_text();
    println!("{}", metrics_str);

    // 7. 再次健康检查
    println!("\n7. 操作后健康检查");
    let health = db.health_check();
    println!("   健康状态: {:?}", health.status);
    println!("   详细信息: {}", health.details);
    let mem_usage_percent = if health.metrics.total_memory > 0 {
        (health.metrics.used_memory as f64 / health.metrics.total_memory as f64) * 100.0
    } else {
        0.0
    };
    println!("   内存使用: {} / {} bytes ({:.2}%)", 
        health.metrics.used_memory, 
        health.metrics.total_memory,
        mem_usage_percent
    );

    // 8. 重置指标
    println!("\n8. 重置指标");
    db.reset_metrics();
    println!("   指标已重置");
    
    let metrics = db.get_metrics();
    let snapshot = metrics.snapshot();
    println!("   重置后的读操作数: {}", snapshot.read_ops);
    println!("   重置后的写操作数: {}", snapshot.write_ops);

    // 9. 表数量信息
    println!("\n9. 数据库信息");
    println!("   表数量: {}", db.table_count());
    println!("   时序表数量: {}", db.time_series_table_count());

    // 10. 性能指标计算
    println!("\n10. 性能指标计算");
    
    // 执行更多操作
    let start = std::time::Instant::now();
    for i in 100..200 {
        db.sql_query(&format!("INSERT INTO test_table VALUES ({}, 'Name_{}', {})", i, i, i as f64 * 10.0))?;
    }
    let duration = start.elapsed();
    
    let metrics = db.get_metrics();
    let snapshot = metrics.snapshot();
    let ops_per_sec = 100.0 / duration.as_secs_f64();
    println!("   插入 100 条记录耗时: {:?}", duration);
    println!("   每秒操作数: {:.2}", ops_per_sec);
    println!("   总写操作数: {}", snapshot.write_ops);

    println!("\n=== 监控和健康检查示例完成 ===");
    Ok(())
}
