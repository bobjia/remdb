//! SQL Transactions Example
//!
//! This example demonstrates RemDB's transaction features:
//! - BEGIN TRANSACTION
//! - COMMIT
//! - ROLLBACK

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

    println!("=== SQL Transactions Example ===\n");

    // 1. Create test table
    println!("1. Create test table");
    db.sql_query("CREATE TABLE accounts (id INT32 PRIMARY KEY, name TEXT, balance REAL)")?;
    println!("   Created table: accounts");

    // 2. Insert initial data
    println!("\n2. Insert initial data");
    db.sql_query("INSERT INTO accounts VALUES (1, 'Alice', 1000.00)")?;
    db.sql_query("INSERT INTO accounts VALUES (2, 'Bob', 500.00)")?;
    println!("   Alice initial balance: 1000.00");
    println!("   Bob initial balance: 500.00");

    // 3. View initial state
    println!("\n3. View initial state");
    let result = db.sql_query("SELECT * FROM accounts")?;
    println!("{}", result.to_string());

    // 4. Successful transaction - transfer
    println!("\n4. Successful transaction - transfer (Alice -> Bob: 200)");
    
    db.sql_query("BEGIN TRANSACTION")?;
    println!("   Started transaction");
    
    db.sql_query("UPDATE accounts SET balance = balance - 200 WHERE id = 1")?;
    println!("   Deducted 200 from Alice");
    
    db.sql_query("UPDATE accounts SET balance = balance + 200 WHERE id = 2")?;
    println!("   Added 200 to Bob");
    
    db.sql_query("COMMIT")?;
    println!("   Committed transaction");
    
    println!("\n   State after transfer:");
    let result = db.sql_query("SELECT * FROM accounts")?;
    println!("{}", result.to_string());

    // 5. Rollback transaction
    println!("\n5. Rollback transaction - simulate failure");
    
    db.sql_query("BEGIN TRANSACTION")?;
    println!("   Started transaction");
    
    db.sql_query("UPDATE accounts SET balance = balance - 500 WHERE id = 1")?;
    println!("   Deducted 500 from Alice (simulated)");
    
    db.sql_query("ROLLBACK")?;
    println!("   Rolled back transaction");
    
    println!("\n   State after rollback (unchanged):");
    let result = db.sql_query("SELECT * FROM accounts")?;
    println!("{}", result.to_string());

    // 6. Transaction with insert
    println!("\n6. Transaction with insert");
    
    db.sql_query("BEGIN TRANSACTION")?;
    println!("   Started transaction");
    
    db.sql_query("INSERT INTO accounts VALUES (3, 'Charlie', 750.00)")?;
    println!("   Inserted Charlie");
    
    db.sql_query("COMMIT")?;
    println!("   Committed transaction");
    
    let result = db.sql_query("SELECT * FROM accounts")?;
    println!("\n   State after insert:");
    println!("{}", result.to_string());

    println!("\n=== SQL Transactions Example Complete ===");
    Ok(())
}
