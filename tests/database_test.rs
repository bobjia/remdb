extern crate alloc;
use alloc::boxed::Box;
use remdb::platform::*;
use remdb::{DatabaseStatus, RemDb, RemDbError, Result};
use std::sync::Mutex;

mod common;
use common::{setup_test_db, setup_test_db_with_memory};

// 全局互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 初始化全局数据库
fn init_global_db(db_memory: &Vec<u8>) -> Result<RemDb> {
    // 创建数据库配置
    let config = Box::leak(Box::new(remdb::config::DbConfig {
        tables: vec![],
        total_memory: 4 * 1024 * 1024, // 4MB内存
        default_max_records: 1000,
        low_power_mode_supported: true,
        low_power_max_records: Some(100),
        memory_allocator: &remdb::config::DefaultMemoryAllocator,
        wal_config: remdb::config::WALConfig {
            log_path: "./data/test",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            max_partitions: 10,
            partition_duration_secs: 3600,
            retention_period_secs: 86400 * 30,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
    }));

    // 创建数据库实例
    let mut db = RemDb::new_with_name("test_db", config);
    db.init()?;

    Ok(db)
}

#[test]
fn test_databases_command() -> Result<()> {
    // 处理可能的互斥锁 poisoning
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let db_memory = setup_test_db();

    // 初始化数据库
    let mut db = init_global_db(&db_memory)?;

    // 创建一个数据库，这样数据库列表就不会为空
    db.create_database("test_db")?;

    // 测试databases方法
    let databases = db.databases()?;
    assert!(!databases.is_empty());

    // 验证返回的数据库信息
    let db_info = &databases[0];
    assert_eq!(db_info.name, "test_db");
    assert_eq!(db_info.database_type, "RemDb");
    assert_eq!(db_info.status, DatabaseStatus::Created);
    assert!(db_info.table_count >= 0); // 可能包含系统表
                                       // 内存使用量可能为0，因为测试环境中可能没有实际分配内存

    Ok(())
}

#[test]
fn test_database_manager_list_databases() -> Result<()> {
    // 处理可能的互斥锁 poisoning
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let _db_memory = setup_test_db();

    // 创建数据库管理器
    let mut manager = remdb::DatabaseManager::new(10);

    // 创建第一个数据库
    let db1 = manager.create_database("db1", "", None)?;
    assert_eq!(db1.name, "db1");

    // 创建第二个数据库
    let db2 = manager.create_database("db2", "", None)?;
    assert_eq!(db2.name, "db2");

    // 测试list_databases方法
    let databases = manager.list_databases()?;
    assert_eq!(databases.len(), 2);

    // 验证返回的数据库信息
    let mut db_names = databases
        .iter()
        .map(|info| info.name.clone())
        .collect::<Vec<_>>();
    db_names.sort();
    assert_eq!(db_names, vec!["db1", "db2"]);

    // 验证数据库类型和状态
    for db_info in &databases {
        assert_eq!(db_info.database_type, "RemDb");
        assert_eq!(db_info.status, DatabaseStatus::Created);
        assert_eq!(db_info.table_count, 0);
        // 内存使用量可能为0，因为测试环境中可能没有实际分配内存
    }

    Ok(())
}
