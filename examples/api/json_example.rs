//! JSON 字段示例
//!
//! 该示例展示如何使用 RemDB 的 JSON 功能：
//! - 创建包含 JSON 字段的表
//! - 插入 JSON 数据
//! - JSON 路径查询
//! - JSON 函数操作

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
    }));

    let mut db = RemDb::new(config);
    db.init()?;

    println!("=== JSON 字段示例 ===\n");

    // 1. 创建包含 JSON 字段的表
    println!("1. 创建包含 JSON 字段的表");
    let create_sql = r#"
        CREATE TABLE users (
            id INT32 PRIMARY KEY,
            name TEXT NOT NULL,
            profile JSON,
            settings JSON
        )
    "#;
    db.sql_query(create_sql)?;
    println!("   创建表: users (包含 profile 和 settings JSON 字段)");

    // 2. 插入包含 JSON 数据的记录
    println!("\n2. 插入包含 JSON 数据的记录");
    
    let insert_sql = r#"
        INSERT INTO users (id, name, profile, settings) VALUES 
        (1, 'Alice', '{"age": 30, "city": "Beijing", "skills": ["Rust", "Python"]}', '{"theme": "dark", "notifications": true}'),
        (2, 'Bob', '{"age": 25, "city": "Shanghai", "skills": ["Java", "Go"]}', '{"theme": "light", "notifications": false}'),
        (3, 'Charlie', '{"age": 35, "city": "Guangzhou", "skills": ["C++", "Rust", "Python"]}', '{"theme": "dark", "notifications": true}')
    "#;
    db.sql_query(insert_sql)?;
    println!("   插入 3 条包含 JSON 数据的记录");

    // 3. 查询 JSON 字段
    println!("\n3. 查询 JSON 字段");
    let result = db.sql_query("SELECT id, name, profile FROM users")?;
    println!("   查询结果:");
    println!("{}", result.to_string());

    // 4. JSON 路径查询 - 提取特定字段
    println!("\n4. JSON 路径查询");
    
    // 使用 JSON 提取函数
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(profile, '$.city') AS city FROM users"#)?;
    println!("   提取城市信息:");
    println!("{}", result.to_string());

    // 5. JSON 条件查询
    println!("\n5. JSON 条件查询");
    
    // 查询年龄大于 30 的用户
    let result = db.sql_query(r#"SELECT id, name, profile FROM users WHERE JSON_EXTRACT(profile, '$.age') > 30"#)?;
    println!("   年龄大于 30 的用户:");
    println!("{}", result.to_string());

    // 6. JSON 数组操作
    println!("\n6. JSON 数组操作");
    
    // 查询包含特定技能的用户
    let result = db.sql_query(r#"SELECT id, name FROM users WHERE JSON_CONTAINS(JSON_EXTRACT(profile, '$.skills'), '"Rust"')"#)?;
    println!("   拥有 Rust 技能的用户:");
    println!("{}", result.to_string());

    // 7. 更新 JSON 字段
    println!("\n7. 更新 JSON 字段");
    db.sql_query(r#"UPDATE users SET profile = '{"age": 31, "city": "Shenzhen", "skills": ["Rust", "Python", "Go"]}' WHERE id = 1"#)?;
    println!("   更新 Alice 的 profile");
    
    let result = db.sql_query("SELECT id, name, profile FROM users WHERE id = 1")?;
    println!("   更新后的数据:");
    println!("{}", result.to_string());

    // 8. JSON 函数示例
    println!("\n8. JSON 函数示例");
    
    // JSON_KEYS - 获取 JSON 对象的所有键
    let result = db.sql_query(r#"SELECT id, name, JSON_KEYS(profile) AS profile_keys FROM users WHERE id = 1"#)?;
    println!("   profile 字段的所有键:");
    println!("{}", result.to_string());

    // JSON_TYPE - 获取 JSON 值的类型
    let result = db.sql_query(r#"SELECT id, JSON_TYPE(JSON_EXTRACT(profile, '$.skills')) AS skills_type FROM users WHERE id = 1"#)?;
    println!("   skills 字段的类型:");
    println!("{}", result.to_string());

    // 9. 嵌套 JSON 查询
    println!("\n9. 嵌套 JSON 查询");
    
    // 插入嵌套 JSON 数据
    let insert_nested = r#"
        INSERT INTO users (id, name, profile, settings) VALUES 
        (4, 'David', '{"age": 28, "address": {"city": "Hangzhou", "zip": "310000"}, "contacts": {"email": "david@example.com", "phone": "1234567890"}}', '{"theme": "auto", "privacy": {"shareData": false}}')
    "#;
    db.sql_query(insert_nested)?;
    println!("   插入嵌套 JSON 数据");
    
    // 查询嵌套字段
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(profile, '$.address.city') AS city FROM users WHERE id = 4"#)?;
    println!("   查询嵌套城市字段:");
    println!("{}", result.to_string());

    // 10. JSON 聚合
    println!("\n10. JSON 聚合统计");
    
    // 统计不同主题设置的用户数
    let result = db.sql_query(r#"SELECT JSON_EXTRACT(settings, '$.theme') AS theme, COUNT(*) AS count FROM users GROUP BY JSON_EXTRACT(settings, '$.theme')"#)?;
    println!("   按主题设置统计用户数:");
    println!("{}", result.to_string());

    println!("\n=== JSON 字段示例完成 ===");
    Ok(())
}
