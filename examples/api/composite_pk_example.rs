extern crate alloc;
use remdb::{DataType, RemDb, Result};

static mut DB_MEMORY: [u8; 4194304] = [0u8; 4194304];

static DB_CONFIG: remdb::config::DbConfig = remdb::config::DbConfig {
    tables: vec![],
    total_memory: 4194304,
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 1000,
    memory_allocator: unsafe {
        static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator =
            remdb::config::DefaultMemoryAllocator;
        &mut DEFAULT_ALLOCATOR
    },
    wal_config: remdb::config::WALConfig {
        log_path: "wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1024 * 1024,
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
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,

    model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
};

/// 复合主键示例
fn main() -> Result<()> {
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;

        #[cfg(feature = "posix")]
        remdb::platform::init_platform(remdb::platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            struct DummyPlatform;
            impl remdb::platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
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
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: remdb::platform::FileMode,
                ) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
                    Err(())
                }
                fn file_close(
                    &self,
                    _handle: remdb::platform::FileHandle,
                ) -> remdb::platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _offset: i64,
                    _whence: remdb::platform::SeekWhence,
                ) -> remdb::platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> remdb::platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            remdb::platform::init_platform(&DUMMY_PLATFORM);
        }
    }

    let mut db = RemDb::new(&DB_CONFIG);
    db.init()?;

    println!("=== 复合主键示例 ===\n");

    test_api_composite_pk(&mut db)?;
    test_sql_composite_pk(&mut db)?;
    test_sql_composite_pk_operations(&mut db)?;

    println!("\n=== 示例完成 ===");
    Ok(())
}

fn test_api_composite_pk(db: &mut RemDb) -> Result<()> {
    println!("=== 方法1: 通过API创建复合主键表 ===\n");

    let fields = [
        ("device_id", DataType::UInt32, 0, None, None),
        ("metric_id", DataType::UInt32, 0, None, None),
        ("timestamp", DataType::UInt64, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];

    let primary_key = Some(vec![0, 1, 2]);

    db.create_table("metrics_api", &fields, primary_key)?;
    println!("成功创建带有复合主键的表: metrics_api (device_id, metric_id, timestamp)");

    let tables = db.get_all_tables();
    let table = tables
        .iter()
        .find(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "metrics_api"
            } else {
                false
            }
        })
        .ok_or(remdb::RemDbError::RecordNotFound)?
        .as_ref()
        .ok_or(remdb::RemDbError::RecordNotFound)?;

    println!("表结构:");
    println!("  字段数: {}", table.def.fields.len());
    println!("  复合主键字段索引: {:?}", table.def.primary_key);

    for (i, field) in table.def.fields.iter().enumerate() {
        println!("  字段 {}: {} ({:?})", i, field.name, field.data_type);
    }

    println!();
    Ok(())
}

fn test_sql_composite_pk(db: &mut RemDb) -> Result<()> {
    println!("=== 方法2: 通过SQL创建复合主键表 ===\n");

    let create_table_sql = r#"
        CREATE TABLE metrics_sql (
            device_id INTEGER NOT NULL,
            metric_id INTEGER NOT NULL,
            timestamp TIMESTAMP NOT NULL,
            value REAL NOT NULL,
            PRIMARY KEY (device_id, metric_id, timestamp)
        )
    "#;

    let result = db.sql_query(create_table_sql);
    match &result {
        Ok(_) => println!("成功通过SQL创建带有复合主键的表: metrics_sql"),
        Err(e) => {
            println!("创建表失败: {:?}", e);
            return result.map(|_| ());
        }
    }

    let tables = db.get_all_tables();
    let table = tables
        .iter()
        .find(|table_opt| {
            if let Some(table) = table_opt {
                table.def.name == "metrics_sql"
            } else {
                false
            }
        })
        .ok_or(remdb::RemDbError::RecordNotFound)?
        .as_ref()
        .ok_or(remdb::RemDbError::RecordNotFound)?;

    println!("\n表结构:");
    println!("  字段数: {}", table.def.fields.len());
    println!("  复合主键字段索引: {:?}", table.def.primary_key);

    for (i, field) in table.def.fields.iter().enumerate() {
        println!("  字段 {}: {} ({:?})", i, field.name, field.data_type);
    }

    println!();
    Ok(())
}

