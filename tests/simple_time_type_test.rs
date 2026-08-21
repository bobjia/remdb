#![cfg(feature = "std")]

extern crate alloc;

use remdb::{RemDb, config};
use remdb::config::WALConfig;

/// 创建测试用的DbConfig
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[],
    total_memory: 104857600,
    default_max_records: 100,
    low_power_mode_supported: false,
    low_power_max_records: None,
    // 添加缺少的字段
    memory_allocator: &config::DefaultMemoryAllocator,
    wal_config: WALConfig {
        log_path: "./wal",
        log_mode: config::LogMode::Async,
        log_prealloc_size: 0,
        log_file_size_limit: 104857600,
        log_segment_size: 1048576,
        checkpoint_interval_ms: 30000,
        retained_checkpoints: 2,
    },
    time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: Some(config::HAConfig {
        node_id: 1,
        ha_role: remdb::ha::HARole::Auto,
        replication_mode: remdb::ha::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 1000,
        master_address: None,
        master_port: None,
        replication_port: 5556,
    }),
};

#[test]
fn test_create_table_with_time_types() {
    // 使用静态内存缓冲区，确保它不会在函数返回时被释放
    static mut DB_MEMORY: [u8; 262144] = [0u8; 262144];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).unwrap();
    }
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_DB_CONFIG);
    db.init().unwrap();
    
    // 创建带有TIMESTAMP和TIMESTAMPTZ类型的表
    let create_table_sql = "CREATE TABLE test_time_types (
        id INTEGER PRIMARY KEY AUTO_INCREMENT,
        ts TIMESTAMP(3),
        tstz TIMESTAMPTZ(6),
        name TEXT
    )";
    
    println!("Running CREATE TABLE...");
    let result = db.sql_query(create_table_sql);
    if !result.is_ok() {
        println!("CREATE TABLE failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ CREATE TABLE succeeded");
    
    // 测试1: 插入带有时间类型的数据
    let insert_sql = "INSERT INTO test_time_types (ts, tstz, name) VALUES 
        (1609459200000, 1609459200000, 'test1'),
        (1609459260000, 1609459260000, 'test2')";
    
    println!("Running INSERT...");
    let result = db.sql_query(insert_sql);
    if !result.is_ok() {
        println!("INSERT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ INSERT succeeded");
    
    // 测试2: 查询数据 (simplified, no functions)
    let select_sql = "SELECT id, ts, tstz, name FROM test_time_types";
    
    println!("Running SELECT...");
    let result = db.sql_query(select_sql);
    if !result.is_ok() {
        println!("SELECT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ SELECT succeeded");

    // 测试2: 插入带有时间类型的数据，用函数
    let insert_sql = "INSERT INTO test_time_types (ts, tstz, name) VALUES 
      (NOW(), CURRENT_TIMESTAMP(), 'test1'),
        (LOCALTIMESTAMP(), NOW(), 'test2')";
    
    println!("Running INSERT...");
    let result = db.sql_query(insert_sql);
    if !result.is_ok() {
        println!("INSERT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ INSERT succeeded");
    
    // 测试3: 查询数据 (simplified, no functions)
    let select_sql = "SELECT id, ts, tstz, name FROM test_time_types";
    
    println!("Running SELECT...");
    let result = db.sql_query(select_sql);
    if !result.is_ok() {
        println!("SELECT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ SELECT succeeded");
    
    println!("\nAll tests passed! ✓");
}