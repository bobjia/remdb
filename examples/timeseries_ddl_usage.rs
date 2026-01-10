use remdb::{RemDb, config::DbConfig, time_series::TimeSeriesConfig, DdlExecutor};
use remdb::memory::allocator::init_global_allocator;

// 示例1：使用derive(MemdbTable)宏定义时序表
#[derive(Debug, Clone, remdb::MemdbTable)]
#[memdb_schema(ddl = "
CREATE TIMESERIES TABLE sensor_readings (
    time TIMESTAMP,
    value FLOAT64,
    sensor_id VARCHAR(32),
    location VARCHAR(64),
    unit VARCHAR(16)
);")]
struct SensorReading {
    time: u64,
    value: f64,
    sensor_id: String,
    location: String,
    unit: String,
}

// 静态内存分配器
static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;

// 静态数据库配置
static mut DB_CONFIG: Option<DbConfig> = None;

fn main() {
    // 初始化内存分配器 - 使用堆分配避免栈溢出
    let mut memory = vec![0u8; 1024 * 1024 * 32];
    init_global_allocator(memory.as_mut_ptr(), memory.len()).unwrap();
    
    // 创建并存储数据库配置
    let config = DbConfig {
        tables: &[],
        total_memory: memory.len(),
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: unsafe { &DEFAULT_ALLOCATOR },
        log_path: "timeseries_ddl_usage.wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        time_series_defaults: TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(remdb::config::HAConfig {
            ha_role: remdb::ha::HARole::Auto,
            replication_mode: remdb::ha::ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
            heartbeat_port: 5557,
        }),
    };
    
    unsafe {
        DB_CONFIG = Some(config);
    }
    
    // 创建数据库实例
    let mut db = unsafe {
        RemDb::new(DB_CONFIG.as_ref().unwrap())
    };
    db.init().unwrap();
    
    println!("=== RemDB 时序表 DDL 使用示例 ===");
    
    // 示例2：使用DdlExecutor trait的create_time_series_table方法
    println!("\n1. 使用DdlExecutor trait创建时序表：");
    let result = db.create_time_series_table(
        "temperature_data",
        "time",
        "value",
        &["sensor_id", "room"],
        None
    );
    
    match result {
        Ok(_) => println!("✓ 成功创建时序表 'temperature_data'"),
        Err(e) => println!("✗ 创建失败: {:?}", e),
    }
    
    // 示例3：使用SQL CREATE TIMESERIES TABLE语句
    println!("\n2. 使用SQL CREATE TIMESERIES TABLE语句：");
    let sql = "CREATE TIMESERIES TABLE humidity_data (\n    time TIMESTAMP,\n    value FLOAT64,\n    sensor_id VARCHAR(32),\n    floor INT32,\n    building VARCHAR(64)\n);";
    
    match db.sql_query(sql) {
        Ok(_) => println!("✓ 成功创建时序表 'humidity_data'"),
        Err(e) => println!("✗ SQL执行失败: {:?}", e),
    }
    
    // 示例4：使用derive(MemdbTable)宏生成的表定义
    println!("\n3. 使用derive(MemdbTable)宏：");
    println!("✓ SensorReading结构体已通过宏定义为时序表");
    println!("   - 表名: sensor_readings");
    println!("   - 时间字段: time");
    println!("   - 值字段: value");
    println!("   - 标签字段: sensor_id, location, unit");
    
    // 示例5：插入和查询时序数据（演示）
    println!("\n4. 插入和查询时序数据：");
    
    // 插入示例数据
    let insert_sql = "INSERT INTO humidity_data (time, value, sensor_id, floor, building) VALUES \
                      (1609459200000, 45.5, 'hum_sensor_001', 1, 'Building A'), \
                      (1609459260000, 46.2, 'hum_sensor_001', 1, 'Building A'), \
                      (1609459320000, 45.8, 'hum_sensor_001', 1, 'Building A');";
    
    match db.sql_query(insert_sql) {
        Ok(_) => println!("✓ 成功插入3条时序数据"),
        Err(e) => println!("✗ 插入失败: {:?}", e),
    }
    
    // 查询示例数据
    let select_sql = "SELECT time, value, sensor_id FROM humidity_data WHERE floor = 1 ORDER BY time DESC LIMIT 2;";
    
    match db.sql_query(select_sql) {
        Ok(result_set) => {
            println!("✓ 成功查询时序数据，返回 {} 行", result_set.rows.len());
            for (i, row) in result_set.rows.iter().enumerate() {
                println!("   行 {}: {:?}", i + 1, row.values);
            }
        },
        Err(e) => println!("✗ 查询失败: {:?}", e),
    }
    
    println!("\n=== 使用示例完成 ===");
    println!("\n支持的时序表DDL语法:");
    println!("  CREATE TIMESERIES TABLE table_name (");
    println!("      time TIMESTAMP,");
    println!("      value FLOAT64,");
    println!("      tag1 VARCHAR(32),");
    println!("      tag2 INT32");
    println!("  );");
    
    println!("\n时序表特点:");
    println!("  - 必须包含时间字段（通常命名为time或timestamp）");
    println!("  - 必须包含值字段（通常命名为value）");
    println!("  - 可以包含多个标签字段");
    println!("  - 自动优化时序数据存储和查询");
    println!("  - 支持高效的时间范围查询");
}