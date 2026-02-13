//! SQL JSON Functions Example
//!
//! This example demonstrates RemDB's JSON function features:
//! - JSON field definition
//! - JSON_EXTRACT
//! - JSON path queries

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

    println!("=== SQL JSON Functions Example ===\n");

    // 1. Create table with JSON field
    println!("1. Create table with JSON field");
    db.sql_query("CREATE TABLE users (id INT32 PRIMARY KEY, name TEXT, profile JSON)")?;
    println!("   Created table: users (with JSON field)");
    
    db.sql_query("CREATE TABLE products (id INT32 PRIMARY KEY, name TEXT, attributes JSON)")?;
    println!("   Created table: products (with JSON field)");

    // 2. Insert JSON data
    println!("\n2. Insert JSON data");
    
    db.sql_query(r#"INSERT INTO users VALUES (1, 'Alice', '{"age": 30, "city": "Beijing", "skills": ["Rust", "Python"], "active": true}')"#)?;
    db.sql_query(r#"INSERT INTO users VALUES (2, 'Bob', '{"age": 25, "city": "Shanghai", "skills": ["Java", "Go"], "active": false}')"#)?;
    db.sql_query(r#"INSERT INTO users VALUES (3, 'Charlie', '{"age": 35, "city": "Guangzhou", "skills": ["C++", "Rust"], "address": {"street": "Main St", "zip": "10001"}}')"#)?;
    println!("   Inserted 3 user records");
    
    db.sql_query(r#"INSERT INTO products VALUES (1, 'Laptop', '{"brand": "Dell", "specs": {"cpu": "i7", "ram": 16}, "price": 999.99}')"#)?;
    db.sql_query(r#"INSERT INTO products VALUES (2, 'Phone', '{"brand": "Apple", "specs": {"cpu": "A15", "ram": 8}, "price": 799.99}')"#)?;
    println!("   Inserted 2 product records");

    // 3. View raw data
    println!("\n3. View raw data");
    let result = db.sql_query("SELECT * FROM users")?;
    println!("{}", result.to_string());

    // 4. JSON_EXTRACT
    println!("\n4. JSON_EXTRACT");
    
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(profile, '$.age') AS age, JSON_EXTRACT(profile, '$.city') AS city FROM users"#)?;
    println!("   Extract age and city:");
    println!("{}", result.to_string());

    // 5. Extract nested value
    println!("\n5. Extract nested value");
    
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(profile, '$.address.street') AS street FROM users WHERE id = 3"#)?;
    println!("   Extract nested address.street:");
    println!("{}", result.to_string());

    // 6. Extract array element
    println!("\n6. Extract array element");
    
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(profile, '$.skills[0]') AS first_skill FROM users"#)?;
    println!("   Extract first skill:");
    println!("{}", result.to_string());

    // 7. JSON condition query
    println!("\n7. JSON condition query");
    
    let result = db.sql_query(r#"SELECT id, name FROM users WHERE JSON_EXTRACT(profile, '$.age') > 28"#)?;
    println!("   Users with age > 28:");
    println!("{}", result.to_string());

    // 8. Product JSON query
    println!("\n8. Product JSON query");
    
    let result = db.sql_query(r#"SELECT id, name, JSON_EXTRACT(attributes, '$.brand') AS brand, JSON_EXTRACT(attributes, '$.specs.cpu') AS cpu FROM products"#)?;
    println!("   Product brand and CPU:");
    println!("{}", result.to_string());

    println!("\n=== SQL JSON Functions Example Complete ===");
    Ok(())
}
