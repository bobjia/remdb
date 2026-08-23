//! 多数据库管理示例
//!
//! 该示例展示如何使用 RemDB 的多数据库管理功能：
//! - 创建多个数据库
//! - 切换数据库
//! - 跨数据库操作
//! - 关闭和删除数据库

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::{RemDb, Result};

static mut DB_MEMORY: [u8; 8 * 1024 * 1024] = [0; 8 * 1024 * 1024];

static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;
    }

    let config = Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 8 * 1024 * 1024,
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
        model_worker_config: Default::default(),
    }));

    let mut db = RemDb::new(config);
    db.init()?;

    println!("=== 多数据库管理示例 ===\n");

    // 1. 显示当前数据库状态
    println!("1. 当前数据库状态");
    println!("   数据库名称: {}", db.name);
    println!("   数据库状态: {:?}", db.status);

    // 2. 创建新数据库
    println!("\n2. 创建新数据库");
    match db.create_database("sales_db") {
        Ok(_) => println!("   创建数据库: sales_db 成功"),
        Err(e) => println!("   创建数据库失败: {:?}", e),
    }
    match db.create_database("inventory_db") {
        Ok(_) => println!("   创建数据库: inventory_db 成功"),
        Err(e) => println!("   创建数据库失败: {:?}", e),
    }
    match db.create_database("analytics_db") {
        Ok(_) => println!("   创建数据库: analytics_db 成功"),
        Err(e) => println!("   创建数据库失败: {:?}", e),
    }

    // 3. 列出所有数据库
    println!("\n3. 列出所有数据库");
    match db.databases() {
        Ok(databases) => {
            println!("   数据库列表:");
            for info in &databases {
                println!("   - {} (状态: {:?})", info.name, info.status);
            }
        }
        Err(e) => println!("   获取数据库列表失败: {:?}", e),
    }

    // 4. 在当前数据库中创建表
    println!("\n4. 在当前数据库中创建表");
    db.sql_query("CREATE TABLE orders (id INT32 PRIMARY KEY, product TEXT, amount REAL)")?;
    println!("   创建表: orders");

    db.sql_query("INSERT INTO orders VALUES (1, 'Laptop', 999.99), (2, 'Phone', 599.99)")?;
    println!("   插入数据");

    // 5. 查询数据
    println!("\n5. 查询数据");
    let result = db.sql_query("SELECT * FROM orders")?;
    println!("   orders 表数据:");
    println!("{}", result.to_string());

    // 6. 尝试切换数据库
    println!("\n6. 尝试切换数据库");
    match db.use_database("sales_db") {
        Ok(_) => {
            println!("   切换到 sales_db 成功");

            // 在 sales_db 中创建表
            db.sql_query("CREATE TABLE sales (id INT32 PRIMARY KEY, amount REAL)")?;
            println!("   在 sales_db 中创建表: sales");
        }
        Err(e) => println!("   切换数据库失败: {:?}", e),
    }

    // 7. 关闭数据库
    println!("\n7. 关闭数据库");
    match db.close_database("analytics_db") {
        Ok(_) => println!("   关闭数据库: analytics_db 成功"),
        Err(e) => println!("   关闭数据库失败: {:?}", e),
    }

    // 8. 删除数据库
    println!("\n8. 删除数据库");
    match db.drop_database("analytics_db") {
        Ok(_) => println!("   删除数据库: analytics_db 成功"),
        Err(e) => println!("   删除数据库失败: {:?}", e),
    }

    // 9. 最终数据库列表
    println!("\n9. 最终数据库列表");
    match db.databases() {
        Ok(databases) => {
            for info in &databases {
                println!(
                    "   - {} (状态: {:?}, 表数量: {}, 内存: {} bytes)",
                    info.name, info.status, info.table_count, info.memory_usage
                );
            }
        }
        Err(e) => println!("   获取数据库列表失败: {:?}", e),
    }

    println!("\n=== 多数据库管理示例完成 ===");
    Ok(())
}
