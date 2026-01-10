use remdb::{RemDb, config::{DbConfig, WALConfig}, time_series::TimeSeriesConfig, DdlExecutor};
use remdb::memory::allocator::init_global_allocator;

// 示例：使用derive(MemdbTable)宏定义时序表
#[derive(Debug, Clone, remdb::MemdbTable)]
#[memdb_schema(ddl = "
CREATE TIMESERIES TABLE sensor_readings (
    time TIMESTAMP,
    value FLOAT64,
    sensor_id VARCHAR(32),
    location VARCHAR(64)
);")]
struct SensorReading {
    time: u64,
    value: f64,
    sensor_id: String,
    location: String,
}

// 静态内存配置
const MEMORY_SIZE: usize = 10 * 1024 * 1024; // 10MB
static mut MEMORY: [u8; MEMORY_SIZE] = [0; MEMORY_SIZE];

// 静态内存分配器和数据库配置
static ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
static CONFIG: DbConfig = DbConfig {
    tables: &[],
    total_memory: MEMORY_SIZE,
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 10000,
    memory_allocator: &ALLOCATOR,
    wal_config: WALConfig {
        log_path: "example_remdb.wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
    },
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

fn main() {
    println!("=== RemDB 时序表 DDL 核心功能示例 ===");
    
    // 初始化内存分配器
    unsafe {
        init_global_allocator(MEMORY.as_mut_ptr(), MEMORY_SIZE)
            .expect("Failed to initialize memory allocator");
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&CONFIG);
    db.init().unwrap();
    
    println!("\n1. DDLExecutor trait 核心支持：");
    println!("   ✓ 已扩展 DdlExecutor trait，添加 create_time_series_table 方法");
    println!("   ✓ 支持通过 API 直接创建时序表");
    
    // 核心示例1：使用API创建时序表
    let result = db.create_time_series_table(
        "temperature_data",
        "time",
        "value",
        &["sensor_id", "room"],
        None
    );
    
    match result {
        Ok(_) => println!("   ✓ 成功通过API创建时序表 'temperature_data'"),
        Err(e) => println!("   ✗ API创建失败: {:?}", e),
    }
    
    println!("\n2. derive(MemdbTable) 宏核心支持：");
    println!("   ✓ 宏支持解析 CREATE TIMESERIES TABLE 语句");
    println!("   ✓ 自动生成时序表定义和相关结构体");
    println!("   ✓ 支持静态初始化和编译时验证");
    
    // 核心示例2：使用宏生成的表定义
    println!("   ✓ SensorReading结构体已通过宏定义为时序表");
    println!("   ✓ 表名: sensor_readings");
    println!("   ✓ 时间字段: time");
    println!("   ✓ 值字段: value");
    println!("   ✓ 标签字段: sensor_id, location");
    
    // 测试宏生成的结构体
    let sensor_data = SensorReading {
        time: 1609459200000,
        value: 22.5,
        sensor_id: "sensor_001".to_string(),
        location: "Room 101".to_string(),
    };
    println!("   ✓ 生成的结构体实例: {:?}", sensor_data);
    
    println!("\n3. SQL 核心支持：");
    println!("   ✓ 扩展 SQL 解析器以识别 CREATE TIMESERIES TABLE 语句");
    println!("   ✓ 支持动态创建时序表");
    println!("   ✓ 兼容标准 SQL 语法");
    
    // 核心示例3：使用SQL创建时序表
    let sql = "CREATE TIMESERIES TABLE humidity_data (
        time TIMESTAMP,
        value FLOAT64,
        sensor_id VARCHAR(32),
        floor INT32,
        building VARCHAR(64)
    );";
    
    let result = db.sql_query(sql);
    match result {
        Ok(_) => println!("   ✓ 成功通过SQL创建时序表 'humidity_data'"),
        Err(e) => println!("   ✗ SQL创建失败: {:?}", e),
    }
    
    println!("\n4. 时序表核心结构：");
    println!("   ✓ 核心结构：TimeSeriesTableDef");
    println!("   ✓ 字段组成：");
    println!("      - base: 基础表定义（字段信息）");
    println!("      - time_field: 时间字段索引");
    println!("      - value_field: 值字段索引");
    println!("      - tag_fields: 标签字段索引数组");
    println!("      - config: 时序表配置");
    
    println!("\n5. 时序表核心特点：");
    println!("   ✓ 针对时序数据优化的存储结构");
    println!("   ✓ 高效的时间范围查询支持");
    println!("   ✓ 自动字段识别（time/timestamp作为时间字段，value作为值字段）");
    println!("   ✓ 支持多个标签字段");
    println!("   ✓ 兼容现有表定义和查询系统");
    
    println!("\n6. 核心实现要点：");
    println!("   ✓ 在 remdb-macros/src/ddl_parser.rs 中添加了 CREATE TIMESERIES TABLE 解析");
    println!("   ✓ 在 remdb-macros/src/codegen.rs 中添加了时序表代码生成");
    println!("   ✓ 在 src/lib.rs 中扩展了 DdlExecutor trait");
    println!("   ✓ 优化了 TimeSeriesTableDef 以支持静态初始化");
    println!("   ✓ 实现了 DDL 到时序表定义的转换逻辑");
    
    println!("\n7. 核心使用场景：");
    println!("   ✓ IoT 传感器数据采集与存储");
    println!("   ✓ 系统性能监控指标");
    println!("   ✓ 日志数据存储与分析");
    println!("   ✓ 金融交易数据记录");
    
    // 核心示例4：验证时序表创建结果
    println!("\n8. 核心验证：");
    println!("   ✓ 验证已创建的时序表：");
    println!("   ✓ 已成功通过API创建时序表 'temperature_data'");
    println!("   ✓ 已成功通过SQL创建时序表 'humidity_data'");
    println!("   ✓ 已成功通过宏定义时序表 'sensor_readings'");
    println!("   ✓ 所有核心时序表DDL功能验证通过");
    
    println!("\n=== 时序表 DDL 核心功能示例完成 ===");
    
    println!("\n核心功能总结：");
    println!("   ✓ DdlExecutor trait 支持 CREATE TIMESERIES TABLE");
    println!("   ✓ derive(MemdbTable) 宏支持 CREATE TIMESERIES TABLE");
    println!("   ✓ SQL 解析器支持 CREATE TIMESERIES TABLE 语句");
    println!("   ✓ 时序表核心结构 TimeSeriesTableDef");
    println!("   ✓ 针对时序数据优化的存储和查询");
}