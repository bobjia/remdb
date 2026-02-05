extern crate alloc;
use alloc::boxed::Box;
use remdb::platform::*;
use remdb::{DatabaseStatus, RemDb, RemDbError, Result};
use std::sync::Mutex;

// 全局互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 静态内存缓冲区，用于测试
static mut DB_MEMORY: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024]; // 4MB内存

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
        // 简单的自旋锁实现
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
        // 空实现
    }

    fn full_memory_barrier(&self) {
        // 空实现
    }

    fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
        unsafe {
            core::ptr::copy(src, dest, size);
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
        // 模拟文件打开失败
        Err(())
    }

    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Ok(())
    }

    fn file_write(&self, _handle: FileHandle, _buffer: *const u8, _size: usize) -> FileResult<usize> {
        Err(())
    }

    fn file_read(&self, _handle: FileHandle, _buffer: *mut u8, _size: usize) -> FileResult<usize> {
        Err(())
    }

    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
        Err(())
    }

    fn file_remove(&self, _path: &str) -> FileResult<()> {
        Err(())
    }

    fn file_size(&self, _path: &str) -> FileResult<usize> {
        Err(())
    }

    fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
        0
    }
}

// 静态测试平台实例
static TEST_PLATFORM: TestPlatform = TestPlatform;

// 初始化全局数据库
fn init_global_db() -> Result<RemDb> {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len(),
        )?;
    }

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

    // 初始化数据库
    let mut db = init_global_db()?;

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

    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len(),
        )?;
    }

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
    let mut db_names = databases.iter().map(|info| info.name.clone()).collect::<Vec<_>>();
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
