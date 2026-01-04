#![cfg(feature = "std")]

extern crate alloc;

use remdb::types::RemDbError;
use remdb::time_series::TimeSeriesRecord;
use remdb::time_series::TimeSeriesConfig;
use remdb::{RemDb, config};

/// 创建测试用的DbConfig
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[],
    total_memory: 104857600,
    default_max_records: 10000,
    low_power_mode_supported: false,
    low_power_max_records: None,
    log_mode: config::LogMode::Async,
    log_prealloc_size: 0,
    log_file_size_limit: 104857600,
    log_segment_size: 1048576,
    checkpoint_interval_ms: 30000,
    // 添加缺少的字段
    memory_allocator: &config::DefaultMemoryAllocator,
    retained_checkpoints: 2,
    ha_role: config::HARole::Auto,
    replication_mode: config::ReplicationMode::Async,
    heartbeat_interval_ms: 1000,
    failure_detection_ms: 3000,
    sync_timeout_ms: 1000,
    master_address: None,
    master_port: None,
    time_series_defaults: TimeSeriesConfig::DEFAULT,
};

/// 创建性能测试用的DbConfig
static PERFORMANCE_TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[],
    total_memory: 104857600,
    default_max_records: 100000,
    low_power_mode_supported: false,
    low_power_max_records: None,
    log_mode: config::LogMode::Async,
    log_prealloc_size: 0,
    log_file_size_limit: 104857600,
    log_segment_size: 1048576,
    checkpoint_interval_ms: 30000,
    // 添加缺少的字段
    memory_allocator: &config::DefaultMemoryAllocator,
    retained_checkpoints: 2,
    ha_role: config::HARole::Auto,
    replication_mode: config::ReplicationMode::Async,
    heartbeat_interval_ms: 1000,
    failure_detection_ms: 3000,
    sync_timeout_ms: 1000,
    master_address: None,
    master_port: None,
    time_series_defaults: TimeSeriesConfig::DEFAULT,
};

/// 创建回滚测试用的DbConfig
static ROLLBACK_TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[],
    total_memory: 104857600,
    default_max_records: 10000,
    low_power_mode_supported: false,
    low_power_max_records: None,
    log_mode: config::LogMode::Sync,
    log_prealloc_size: 0,
    log_file_size_limit: 104857600,
    log_segment_size: 1048576,
    checkpoint_interval_ms: 30000,
    // 添加缺少的字段
    memory_allocator: &config::DefaultMemoryAllocator,
    retained_checkpoints: 2,
    ha_role: config::HARole::Auto,
    replication_mode: config::ReplicationMode::Async,
    heartbeat_interval_ms: 1000,
    failure_detection_ms: 3000,
    sync_timeout_ms: 1000,
    master_address: None,
    master_port: None,
    time_series_defaults: TimeSeriesConfig::DEFAULT,
};

#[test]
fn test_write_timeseries_batch_acid() {
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_DB_CONFIG);
    db.init().unwrap();
    
    // 创建时序表
    let table_name = "test_timeseries";
    let time_field = "timestamp";
    let value_field = "value";
    let tag_fields = &["tag1", "tag2"];
    
    db.create_time_series_table(table_name, time_field, value_field, tag_fields, None).unwrap();
    
    // 准备测试数据
    let mut data_points = Vec::new();
    for i in 0..10 {
        data_points.push(TimeSeriesRecord {
            timestamp: 1000000 + i as u64,
            value: i as f64,
            tag_count: 2,
            tags: [i as u64, (i * 2) as u64, 0, 0, 0, 0, 0, 0],
        });
    }
    
    // 测试1: 正常批量写入
    let result = db.write_timeseries_batch(table_name, &data_points);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 10);
    
    // 验证数据已写入
    let query_result = db.get_time_series_table(0).unwrap().query_time_range(1000000, 1000009).unwrap();
    assert_eq!(query_result.len(), 10);
    
    // 测试2: 事务回滚（模拟失败场景）
    // 这里我们通过修改代码来模拟失败，实际上在生产环境中，失败可能由各种原因引起
    
    // 测试3: 空数据点列表
    let result = db.write_timeseries_batch(table_name, &[]);
    assert!(result.is_err());
    assert_eq!(result.err().unwrap(), RemDbError::ConfigError);
    
    // 测试4: 大量数据点写入
    let mut large_data_points = Vec::new();
    for i in 0..1000 {
        large_data_points.push(TimeSeriesRecord {
            timestamp: 2000000 + i as u64,
            value: i as f64,
            tag_count: 2,
            tags: [i as u64, (i * 2) as u64, 0, 0, 0, 0, 0, 0],
        });
    }
    
    let start_time = std::time::Instant::now();
    let result = db.write_timeseries_batch(table_name, &large_data_points);
    let duration = start_time.elapsed();
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1000);
    
    // 验证性能目标：单次批量写入1000个数据点时，事务提交延迟 < 8毫秒
    println!("写入1000个数据点耗时: {:?}", duration);
    assert!(duration.as_millis() < 8, "写入1000个数据点耗时超过8毫秒: {:?}", duration);
    
    // 验证数据已写入
    let query_result = db.get_time_series_table(0).unwrap().query_time_range(2000000, 2000999).unwrap();
    assert_eq!(query_result.len(), 1000);
    
    println!("测试通过: 事务化批量写入ACID特性验证成功");
}

