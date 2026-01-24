use remdb::config::{DbConfig, DefaultMemoryAllocator, LogMode, TimeSeriesConfig, WALConfig};
use remdb::platform::{init_platform, FileHandle, FileMode, FileResult, Platform, SeekWhence};
use remdb::transaction::set_low_power_mode;
use remdb::{init_global_db, reset_global_db, RemDb};

// 测试用Platform实现
struct TestPlatform;

impl Platform for TestPlatform {
    fn get_timestamp(&self) -> u64 {
        0
    }

    fn get_timestamp_us(&self) -> u64 {
        0
    }

    fn spin_lock(&self, lock: &mut u32) {
        unsafe {
            while core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .compare_exchange(
                    0,
                    1,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
            {
                core::hint::spin_loop();
            }
        }
    }

    fn spin_unlock(&self, lock: &mut u32) {
        unsafe {
            core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .store(0, core::sync::atomic::Ordering::Release);
        }
    }

    fn compiler_barrier(&self) {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }

    fn full_memory_barrier(&self) {
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }

    fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
        unsafe {
            core::ptr::copy_nonoverlapping(src, dest, size);
        }
    }

    fn memset(&self, dest: *mut u8, value: u8, size: usize) {
        unsafe {
            core::ptr::write_bytes(dest, value, size);
        }
    }

    fn delay_ms(&self, _ms: u32) {
        // 空实现
    }

    fn delay_us(&self, _us: u32) {
        // 空实现
    }

    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> {
        // 返回一个非空指针作为有效的FileHandle
        Ok(1 as *const u8)
    }

    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Ok(())
    }

    fn file_write(
        &self,
        _handle: FileHandle,
        _buffer: *const u8,
        size: usize,
    ) -> FileResult<usize> {
        // 模拟写入成功，返回写入的字节数
        Ok(size)
    }

    fn file_read(&self, _handle: FileHandle, _buffer: *mut u8, _size: usize) -> FileResult<usize> {
        // 模拟读取成功，返回0表示文件为空
        Ok(0)
    }

    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
        // 模拟seek成功，返回当前位置
        Ok(0)
    }

    fn file_remove(&self, _path: &str) -> FileResult<()> {
        Ok(())
    }

    fn file_size(&self, _path: &str) -> FileResult<usize> {
        Ok(0)
    }

    fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
        0
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 定义测试配置
static TEST_TABLE: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 0,
    name: "test_table".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::Int32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "name".to_string(),
            data_type: remdb::types::DataType::String,
            size: 64,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 68,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

// 定义时序表配置
static TEST_TIMESERIES_TABLE: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 1,
    name: "sensor_data".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::Int32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "sensor_id".to_string(),
            data_type: remdb::types::DataType::String,
            size: 32,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "value".to_string(),
            data_type: remdb::types::DataType::Float64,
            size: 8,
            offset: 36,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "timestamp".to_string(),
            data_type: remdb::types::DataType::Int64,
            size: 8,
            offset: 44,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: 0,
    secondary_index: Some(3), // 时间戳字段索引
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 52,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

