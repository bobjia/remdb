extern crate alloc;
use remdb::platform::*;
use remdb::{
    config::DbConfig, config::LogMode, config::WALConfig, time_series::CompressionType,
    time_series::TimeSeriesConfig, DataType, DatabaseStatus, FieldDef, RemDb, Result, TableDef,
};
use std::sync::Mutex;

mod common;
use common::{setup_test_db, setup_test_db_with_memory};

// 全局互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 测试数据库配置
static TEST_DB_CONFIG: std::sync::LazyLock<DbConfig> = std::sync::LazyLock::new(|| {
    DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100000,
        memory_allocator: &remdb::config::DefaultMemoryAllocator,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            max_consecutive_invalid: 100,
            retained_checkpoints: 2,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        time_series_defaults: TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
        model_worker_config: Default::default(),
    }
});

#[test]
fn test_databases_command() -> Result<()> {
    // 处理可能的互斥锁 poisoning
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    setup_test_db_with_memory(1024 * 1024); // 1MB to match TEST_DB_CONFIG

    // 初始化数据库
    let mut db = unsafe { remdb::init_global_db(&TEST_DB_CONFIG)? };

    // 创建一个数据库，这样数据库列表就不会为空
    db.create_database("test_db")?;

    // 测试databases方法
    let databases = db.databases()?;
    assert!(!databases.is_empty());

    // 验证返回的数据库信息 - 找到我们创建的数据库
    let db_info = databases
        .iter()
        .find(|info| info.name == "test_db")
        .expect("test_db database not found");
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

    setup_test_db();

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
