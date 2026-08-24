//! SQL Aggregate Functions Example
//!
//! This example demonstrates RemDB's aggregate function features:
//! - COUNT, SUM, AVG, MIN, MAX
//! - GROUP BY + aggregate functions

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

    println!("=== SQL Aggregate Functions Example ===\n");

    // 1. Create test table
    println!("1. Create test table");
    db.sql_query("CREATE TABLE sales (id INT32 PRIMARY KEY, product TEXT, category TEXT, quantity INT32, price REAL)")?;
    println!("   Created table: sales");

    // 2. Insert test data
    println!("\n2. Insert test data");
    db.sql_query("INSERT INTO sales VALUES (1, 'Laptop', 'Electronics', 5, 999.99)")?;
    db.sql_query("INSERT INTO sales VALUES (2, 'Mouse', 'Electronics', 20, 29.99)")?;
    db.sql_query("INSERT INTO sales VALUES (3, 'Keyboard', 'Electronics', 15, 79.99)")?;
    db.sql_query("INSERT INTO sales VALUES (4, 'Desk', 'Furniture', 3, 299.99)")?;
    db.sql_query("INSERT INTO sales VALUES (5, 'Chair', 'Furniture', 8, 149.99)")?;
    db.sql_query("INSERT INTO sales VALUES (6, 'Monitor', 'Electronics', 10, 399.99)")?;
    db.sql_query("INSERT INTO sales VALUES (7, 'Lamp', 'Furniture', 25, 49.99)")?;
    db.sql_query("INSERT INTO sales VALUES (8, 'Headphones', 'Electronics', 12, 149.99)")?;
    println!("   Inserted 8 sales records");

    // 3. COUNT function
    println!("\n3. COUNT function");

    let result = db.sql_query("SELECT COUNT(*) AS total_count FROM sales")?;
    println!("   Total records:");
    println!("{}", result.to_string());

    // 4. SUM function
    println!("\n4. SUM function");

    let result = db.sql_query("SELECT SUM(quantity) AS total_quantity FROM sales")?;
    println!("   Total quantity:");
    println!("{}", result.to_string());

    // 5. AVG function
    println!("\n5. AVG function");

    let result = db.sql_query("SELECT AVG(price) AS avg_price FROM sales")?;
    println!("   Average price:");
    println!("{}", result.to_string());

    // 6. MIN and MAX functions
    println!("\n6. MIN and MAX functions");

    let result =
        db.sql_query("SELECT MIN(price) AS min_price, MAX(price) AS max_price FROM sales")?;
    println!("   Min and max price:");
    println!("{}", result.to_string());

    // 7. Combined aggregate functions
    println!("\n7. Combined aggregate functions");

    let result = db.sql_query(
        "SELECT COUNT(*) AS count, SUM(quantity) AS total_qty, AVG(price) AS avg_price, MIN(price) AS min_price, MAX(price) AS max_price FROM sales"
    )?;
    println!("   Summary statistics:");
    println!("{}", result.to_string());

    // 8. GROUP BY + aggregate functions
    println!("\n8. GROUP BY + aggregate functions");

    let result = db.sql_query(
        "SELECT category, COUNT(*) AS count, SUM(quantity) AS total_qty, AVG(price) AS avg_price FROM sales GROUP BY category"
    )?;
    println!("   Statistics by category:");
    println!("{}", result.to_string());

    // 9. GROUP BY + ORDER BY
    println!("\n9. GROUP BY + ORDER BY");

    let result = db.sql_query(
        "SELECT category, SUM(quantity * price) AS total_revenue FROM sales GROUP BY category ORDER BY total_revenue DESC"
    )?;
    println!("   Revenue by category (sorted):");
    println!("{}", result.to_string());

    // 10. GROUP BY + HAVING
    println!("\n10. GROUP BY + HAVING");

    let result = db.sql_query(
        "SELECT category, COUNT(*) AS count FROM sales GROUP BY category HAVING count >= 3",
    )?;
    println!("   Categories with 3+ records:");
    println!("{}", result.to_string());

    println!("\n=== SQL Aggregate Functions Example Complete ===");
    Ok(())
}