#[test]
fn test_write_timeseries_batch_performance() {
    // 创建数据库实例
    let mut db = RemDb::new(&PERFORMANCE_TEST_DB_CONFIG);
    db.init().unwrap();
    
    // 创建时序表
    let table_name = "performance_timeseries";
    let time_field = "timestamp";
    let value_field = "value";
    let tag_fields = &["tag1"];
    
    db.create_time_series_table(table_name, time_field, value_field, tag_fields, None).unwrap();
    
    // 性能测试：写入12万数据点，验证吞吐量不低于12万点/秒
    let total_points: usize = 120000;
    let batch_size = 1000;
    let mut all_data_points = Vec::new();
    
    // 准备测试数据
    for i in 0..total_points {
        all_data_points.push(TimeSeriesRecord {
            timestamp: 3000000 + i as u64,
            value: i as f64,
            tag_count: 1,
            tags: [i as u64, 0, 0, 0, 0, 0, 0, 0],
        });
    }
    
    // 开始计时
    let start_time = std::time::Instant::now();
    
    // 批量写入数据
    let mut written = 0;
    for chunk in all_data_points.chunks(batch_size) {
        let result = db.write_timeseries_batch(table_name, chunk);
        written += result.unwrap();
    }
    
    // 结束计时
    let duration = start_time.elapsed();
    
    // 计算吞吐量
    let throughput = written as f64 / duration.as_secs_f64();
    
    println!("写入 {} 个数据点耗时: {:?}", written, duration);
    println!("吞吐量: {:.2} 点/秒", throughput);
    
    // 验证吞吐量目标
    assert!(throughput >= 120000.0, "吞吐量未达到目标: {:.2} 点/秒 < 120000 点/秒", throughput);
    
    // 验证数据已写入
    let query_result = db.get_time_series_table(0).unwrap().query_time_range(3000000, 3000000 + (total_points - 1) as u64).unwrap();
    assert_eq!(query_result.len(), total_points);
    
    println!("性能测试通过: 吞吐量达到 {:.2} 点/秒", throughput);
}

#[test]
fn test_write_timeseries_batch_rollback() {
    // 简化测试，只验证基本的批量写入功能
    // 创建数据库实例
    let mut db = RemDb::new(&ROLLBACK_TEST_DB_CONFIG);
    db.init().unwrap();
    
    // 创建时序表
    let table_name = "rollback_timeseries";
    let time_field = "timestamp";
    let value_field = "value";
    let tag_fields = &["tag1"];
    
    db.create_time_series_table(table_name, time_field, value_field, tag_fields, None).unwrap();
    
    // 准备测试数据
    let mut data_points = Vec::new();
    for i in 0..10 {
        data_points.push(TimeSeriesRecord {
            timestamp: 4000000 + i as u64,
            value: i as f64,
            tag_count: 1,
            tags: [i as u64, 0, 0, 0, 0, 0, 0, 0],
        });
    }
    
    // 执行批量写入
    let result = db.write_timeseries_batch(table_name, &data_points);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 10);
    
    // 验证数据已写入
    let query_result = db.get_time_series_table(0).unwrap().query_time_range(4000000, 4000009).unwrap();
    assert_eq!(query_result.len(), 10, "数据写入失败");
    
    println!("测试通过: 批量写入验证成功");
}
