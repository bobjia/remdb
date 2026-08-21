#![allow(unsafe_code)]

use remdb::config::{DefaultMemoryAllocator, LogMode, WALConfig};
use remdb::time_series::compression::CompressionType;
use remdb::time_series::TimeSeriesConfig;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

static DEFAULT_ALLOC: DefaultMemoryAllocator = DefaultMemoryAllocator;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bind = args.get(1).map(|s| s.as_str()).unwrap_or("127.0.0.1:9999");

    // The posix platform must be registered before `RemDb::init()`, which
    // opens the WAL file through the platform abstraction.
    remdb::platform::init_platform(remdb_platform_posix::get_posix_platform());

    let config = remdb::config::DbConfig {
        tables: &[],
        total_memory: 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100_000,
        memory_allocator: &DEFAULT_ALLOC,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Async,
            checkpoint_interval_ms: 60_000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 4 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
        },
        time_series_defaults: TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 7 * 24 * 3600,
            compression: CompressionType::None,
            max_partitions: 100,
        },
        pubsub_config: None,
        ha_config: None,
    };

    if !remdb::config::validate_config(&config) {
        eprintln!("invalid database config");
        std::process::exit(2);
    }

    let config = Box::leak(Box::new(config));
    let mut db = remdb::RemDb::new(config);
    db.init().expect("failed to init database");

    let db = Arc::new(Mutex::new(db));
    let pubsub = Arc::new(remdb_server::pubsub::PubSubManager::new());

    let listener = TcpListener::bind(&bind).expect("failed to bind");
    println!("listening on {}", bind);

    let _ = remdb_server::server::serve(listener, db, pubsub);
}