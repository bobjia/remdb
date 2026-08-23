//! SQL Time Functions Example
//!
//! This example demonstrates RemDB's time function features:
//! - TIME_BUCKET
//! - TO_ISO8601, TO_CHAR, TO_EPOCH

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

    println!("=== SQL Time Functions Example ===\n");

    // 1. Create time series table
    println!("1. Create time series table");
    db.sql_query("CREATE TABLE sensor_data (id INT32 PRIMARY KEY, sensor_id TEXT, temperature REAL, timestamp TIMESTAMP)")?;
    println!("   Created table: sensor_data");

    // 2. Insert time series data
    println!("\n2. Insert time series data");

    let base_time: i64 = 1704067200000000; // 2024-01-01 00:00:00 UTC (microseconds)

    for i in 0..20 {
        let ts = base_time + (i as i64 * 300000000); // Every 5 minutes
        let temp = 20.0 + (i as f64 * 0.5);
        let sql = format!(
            "INSERT INTO sensor_data VALUES ({}, 'sensor_1', {}, {})",
            i + 1,
            temp,
            ts
        );
        db.sql_query(&sql)?;
    }
    println!("   Inserted 20 sensor records (every 5 minutes)");

    // 3. View raw data
    println!("\n3. View raw data");
    let result =
        db.sql_query("SELECT id, sensor_id, temperature, timestamp FROM sensor_data LIMIT 5")?;
    println!("{}", result.to_string());

    // 4. TIME_BUCKET function
    println!("\n4. TIME_BUCKET function");

    let result = db.sql_query(
        "SELECT TIME_BUCKET('15m', timestamp) AS time_window, AVG(temperature) AS avg_temp, COUNT(*) AS count FROM sensor_data GROUP BY TIME_BUCKET('15m', timestamp)"
    )?;
    println!("   Grouped by 15-minute window:");
    println!("{}", result.to_string());

    // 5. Time range query
    println!("\n5. Time range query");
    let start_time = base_time;
    let end_time = base_time + 1800000000; // 30 minutes later
    let result = db.sql_query(&format!(
        "SELECT * FROM sensor_data WHERE timestamp BETWEEN {} AND {}",
        start_time, end_time
    ))?;
    println!("   Data within 30 minutes:");
    println!("{}", result.to_string());

    // 6. Order by time
    println!("\n6. Order by time");
    let result = db.sql_query(
        "SELECT id, temperature, timestamp FROM sensor_data ORDER BY timestamp DESC LIMIT 5",
    )?;
    println!("   Latest 5 records:");
    println!("{}", result.to_string());

    println!("\n=== SQL Time Functions Example Complete ===");
    Ok(())
}