fn test_sql_composite_pk_operations(db: &mut RemDb) -> Result<()> {
    println!("=== 方法3: SQL复合主键表数据操作测试 ===\n");

    let create_table_sql = r#"
        CREATE TABLE sensor_readings (
            sensor_id INTEGER NOT NULL,
            region_id INTEGER NOT NULL,
            reading_time TIMESTAMP NOT NULL,
            temperature REAL,
            humidity REAL,
            PRIMARY KEY (sensor_id, region_id, reading_time)
        )
    "#;

    db.sql_query(create_table_sql)?;
    println!("成功创建表: sensor_readings");

    println!("\n插入测试数据...");

    let insert_statements = [
        "INSERT INTO sensor_readings VALUES (1, 100, 1609459200000, 23.5, 65.0)",
        "INSERT INTO sensor_readings VALUES (1, 100, 1609459260000, 23.8, 64.5)",
        "INSERT INTO sensor_readings VALUES (1, 100, 1609459320000, 24.1, 64.0)",
        "INSERT INTO sensor_readings VALUES (1, 200, 1609459200000, 22.0, 70.0)",
        "INSERT INTO sensor_readings VALUES (2, 100, 1609459200000, 25.0, 60.0)",
        "INSERT INTO sensor_readings VALUES (2, 100, 1609459260000, 25.5, 59.5)",
    ];

    for sql in &insert_statements {
        match db.sql_query(sql) {
            Ok(result) => println!("  插入成功: {}", result.to_string()),
            Err(e) => println!("  插入失败: {:?}", e),
        }
    }

    println!("\n查询所有数据:");
    let result = db.sql_query("SELECT * FROM sensor_readings")?;
    println!("{}", result.to_string());

    println!("\n按sensor_id查询:");
    let result = db.sql_query("SELECT * FROM sensor_readings WHERE sensor_id = 1")?;
    println!("{}", result.to_string());

    println!("\n按复合条件查询 (sensor_id=1 AND region_id=100):");
    let result =
        db.sql_query("SELECT * FROM sensor_readings WHERE sensor_id = 1 AND region_id = 100")?;
    println!("{}", result.to_string());

    println!("\n按时间范围查询:");
    let result =
        db.sql_query("SELECT * FROM sensor_readings WHERE reading_time >= 1609459260000")?;
    println!("{}", result.to_string());

    println!("\n聚合查询 - 按sensor_id分组统计平均温度:");
    let result = db.sql_query(
        "SELECT sensor_id, AVG(temperature) AS avg_temp FROM sensor_readings GROUP BY sensor_id",
    )?;
    println!("{}", result.to_string());

    println!("\n更新测试:");
    let update_result = db.sql_query("UPDATE sensor_readings SET temperature = 24.0 WHERE sensor_id = 1 AND region_id = 100 AND reading_time = 1609459200000");
    match update_result {
        Ok(result) => println!("  更新成功: {}", result.to_string()),
        Err(e) => println!("  更新失败: {:?}", e),
    }

    println!("\n验证更新结果:");
    let result =
        db.sql_query("SELECT * FROM sensor_readings WHERE sensor_id = 1 AND region_id = 100")?;
    println!("{}", result.to_string());

    println!("\n删除测试:");
    let delete_result = db.sql_query("DELETE FROM sensor_readings WHERE sensor_id = 2");
    match delete_result {
        Ok(result) => println!("  删除成功: {}", result.to_string()),
        Err(e) => println!("  删除失败: {:?}", e),
    }

    println!("\n验证删除结果:");
    let result = db.sql_query("SELECT * FROM sensor_readings")?;
    println!("{}", result.to_string());

    println!("\n测试复合主键唯一性约束...");
    let duplicate_insert =
        db.sql_query("INSERT INTO sensor_readings VALUES (1, 100, 1609459200000, 30.0, 50.0)");
    match duplicate_insert {
        Ok(_) => println!("  警告: 复合主键重复插入成功，可能存在问题"),
        Err(e) => println!("  符合预期: 复合主键重复插入被拒绝 ({:?})", e),
    }

    println!("\n测试不同复合主键组合的插入...");
    let new_insert =
        db.sql_query("INSERT INTO sensor_readings VALUES (1, 100, 1609459380000, 24.5, 63.0)");
    match new_insert {
        Ok(result) => println!("  新记录插入成功: {}", result.to_string()),
        Err(e) => println!("  插入失败: {:?}", e),
    }

    println!("\n最终数据:");
    let result =
        db.sql_query("SELECT * FROM sensor_readings ORDER BY sensor_id, region_id, reading_time")?;
    println!("{}", result.to_string());

    Ok(())
}
