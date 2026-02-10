#![cfg(all(feature = "std", feature = "ha"))]

use remdb::{
    config::{DbConfig, DefaultMemoryAllocator, LogMode, TimeSeriesConfig, WALConfig},
    ha::{HAConfig, HARole, ReplicationMode},
    DataType, RemDb,
};

fn main() {
    println!("Testing CREATE TIMESERIES TABLE syntax...");
    println!("=========================================");

    // 1. 初始化内存分配器
    println!("Initializing memory allocator...");
    const MEMORY_SIZE: usize = 10 * 1024 * 1024; // 10MB
    static mut MEMORY: [u8; MEMORY_SIZE] = [0; MEMORY_SIZE];

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(MEMORY.as_mut_ptr(), MEMORY_SIZE)
            .expect("Failed to initialize memory allocator");
        println!(
            "Memory allocator initialized with {} MB",
            MEMORY_SIZE / 1024 / 1024
        );
    }

    // 2. 创建静态的数据库配置
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    static CONFIG: std::sync::LazyLock<DbConfig> = std::sync::LazyLock::new(|| DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024 * 10, // 10MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Sync,
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
        time_series_defaults: TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(HAConfig {
            node_id: 1, // 默认节点ID为1
            ha_role: HARole::Auto,
            replication_mode: ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
        }),
    });

    // 3. 创建数据库实例
    println!("Creating database instance...");
    let mut db = RemDb::new(&CONFIG);

    // 4. 初始化数据库
    println!("Initializing database...");
    db.init().expect("Failed to initialize database");

    // 测试1：创建简单的时序表
    println!("\n1. Testing: CREATE TIMESERIES TABLE with minimal schema");
    let create_table_sql1 = "CREATE TIMESERIES TABLE sensor_data (
        timestamp TIMESTAMP,
        value DOUBLE,
        sensor_id VARCHAR(50)
    )";

    match db.sql_query(create_table_sql1) {
        Ok(_result) => {
            println!("   ✓ CREATE TIMESERIES TABLE executed successfully");
            println!("   Result: Table created");
        }
        Err(e) => {
            println!("   ✗ CREATE TIMESERIES TABLE failed: {:?}", e);
        }
    }

    // 测试2：创建包含多个标签的时序表
    println!("\n2. Testing: CREATE TIMESERIES TABLE with multiple tags");
    let create_table_sql2 = "CREATE TIMESERIES TABLE device_metrics (
        ts TIMESTAMP,
        cpu_usage FLOAT,
        memory_usage FLOAT,
        device_id VARCHAR(50),
        location VARCHAR(100),
        active BOOLEAN
    )";

    match db.sql_query(create_table_sql2) {
        Ok(_result) => {
            println!("   ✓ CREATE TIMESERIES TABLE with multiple tags executed successfully");
            println!("   Result: Table created");
        }
        Err(e) => {
            println!(
                "   ✗ CREATE TIMESERIES TABLE with multiple tags failed: {:?}",
                e
            );
        }
    }

    // 测试3：插入数据到时序表
    println!("\n3. Testing: INSERT INTO timeseries table");
    let insert_sql = "INSERT INTO sensor_data (timestamp, value, sensor_id) VALUES (1609459200, 25.5, 'sensor_001')";

    match db.sql_query(insert_sql) {
        Ok(_result) => {
            println!("   ✓ INSERT INTO timeseries table executed successfully");
            println!("   Result: Record inserted");
        }
        Err(e) => {
            println!("   ✗ INSERT INTO timeseries table failed: {:?}", e);
        }
    }

    println!("\nAll tests completed!");
}