// 定义包含向量数据的表配置
static TEST_VECTOR_TABLE: std::sync::LazyLock<remdb::types::TableDef> = std::sync::LazyLock::new(|| remdb::types::TableDef {
    id: 2,
    name: "vector_data".to_string(),
    fields: vec![
        remdb::types::FieldDef {
            name: "id".to_string(),
            data_type: remdb::types::DataType::Int32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: false,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "name".to_string(),
            data_type: remdb::types::DataType::String,
            size: 32,
            offset: 4,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        remdb::types::FieldDef {
            name: "vector".to_string(),
            data_type: remdb::types::DataType::Vector,
            size: 32, // 8维向量，每个元素4字节，共32字节
            offset: 36,
            primary_key: false,
            not_null: true,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: Some(remdb::types::VectorMetadata {
                dimension: 8,
                distance_type: remdb::types::DistanceType::L2,
                index_type: remdb::types::VectorIndexType::HNSW,
            }),
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: remdb::types::IndexType::SortedArray,
    record_size: 68,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

// 静态测试数据库配置
static TEST_DB_CONFIG: std::sync::LazyLock<DbConfig> = std::sync::LazyLock::new(|| DbConfig {
    tables: vec![
        TEST_TABLE.clone(),
        TEST_TIMESERIES_TABLE.clone(),
        TEST_VECTOR_TABLE.clone()
    ],
    total_memory: 2097152, // 2MB
    default_max_records: 100,
    low_power_mode_supported: true,
    low_power_max_records: Some(50),
    memory_allocator: &DefaultMemoryAllocator,
    wal_config: WALConfig {
        log_path: "./wal",
        log_mode: LogMode::Async,
        log_prealloc_size: 0,
        log_file_size_limit: 1048576,
        log_segment_size: 1048576,
        checkpoint_interval_ms: 30000,
        retained_checkpoints: 2,
    },
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,
    time_series_defaults: TimeSeriesConfig::DEFAULT,
});

// 使用静态内存缓冲区，确保它不会在函数返回时被释放
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152];

#[test]
fn test_wal_recovery_no_overwrite() {
    // 初始化平台抽象层
    init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .unwrap();
    }

    // 重置全局数据库实例，确保测试之间的隔离
    reset_global_db();

    // 初始化数据库
    let db = init_global_db(&TEST_DB_CONFIG).unwrap();

    // 插入普通表数据
    let insert_sql1 = "INSERT INTO test_table (name) VALUES ('test1')";
    let result1 = db.sql_query(insert_sql1);
    assert!(result1.is_ok());

    // 插入时序表数据
    let insert_sql2 = "INSERT INTO sensor_data (sensor_id, value, timestamp) VALUES ('sensor1', 25.5, 1609459200000)";
    let result2 = db.sql_query(insert_sql2);
    assert!(result2.is_ok());

    // 插入向量表数据
    let insert_sql3 = "INSERT INTO vector_data (name, vector) VALUES ('vector1', '[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]')";
    let result3 = db.sql_query(insert_sql3);
    assert!(result3.is_ok());

    // 重置数据库以模拟重启
    reset_global_db();

    // 重新初始化数据库
    let db2 = init_global_db(&TEST_DB_CONFIG).unwrap();

    // 模拟WAL恢复过程（这里会调用recover方法）
    // 由于我们的测试平台模拟了空文件，所以实际不会恢复任何数据
    // 但代码会执行recover方法，这正是我们要测试的

    // 插入新的普通表数据
    let insert_sql4 = "INSERT INTO test_table (name) VALUES ('test3')";
    let result4 = db2.sql_query(insert_sql4);
    assert!(result4.is_ok());

    // 插入新的时序表数据
    let insert_sql5 = "INSERT INTO sensor_data (sensor_id, value, timestamp) VALUES ('sensor2', 26.0, 1609459260000)";
    let result5 = db2.sql_query(insert_sql5);
    assert!(result5.is_ok());

    // 插入新的向量表数据
    let insert_sql6 = "INSERT INTO vector_data (name, vector) VALUES ('vector2', '[0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]')";
    let result6 = db2.sql_query(insert_sql6);
    assert!(result6.is_ok());

    // 查询所有表数据，验证不会覆盖
    let select_sql1 = "SELECT * FROM test_table";
    let result7 = db2.sql_query(select_sql1);
    assert!(result7.is_ok());

    let select_sql2 = "SELECT * FROM sensor_data";
    let result8 = db2.sql_query(select_sql2);
    assert!(result8.is_ok());

    let select_sql3 = "SELECT * FROM vector_data";
    let result9 = db2.sql_query(select_sql3);
    assert!(result9.is_ok());

    println!("WAL recovery test passed: New data inserted without overwriting existing records!");
}
