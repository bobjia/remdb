//! SQL LIKE Operator Example
//!
//! This example demonstrates RemDB's LIKE operator features:
//! - % wildcard (matches any length of characters)
//! - _ wildcard (matches single character)
//! - NOT LIKE

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

    println!("=== SQL LIKE Operator Example ===\n");

    // 1. Create test table
    println!("1. Create test table");
    db.sql_query("CREATE TABLE users (id INT32 PRIMARY KEY, name TEXT, email TEXT, phone TEXT)")?;
    println!("   Created table: users");

    // 2. Insert test data
    println!("\n2. Insert test data");

    db.sql_query("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com', '13812345678')")?;
    db.sql_query("INSERT INTO users VALUES (2, 'Bob', 'bob@test.org', '13987654321')")?;
    db.sql_query("INSERT INTO users VALUES (3, 'Charlie', 'charlie@example.com', '13611112222')")?;
    db.sql_query("INSERT INTO users VALUES (4, 'David', 'david@example.org', '13733334444')")?;
    db.sql_query("INSERT INTO users VALUES (5, 'Eve', 'eve@test.com', '13855556666')")?;
    db.sql_query("INSERT INTO users VALUES (6, 'Frank', 'frank@demo.net', '13977778888')")?;
    println!("   Inserted 6 user records");

    // 3. % wildcard - prefix match
    println!("\n3. % wildcard - prefix match");

    let result = db.sql_query("SELECT id, name FROM users WHERE name LIKE 'A%'")?;
    println!("   Names starting with 'A':");
    println!("{}", result.to_string());

    // 4. % wildcard - suffix match
    println!("\n4. % wildcard - suffix match");

    let result = db.sql_query("SELECT id, name, email FROM users WHERE email LIKE '%.com'")?;
    println!("   Emails ending with .com:");
    println!("{}", result.to_string());

    // 5. % wildcard - contains match
    println!("\n5. % wildcard - contains match");

    let result = db.sql_query("SELECT id, name, email FROM users WHERE email LIKE '%example%'")?;
    println!("   Emails containing 'example':");
    println!("{}", result.to_string());

    // 6. _ wildcard - single character match
    println!("\n6. _ wildcard - single character match");

    let result = db.sql_query("SELECT id, name FROM users WHERE name LIKE '_ob'")?;
    println!("   3-character names ending with 'ob':");
    println!("{}", result.to_string());

    // 7. Combined wildcards
    println!("\n7. Combined wildcards");

    let result = db.sql_query("SELECT id, name FROM users WHERE name LIKE 'A_i%'")?;
    println!("   Names starting with 'A' and third char is 'i':");
    println!("{}", result.to_string());

    // 8. NOT LIKE
    println!("\n8. NOT LIKE");

    let result =
        db.sql_query("SELECT id, name, email FROM users WHERE email NOT LIKE '%example%'")?;
    println!("   Emails NOT containing 'example':");
    println!("{}", result.to_string());

    // 9. LIKE with AND/OR
    println!("\n9. LIKE with AND/OR");

    let result = db.sql_query(
        "SELECT id, name, email FROM users WHERE email LIKE '%.com' OR email LIKE '%.org'",
    )?;
    println!("   Emails ending with .com or .org:");
    println!("{}", result.to_string());

    println!("\n=== SQL LIKE Operator Example Complete ===");
    Ok(())
}
