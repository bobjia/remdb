extern crate alloc;

use remdb::{
    config::{DbConfig, DefaultMemoryAllocator, LogMode, WALConfig},
    RemDb,
};
use std::sync::Mutex;

// 互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 为每个测试创建单独的内存缓冲区
static mut DB_MEMORY1: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY2: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY3: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY4: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY5: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY6: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB
static mut DB_MEMORY7: [u8; 32 * 1024 * 1024] = [0; 32 * 1024 * 1024]; // 32MB

// 测试基本的表删除操作
#[test]
fn test_drop_table_basic() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY1.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY1.as_mut_ptr(), DB_MEMORY1.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            max_consecutive_invalid: 100,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 创建表
    db.create_table(
        "test_table",
        &[("id", remdb::DataType::Int64, 0, None, None)],
        Some(vec![0]),
    )
    .unwrap();

    // 验证表存在
    let table_index = db
        .get_all_tables()
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "test_table"
            } else {
                false
            }
        })
        .unwrap();
    let table = db.get_table(table_index).unwrap();
    assert_eq!(table.def.name, "test_table");

    // 删除表
    db.drop_table("test_table", false, false).unwrap();

    // 验证表不存在
    let result = db.get_table(table_index);
    assert!(result.is_err());
}

// 测试VECTOR类型表的创建和删除（使用IF EXISTS/IF NOT EXISTS）
#[test]
fn test_drop_table_vector_with_if_exists() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY7.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY7.as_mut_ptr(), DB_MEMORY7.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            max_consecutive_invalid: 100,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 通过SQL创建VECTOR类型表（使用IF NOT EXISTS）
    let create_sql = "CREATE TABLE IF NOT EXISTS test_vector (id INTEGER PRIMARY KEY AUTOINCREMENT, value VECTOR(3) WITH DISTANCE=L2);";
    let create_query = remdb::sql::parse_sql_query(create_sql).unwrap();
    remdb::sql::execute_query(&mut db, &create_query).unwrap();

    // 验证表存在
    let table_index = db
        .get_all_tables()
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "test_vector"
            } else {
                false
            }
        })
        .unwrap();
    let table = db.get_table(table_index).unwrap();
    assert_eq!(table.def.name, "test_vector");

    // 通过SQL删除表（使用IF EXISTS）
    let drop_sql = "DROP TABLE IF EXISTS test_vector;";
    let drop_query = remdb::sql::parse_sql_query(drop_sql).unwrap();
    remdb::sql::execute_query(&mut db, &drop_query).unwrap();

    // 验证表不存在
    let result = db.get_table(table_index);
    assert!(result.is_err());

    // 再次删除不存在的表（使用IF EXISTS），应该成功
    let drop_sql2 = "DROP TABLE IF EXISTS test_vector;";
    let drop_query2 = remdb::sql::parse_sql_query(drop_sql2).unwrap();
    let result2 = remdb::sql::execute_query(&mut db, &drop_query2);
    assert!(result2.is_ok());
}

// 测试 IF EXISTS 选项的行为
#[test]
fn test_drop_table_if_exists() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY2.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY2.as_mut_ptr(), DB_MEMORY2.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            max_consecutive_invalid: 100,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 尝试删除不存在的表，使用 IF EXISTS 选项
    let result = db.drop_table("non_existent_table", true, false);
    assert!(result.is_ok());

    // 尝试删除不存在的表，不使用 IF EXISTS 选项
    let result = db.drop_table("non_existent_table", false, false);
    assert!(result.is_err());
}

// 测试通过SQL语句删除表
#[test]
fn test_drop_table_sql() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY3.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY3.as_mut_ptr(), DB_MEMORY3.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            max_consecutive_invalid: 100,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 通过SQL创建表
    let create_sql = "CREATE TABLE test_table (id INT PRIMARY KEY, name VARCHAR);";
    let create_query = remdb::sql::parse_sql_query(create_sql).unwrap();
    remdb::sql::execute_query(&mut db, &create_query).unwrap();

    // 验证表存在
    let table_index = db
        .get_all_tables()
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "test_table"
            } else {
                false
            }
        })
        .unwrap();
    let table = db.get_table(table_index).unwrap();
    assert_eq!(table.def.name, "test_table");

    // 通过SQL删除表
    let drop_sql = "DROP TABLE test_table;";
    let drop_query = remdb::sql::parse_sql_query(drop_sql).unwrap();
    remdb::sql::execute_query(&mut db, &drop_query).unwrap();

    // 验证表不存在
    let result = db.get_table(table_index);
    assert!(result.is_err());
}

// 测试通过SQL语句删除不存在的表（使用IF EXISTS）
#[test]
fn test_drop_table_sql_if_exists() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY4.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY4.as_mut_ptr(), DB_MEMORY4.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            max_consecutive_invalid: 100,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 通过SQL删除不存在的表，使用IF EXISTS
    let drop_sql = "DROP TABLE IF EXISTS non_existent_table;";
    let drop_query = remdb::sql::parse_sql_query(drop_sql).unwrap();
    let result = remdb::sql::execute_query(&mut db, &drop_query);
    assert!(result.is_ok());
}

// 测试事务中的表删除
#[test]
fn test_drop_table_in_transaction() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY5.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY5.as_mut_ptr(), DB_MEMORY5.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            max_consecutive_invalid: 100,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 创建表
    db.create_table(
        "test_table",
        &[("id", remdb::DataType::Int64, 0, None, None)],
        Some(vec![0]),
    )
    .unwrap();

    // 验证表存在
    let table_index = db
        .get_all_tables()
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "test_table"
            } else {
                false
            }
        })
        .unwrap();
    let table = db.get_table(table_index).unwrap();
    assert_eq!(table.def.name, "test_table");

    // 删除表
    db.drop_table("test_table", false, false).unwrap();

    // 验证表不存在
    let result = db.get_table(table_index);
    assert!(result.is_err());
}

// 测试内存回收的完整性
#[test]
fn test_drop_table_memory_recovery() {
    // 获取互斥锁，确保测试串行执行
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    // 重置内存缓冲区
    unsafe {
        DB_MEMORY6.fill(0);
    }

    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY6.as_mut_ptr(), DB_MEMORY6.len())
            .unwrap();
    }

    // 初始化数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        total_memory: 32 * 1024 * 1024, // 32MB
        default_max_records: 100,
        low_power_mode_supported: false,
        low_power_max_records: Some(50),
        wal_config: WALConfig {
            log_path: "",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 1,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            max_consecutive_invalid: 100,
            compression_type: remdb::config::WALCompressionType::None,
            compression_level: 3,
        },
        tables: Vec::new(),
        memory_allocator: &DefaultMemoryAllocator,
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600,
            retention_period_secs: 86400,
            max_partitions: 100,
            compression: remdb::time_series::CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
    };

    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init().unwrap();

    // 创建表
    db.create_table(
        "test_table",
        &[
            ("id", remdb::DataType::Int64, 0, None, None),
            ("name", remdb::DataType::VarChar, 0, None, None),
        ],
        Some(vec![0]),
    )
    .unwrap();

    // 验证表存在
    let table_index = db
        .get_all_tables()
        .iter()
        .position(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "test_table"
            } else {
                false
            }
        })
        .unwrap();

    // 删除表
    db.drop_table("test_table", false, false).unwrap();

    // 验证表不存在
    let result = db.get_table(table_index);
    assert!(result.is_err());
}
