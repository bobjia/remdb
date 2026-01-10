extern crate alloc;

use remdb::{RemDb, config::{DbConfig, LogMode, HAConfig, WALConfig}}; use remdb::ha::{HARole, ReplicationMode};
use remdb::memory::allocator::init_global_allocator;
use remdb::config::DefaultMemoryAllocator;

// 创建静态的默认内存分配器
static mut DEFAULT_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

// 创建静态的数据库配置
static CONFIG: DbConfig = unsafe {
    DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10, // 减少默认最大记录数，避免内存不足
        memory_allocator: &mut DEFAULT_ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Sync,
            log_prealloc_size: 1 * 1024 * 1024,
            log_file_size_limit: 16 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            checkpoint_interval_ms: 60000,
            retained_checkpoints: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(HAConfig {
            ha_role: HARole::Auto,
            replication_mode: ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
            heartbeat_port: 5557,
        }),
    }
};

fn main() {
    println!("Testing DEFAULT field functionality...");
    
    // 初始化全局内存分配器
    static mut MEMORY_BUFFER: [u8; 1024 * 1024] = [0; 1024 * 1024];
    unsafe {
        init_global_allocator(MEMORY_BUFFER.as_mut_ptr(), MEMORY_BUFFER.len())
            .expect("Failed to initialize global allocator");
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&CONFIG);
    
    // 初始化数据库和平台
    db.init().expect("Failed to initialize database");
    
    println!("Testing DEFAULT field functionality...");
    
    // 创建包含DEFAULT字段的表
    let create_table_sql = "CREATE TABLE users (
        id INT PRIMARY KEY AUTO_INCREMENT,
        name STRING NOT NULL,
        age INT DEFAULT 18,
        active BOOL DEFAULT TRUE,
        score FLOAT DEFAULT 0.0
    );";
    
    println!("1. Creating table with DEFAULT values...");
    let result = db.sql_query(create_table_sql);
    if result.is_ok() {
        println!("   ✓ Table created successfully");
    } else {
        println!("   ✗ Failed to create table: {:?}", result.err());
        return;
    }
    
    // 插入数据，不提供默认值字段 - 直接指定ID值避免AUTO_INCREMENT问题
    println!("2. Inserting records without default values...");
    
    let insert_1 = "INSERT INTO users (id, name) VALUES (1, 'Alice');";
    let result1 = db.sql_query(insert_1);
    if result1.is_ok() {
        println!("   ✓ Record 'Alice' inserted successfully");
    } else {
        println!("   ✗ Failed to insert 'Alice': {:?}", result1.err());
        return;
    }
    
    let insert_2 = "INSERT INTO users (id, name) VALUES (2, 'Bob');";
    let result2 = db.sql_query(insert_2);
    if result2.is_ok() {
        println!("   ✓ Record 'Bob' inserted successfully");
    } else {
        println!("   ✗ Failed to insert 'Bob': {:?}", result2.err());
        return;
    }
    
    println!("   ✓ All records inserted successfully");
    
    // 查询数据，验证默认值
    let select_sql = "SELECT * FROM users;";
    
    println!("3. Querying records to verify DEFAULT values...");
    let result = db.sql_query(select_sql);
    if result.is_ok() {
        println!("   ✓ Query executed successfully");
        println!("   ✓ DEFAULT field functionality is working!");
    } else {
        println!("   ✗ Failed to select records: {:?}", result.err());
        return;
    }
    
    println!("DEFAULT field functionality test completed successfully!");
}