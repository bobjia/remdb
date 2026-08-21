//! Benchmark the zero-copy request path: `handle_request` against an in-process
//! `SharedDb`, directly (no network). Bootstraps a real RemDb with the
//! feature-unified config and benches a single-row SELECT on ~100 rows.

use std::sync::{Arc, Mutex, Once};

use criterion::{criterion_group, criterion_main, Criterion};

use remdb_server::handler::{handle_request, SharedDb};
use remdb_server::pb;

static PLATFORM_INIT: Once = Once::new();
static ALLOC: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;

/// Bootstrap a `SharedDb` like the integration test: feature-unified config
/// (incl. `pubsub_config: None` and `ha_config: None`), posix platform, a table
/// with ~100 rows.
fn setup() -> SharedDb {
    PLATFORM_INIT.call_once(|| {
        remdb::platform::init_platform(remdb_platform_posix::get_posix_platform());
    });
    let config = Box::leak(Box::new(remdb::config::DbConfig {
        tables: &[],
        total_memory: 8 * 1024 * 1024,
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100_000,
        memory_allocator: &ALLOC,
        wal_config: remdb::config::WALConfig {
            log_path: "/tmp/remdb-srv-bench-wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60_000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 4 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
        },
        time_series_defaults: remdb::time_series::table::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 3600,
            compression: remdb::time_series::compression::CompressionType::None,
            max_partitions: 10,
        },
        pubsub_config: None,
        ha_config: None,
    }));
    assert!(remdb::config::validate_config(config));
    let mut db = remdb::RemDb::new(config);
    db.init().expect("init");
    let db: SharedDb = Arc::new(Mutex::new(db));

    {
        let mut g = db.lock().expect("lock db");
        let ddl = remdb::sql::parse_sql_query(
            "CREATE TABLE t (id INT PRIMARY KEY, name TEXT, score DOUBLE)",
        )
        .expect("parse ddl");
        remdb::sql::execute_query_raw(&mut g, &ddl).expect("create table");
        for i in 0..100 {
            let ins = remdb::sql::parse_sql_query(&format!(
                "INSERT INTO t (id, name, score) VALUES ({}, 'n{}', {}.0)",
                i, i, i
            ))
            .expect("parse insert");
            remdb::sql::execute_query_raw(&mut g, &ins).expect("insert");
        }
    }
    db
}

fn bench_single_row_query(c: &mut Criterion) {
    let db = setup();
    let req = pb::Request {
        request_id: 1,
        op: Some(pb::request::Op::Query(pb::QueryRequest {
            sql: "SELECT * FROM t WHERE id = 50".into(),
        })),
    };
    c.bench_function("single_row_query", |b| {
        b.iter(|| {
            let r = handle_request(&db, &req);
            criterion::black_box(r);
        })
    });
}

criterion_group!(benches, bench_single_row_query);
criterion_main!(benches);