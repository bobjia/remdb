use remdb::config::WALConfig;
use remdb::platform::*;
use remdb::transaction::*;
use remdb::types::*;
use remdb::*;
use serial_test::serial;

mod common;
use common::{setup_test_db, setup_test_db_with_memory};

// 简单的表定义用于测试
static TEST_TABLE_DEF: std::sync::LazyLock<TableDef> = std::sync::LazyLock::new(|| TableDef {
    id: 0,
    name: "test_table".to_string(),
    fields: vec![
        FieldDef {
            name: "id".to_string(),
            data_type: DataType::UInt32,
            size: 4,
            string_length: None,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
        FieldDef {
            name: "value".to_string(),
            data_type: DataType::Float32,
            size: 4,
            string_length: None,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
            json_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 8,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

// 静态内存分配器实例
static DEFAULT_ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;

// 数据库配置
static TEST_DB_CONFIG: std::sync::LazyLock<config::DbConfig> =
    std::sync::LazyLock::new(|| config::DbConfig {
        tables: vec![TEST_TABLE_DEF.clone()],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100000,
        memory_allocator: &DEFAULT_ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
        },
        time_series_defaults: time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(config::HAConfig {
            node_id: 1,
            ha_role: remdb::ha::HARole::Auto,
            replication_mode: remdb::ha::ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
        }),
    });

#[test]
#[serial]
fn test_sql_transaction_commit() {
    let _db_memory = setup_test_db();

    // 重置全局数据库实例和事务管理器
    remdb::reset_global_db();
    crate::transaction::init_tx_manager();

    // 创建数据库实例
    let db = init_global_db(&TEST_DB_CONFIG).unwrap();

    // 开始事务
    let result = db.sql_query("BEGIN TRANSACTION;");
    assert!(result.is_ok());

    // 插入记录
    let result = db.sql_query("INSERT INTO test_table VALUES (1, 3.14);");
    assert!(result.is_ok());

    // 插入另一条记录
    let result = db.sql_query("INSERT INTO test_table VALUES (2, 6.28);");
    assert!(result.is_ok());

    // 提交事务
    let result = db.sql_query("COMMIT;");
    assert!(result.is_ok());

    // 验证记录已插入
    let result = db.sql_query("SELECT COUNT(*) FROM test_table;");
    assert!(result.is_ok());
    let result_set = result.unwrap();
    assert_eq!(result_set.rows.len(), 1);

    // 显式重置数据库实例，确保所有资源被正确释放
    remdb::reset_global_db();
}

#[test]
#[serial]
fn test_sql_transaction_rollback() {
    let _db_memory = setup_test_db();

    // 重置全局数据库实例和事务管理器
    remdb::reset_global_db();
    crate::transaction::init_tx_manager();

    // 创建数据库实例
    let db = init_global_db(&TEST_DB_CONFIG).unwrap();

    // 开始事务
    let result = db.sql_query("BEGIN;");
    assert!(result.is_ok());

    // 插入记录
    let result = db.sql_query("INSERT INTO test_table VALUES (1, 3.14);");
    assert!(result.is_ok());

    // 插入另一条记录
    let result = db.sql_query("INSERT INTO test_table VALUES (2, 6.28);");
    assert!(result.is_ok());

    // 回滚事务
    let result = db.sql_query("ROLLBACK;");
    assert!(result.is_ok());

    // 验证记录已回滚
    let result = db.sql_query("SELECT COUNT(*) FROM test_table;");
    assert!(result.is_ok());
    let result_set = result.unwrap();
    assert_eq!(result_set.rows.len(), 1);

    // 显式重置数据库实例，确保所有资源被正确释放
    remdb::reset_global_db();
}

#[test]
#[serial]
fn test_sql_transaction_simple() {
    let _db_memory = setup_test_db();

    // 重置全局数据库实例和事务管理器
    remdb::reset_global_db();
    crate::transaction::init_tx_manager();

    // 创建数据库实例
    let db = init_global_db(&TEST_DB_CONFIG).unwrap();

    // 执行完整的事务
    let result = db.sql_query("BEGIN TRANSACTION;");
    assert!(result.is_ok());

    let result = db.sql_query("INSERT INTO test_table VALUES (1, 1.0);");
    assert!(result.is_ok());

    let result = db.sql_query("INSERT INTO test_table VALUES (2, 2.0);");
    assert!(result.is_ok());

    let result = db.sql_query("INSERT INTO test_table VALUES (3, 3.0);");
    assert!(result.is_ok());

    let result = db.sql_query("COMMIT;");
    assert!(result.is_ok());

    // 验证记录已插入
    let result = db.sql_query("SELECT * FROM test_table;");
    assert!(result.is_ok());
    let result_set = result.unwrap();
    assert_eq!(result_set.rows.len(), 3);

    // 显式重置数据库实例，确保所有资源被正确释放
    remdb::reset_global_db();
}
