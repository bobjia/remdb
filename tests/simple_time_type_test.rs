#![cfg(feature = "std")]
#![allow(unsafe_code)]

extern crate alloc;

use remdb::{RemDb, config};
use remdb::config::WALConfig;

/// 创建测试用的DbConfig
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[],
    total_memory: 104857600,
    default_max_records: 100,
    low_power_mode_supported: false,
    low_power_max_records: None,
    // 添加缺少的字段
    memory_allocator: &config::DefaultMemoryAllocator,
    wal_config: WALConfig {
        log_path: "./wal",
        log_mode: config::LogMode::Async,
        log_prealloc_size: 0,
        log_file_size_limit: 104857600,
        log_segment_size: 1048576,
        checkpoint_interval_ms: 30000,
        retained_checkpoints: 2,
    },
    time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: Some(config::HAConfig {
        node_id: 1,
        ha_role: remdb::ha::HARole::Auto,
        replication_mode: remdb::ha::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 1000,
        master_address: None,
        master_port: None,
        replication_port: 5556,
    }),
};

/// 基于std的真实文件I/O平台实现，用于WAL初始化和NOW()等时间函数
struct SimpleTestPlatform;

impl remdb::platform::Platform for SimpleTestPlatform {
    fn get_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn get_timestamp_us(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0)
    }

    fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
        let len = core::cmp::min(dest.len(), src.len());
        dest[..len].copy_from_slice(&src[..len]);
    }

    fn memset(&self, dest: &mut [u8], value: u8) {
        dest.fill(value);
    }

    fn delay_ms(&self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }

    fn delay_us(&self, us: u32) {
        std::thread::sleep(std::time::Duration::from_micros(us as u64));
    }

    fn file_open(&self, path: &str, mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
        use std::fs::OpenOptions;
        let mut options = OpenOptions::new();
        match mode {
            remdb::platform::FileMode::Read => { options.read(true); }
            remdb::platform::FileMode::Write => { options.write(true).create(true).truncate(true); }
            remdb::platform::FileMode::ReadWrite => { options.read(true).write(true).create(true); }
            remdb::platform::FileMode::Append => { options.write(true).create(true).append(true); }
        }
        match options.open(path) {
            Ok(file) => Ok(Box::into_raw(Box::new(file)) as remdb::platform::FileHandle),
            Err(_) => Err(()),
        }
    }

    fn file_close(&self, handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
        unsafe { drop(Box::from_raw(handle as *mut std::fs::File)); }
        Ok(())
    }

    fn file_write(&self, handle: remdb::platform::FileHandle, buf: &[u8]) -> remdb::platform::FileResult<usize> {
        use std::io::Write;
        unsafe {
            let file = &mut *(handle as *mut std::fs::File);
            file.write_all(buf).map_err(|_| ())?;
            file.flush().map_err(|_| ())?;
        }
        Ok(buf.len())
    }

    fn file_read(&self, handle: remdb::platform::FileHandle, buf: &mut [u8]) -> remdb::platform::FileResult<usize> {
        use std::io::Read;
        unsafe { (&mut *(handle as *mut std::fs::File)).read(buf).map_err(|_| ()) }
    }

    fn file_seek(&self, handle: remdb::platform::FileHandle, offset: i64, whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
        use std::io::{Seek, SeekFrom};
        let seek_from = match whence {
            remdb::platform::SeekWhence::SeekSet => SeekFrom::Start(offset as u64),
            remdb::platform::SeekWhence::SeekCur => SeekFrom::Current(offset),
            remdb::platform::SeekWhence::SeekEnd => SeekFrom::End(offset),
        };
        unsafe { (&mut *(handle as *mut std::fs::File)).seek(seek_from).map_err(|_| ()) }
    }

    fn file_remove(&self, path: &str) -> remdb::platform::FileResult<()> {
        std::fs::remove_file(path).map_err(|_| ())
    }

    fn file_size(&self, path: &str) -> remdb::platform::FileResult<usize> {
        std::fs::metadata(path).map(|m| m.len() as usize).map_err(|_| ())
    }

    fn crc32(&self, data: &[u8]) -> u32 {
        const CRC32_POLY: u32 = 0xEDB88320;
        let mut crc = 0xFFFFFFFFu32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ CRC32_POLY } else { crc >> 1 };
            }
        }
        crc ^ 0xFFFFFFFF
    }
}

#[test]
fn test_create_table_with_time_types() {
    // 注册基于std的真实文件I/O平台（db.init()初始化WAL需要，NOW()等需要时间戳）
    static TEST_PLATFORM: SimpleTestPlatform = SimpleTestPlatform;
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 使用静态内存缓冲区，确保它不会在函数返回时被释放
    static mut DB_MEMORY: [u8; 262144] = [0u8; 262144];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).unwrap();
    }
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_DB_CONFIG);
    db.init().unwrap();
    
    // 创建带有TIMESTAMP和TIMESTAMPTZ类型的表
    let create_table_sql = "CREATE TABLE test_time_types (
        id INTEGER PRIMARY KEY AUTO_INCREMENT,
        ts TIMESTAMP(3),
        tstz TIMESTAMPTZ(6),
        name TEXT
    )";
    
    println!("Running CREATE TABLE...");
    let result = db.sql_query(create_table_sql);
    if !result.is_ok() {
        println!("CREATE TABLE failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ CREATE TABLE succeeded");
    
    // 测试1: 插入带有时间类型的数据
    let insert_sql = "INSERT INTO test_time_types (ts, tstz, name) VALUES 
        (1609459200000, 1609459200000, 'test1'),
        (1609459260000, 1609459260000, 'test2')";
    
    println!("Running INSERT...");
    let result = db.sql_query(insert_sql);
    if !result.is_ok() {
        println!("INSERT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ INSERT succeeded");
    
    // 测试2: 查询数据 (simplified, no functions)
    let select_sql = "SELECT id, ts, tstz, name FROM test_time_types";
    
    println!("Running SELECT...");
    let result = db.sql_query(select_sql);
    if !result.is_ok() {
        println!("SELECT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ SELECT succeeded");

    // 测试2: 插入带有时间类型的数据，用函数
    let insert_sql = "INSERT INTO test_time_types (ts, tstz, name) VALUES 
      (NOW(), CURRENT_TIMESTAMP(), 'test1'),
        (LOCALTIMESTAMP(), NOW(), 'test2')";
    
    println!("Running INSERT...");
    let result = db.sql_query(insert_sql);
    if !result.is_ok() {
        println!("INSERT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ INSERT succeeded");
    
    // 测试3: 查询数据 (simplified, no functions)
    let select_sql = "SELECT id, ts, tstz, name FROM test_time_types";
    
    println!("Running SELECT...");
    let result = db.sql_query(select_sql);
    if !result.is_ok() {
        println!("SELECT failed with error: {:?}", result.as_ref().err());
    }
    assert!(result.is_ok());
    println!("✓ SELECT succeeded");
    
    println!("\nAll tests passed! ✓");
}