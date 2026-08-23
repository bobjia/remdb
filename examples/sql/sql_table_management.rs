//! SQL Table Management Example
//!
//! This example demonstrates RemDB's table management features:
//! - DROP TABLE
//! - DESCRIBE TABLE
//! - SHOW TABLES

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

    println!("=== SQL Table Management Example ===\n");

    // 1. Create test tables
    println!("1. Create test tables");
    db.sql_query("CREATE TABLE users (id INT32 PRIMARY KEY, name TEXT NOT NULL, email TEXT)")?;
    println!("   Created table: users");

    db.sql_query("CREATE TABLE products (id INT32 PRIMARY KEY, name TEXT, price REAL)")?;
    println!("   Created table: products");

    db.sql_query("CREATE TABLE orders (id INT32 PRIMARY KEY, user_id INT32, product_id INT32, quantity INT32)")?;
    println!("   Created table: orders");

    // 2. SHOW TABLES
    println!("\n2. SHOW TABLES");
    let result = db.sql_query("SHOW TABLES")?;
    println!("   Tables in database:");
    println!("{}", result.to_string());

    // 3. DESCRIBE TABLE
    println!("\n3. DESCRIBE TABLE");

    let result = db.sql_query("DESCRIBE users")?;
    println!("   users table structure:");
    println!("{}", result.to_string());

    // 4. Insert test data
    println!("\n4. Insert test data");
    db.sql_query("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')")?;
    db.sql_query("INSERT INTO users VALUES (2, 'Bob', 'bob@example.com')")?;
    println!("   Inserted test data");

    // 5. DROP TABLE
    println!("\n5. DROP TABLE");

    db.sql_query("DROP TABLE orders")?;
    println!("   Dropped table: orders");

    let result = db.sql_query("SHOW TABLES")?;
    println!("   Tables after drop:");
    println!("{}", result.to_string());

    // 6. DROP TABLE IF EXISTS
    println!("\n6. DROP TABLE IF EXISTS");

    db.sql_query("DROP TABLE IF EXISTS products")?;
    println!("   Dropped table: products (IF EXISTS)");

    // 7. Final table list
    println!("\n7. Final table list");
    let result = db.sql_query("SHOW TABLES")?;
    println!("   Final tables in database:");
    println!("{}", result.to_string());

    println!("\n=== SQL Table Management Example Complete ===");
    Ok(())
}
