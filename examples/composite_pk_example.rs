extern crate alloc;
use remdb::{DataType, RemDb, Result};

/// 定义静态内存缓冲区
static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024]; // 1MB内存缓冲区

/// 定义静态数据库配置
static DB_CONFIG: remdb::config::DbConfig = remdb::config::DbConfig {
    tables: vec![],
    total_memory: 1024 * 1024, // 1MB
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 1000,
    memory_allocator: unsafe {
        // 使用静态DEFAULT_ALLOCATOR
        static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
        &mut DEFAULT_ALLOCATOR
    },
    wal_config: remdb::config::WALConfig {
        log_path: "wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
    },
    time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,
};

/// 复合主键示例
fn main() -> Result<()> {
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init()?;
    
    println!("=== 复合主键示例 ===");
    
    // 创建带有复合主键的表
    let fields = [
        ("device_id", DataType::UInt32, 0, None, None),
        ("metric_id", DataType::UInt32, 0, None, None),
        ("timestamp", DataType::UInt64, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    db.create_table("metrics", &fields, None)?;
    println!("成功创建带有复合主键的表: metrics (device_id, metric_id, timestamp)");
    
    // 获取表
    let table_id = 1; // 系统表占用0，新表ID为1
    let table = db.get_table(table_id)?;
    
    println!("表结构:");
    println!("  字段数: {}", table.def.fields.len());
    println!("  复合主键字段索引: {:?}", table.def.primary_key);
    
    for (i, field) in table.def.fields.iter().enumerate() {
        println!("  字段 {}: {} ({:?})", i, field.name, field.data_type);
    }
    
    println!("\n=== 示例完成 ===");
    Ok(())
}
