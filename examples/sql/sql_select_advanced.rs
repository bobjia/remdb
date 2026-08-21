//! SQL Advanced Query Example
//!
//! This example demonstrates RemDB's advanced SQL query features:
//! - DISTINCT
//! - GROUP BY
//! - HAVING
//! - JOIN

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

    println!("=== SQL Advanced Query Example ===\n");

    // 1. Create test tables
    println!("1. Create test tables");
    db.sql_query("CREATE TABLE users (id INT32 PRIMARY KEY, name TEXT, department TEXT, salary REAL)")?;
    println!("   Created table: users");
    
    db.sql_query("CREATE TABLE orders (id INT32 PRIMARY KEY, user_id INT32, product TEXT, amount REAL)")?;
    println!("   Created table: orders");

    // 2. Insert test data
    println!("\n2. Insert test data");
    db.sql_query("INSERT INTO users VALUES (1, 'Alice', 'Engineering', 80000)")?;
    db.sql_query("INSERT INTO users VALUES (2, 'Bob', 'Engineering', 75000)")?;
    db.sql_query("INSERT INTO users VALUES (3, 'Charlie', 'Sales', 65000)")?;
    db.sql_query("INSERT INTO users VALUES (4, 'David', 'Sales', 70000)")?;
    db.sql_query("INSERT INTO users VALUES (5, 'Eve', 'Marketing', 60000)")?;
    db.sql_query("INSERT INTO users VALUES (6, 'Frank', 'Engineering', 85000)")?;
    println!("   Inserted 6 user records");
    
    db.sql_query("INSERT INTO orders VALUES (1, 1, 'Laptop', 1200)")?;
    db.sql_query("INSERT INTO orders VALUES (2, 1, 'Mouse', 50)")?;
    db.sql_query("INSERT INTO orders VALUES (3, 2, 'Keyboard', 100)")?;
    db.sql_query("INSERT INTO orders VALUES (4, 3, 'Monitor', 300)")?;
    db.sql_query("INSERT INTO orders VALUES (5, 3, 'Laptop', 1100)")?;
    db.sql_query("INSERT INTO orders VALUES (6, 4, 'Mouse', 45)")?;
    println!("   Inserted 6 order records");

    // 3. DISTINCT query
    println!("\n3. DISTINCT query");
    
    let result = db.sql_query("SELECT DISTINCT department FROM users")?;
    println!("   Distinct departments:");
    println!("{}", result.to_string());

    // 4. GROUP BY query
    println!("\n4. GROUP BY query");
    
    let result = db.sql_query("SELECT department, COUNT(*) AS count FROM users GROUP BY department")?;
    println!("   Count by department:");
    println!("{}", result.to_string());

    // 5. GROUP BY + multiple aggregate functions
    println!("\n5. GROUP BY + multiple aggregate functions");
    
    let result = db.sql_query(
        "SELECT department, COUNT(*) AS count, AVG(salary) AS avg_salary, MIN(salary) AS min_salary, MAX(salary) AS max_salary FROM users GROUP BY department"
    )?;
    println!("   Department summary:");
    println!("{}", result.to_string());

    // 6. HAVING clause
    println!("\n6. HAVING clause");
    
    let result = db.sql_query(
        "SELECT department, COUNT(*) AS count FROM users GROUP BY department HAVING count > 1"
    )?;
    println!("   Departments with more than 1 employee:");
    println!("{}", result.to_string());

    // 7. Column alias
    println!("\n7. Column alias");
    
    let result = db.sql_query("SELECT id AS user_id, name AS user_name, salary AS income FROM users")?;
    println!("   Using column aliases:");
    println!("{}", result.to_string());

    // 8. JOIN query
    println!("\n8. JOIN query");
    
    let result = db.sql_query(
        "SELECT users.id, users.name, orders.product, orders.amount FROM users INNER JOIN orders ON users.id = orders.user_id"
    )?;
    println!("   INNER JOIN (users with orders):");
    println!("{}", result.to_string());

    // 9. JOIN + WHERE
    println!("\n9. JOIN + WHERE");
    
    let result = db.sql_query(
        "SELECT users.name, orders.product, orders.amount FROM users INNER JOIN orders ON users.id = orders.user_id WHERE orders.amount > 100"
    )?;
    println!("   Orders with amount > 100:");
    println!("{}", result.to_string());

    // 10. Complex query
    println!("\n10. Complex query");
    
    let result = db.sql_query(
        "SELECT users.department, COUNT(*) AS order_count, SUM(orders.amount) AS total_amount FROM users INNER JOIN orders ON users.id = orders.user_id GROUP BY users.department ORDER BY total_amount DESC"
    )?;
    println!("   Order totals by department:");
    println!("{}", result.to_string());

    println!("\n=== SQL Advanced Query Example Complete ===");
    Ok(())
}
