//! ALTER TABLE 示例
//!
//! 该示例展示如何使用 RemDB 的 ALTER TABLE 功能：
//! - 添加新列
//! - 删除列
//! - 修改列
//! - 重命名列

use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig};
use remdb::{AlterTableOperation, DataType, DdlExecutor, FieldConstraint, RemDb, Result};

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

    println!("=== ALTER TABLE 示例 ===\n");

    // 1. 创建初始表
    println!("1. 创建初始表");
    db.sql_query("CREATE TABLE users (id INT32 PRIMARY KEY, name TEXT NOT NULL)")?;
    println!("   创建表: users (id, name)");

    db.sql_query("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Charlie')")?;
    println!("   插入测试数据");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   当前表结构:");
    println!("{}", result.to_string());

    // 2. 添加新列 - 使用 SQL
    println!("\n2. 添加新列 (SQL 方式)");
    db.sql_query("ALTER TABLE users ADD COLUMN age INT32")?;
    println!("   添加列: age (INT32)");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   添加列后的数据:");
    println!("{}", result.to_string());

    // 3. 添加带默认值的列
    println!("\n3. 添加带默认值的列");
    db.sql_query("ALTER TABLE users ADD COLUMN active BOOLEAN DEFAULT true")?;
    println!("   添加列: active (BOOLEAN, 默认值: true)");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   添加列后的数据:");
    println!("{}", result.to_string());

    // 4. 使用 API 添加列
    println!("\n4. 使用 API 添加列");
    let operation = AlterTableOperation::AddColumn {
        name: "email".to_string(),
        data_type: DataType::VarChar,
        size: 64,
        distance_type: None,
        default_value: None,
        constraints: FieldConstraint {
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
        },
    };
    db.alter_table("users", operation)?;
    println!("   通过 API 添加列: email (VARCHAR(64))");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   添加列后的数据:");
    println!("{}", result.to_string());

    // 5. 更新数据以填充新列
    println!("\n5. 更新数据");
    db.sql_query("UPDATE users SET age = 30, email = 'alice@example.com' WHERE id = 1")?;
    db.sql_query("UPDATE users SET age = 25, email = 'bob@example.com' WHERE id = 2")?;
    db.sql_query("UPDATE users SET age = 35, email = 'charlie@example.com' WHERE id = 3")?;
    println!("   更新 age 和 email 字段");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   更新后的数据:");
    println!("{}", result.to_string());

    // 6. 修改列
    println!("\n6. 修改列");
    db.sql_query("ALTER TABLE users MODIFY COLUMN name TEXT NOT NULL")?;
    println!("   修改 name 列为 NOT NULL");

    // 7. 重命名列
    println!("\n7. 重命名列");
    let operation = AlterTableOperation::RenameColumn {
        old_name: "email".to_string(),
        new_name: "email_address".to_string(),
    };
    db.alter_table("users", operation)?;
    println!("   重命名列: email -> email_address");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   重命名后的数据:");
    println!("{}", result.to_string());

    // 8. 删除列
    println!("\n8. 删除列");
    db.sql_query("ALTER TABLE users DROP COLUMN active")?;
    println!("   删除列: active");

    let result = db.sql_query("SELECT * FROM users")?;
    println!("\n   删除列后的数据:");
    println!("{}", result.to_string());

    // 9. 添加多个列
    println!("\n9. 连续添加多个列");
    db.sql_query("ALTER TABLE users ADD COLUMN created_at TIMESTAMP")?;
    db.sql_query("ALTER TABLE users ADD COLUMN updated_at TIMESTAMP")?;
    println!("   添加列: created_at, updated_at");

    let result =
        db.sql_query("SELECT id, name, age, email_address, created_at, updated_at FROM users")?;
    println!("\n   最终表结构:");
    println!("{}", result.to_string());

    // 10. 显示最终表结构
    println!("\n10. 显示表结构");
    let result = db.sql_query("DESCRIBE users")?;
    println!("   表结构信息:");
    println!("{}", result.to_string());

    println!("\n=== ALTER TABLE 示例完成 ===");
    Ok(())
}
