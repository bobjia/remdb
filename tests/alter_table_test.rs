use remdb::config::WALConfig;
use remdb::platform::{FileHandle, FileMode, FileResult, Platform, SeekWhence};
use remdb::*;
use std::sync::Mutex;

// 定义测试平台
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
        Ok(core::ptr::null())
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
        Ok(size)
    }

    fn file_read(&self, _handle: FileHandle, _buffer: *mut u8, _size: usize) -> FileResult<usize> {
        Ok(0)
    }

    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
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

// 静态测试平台实例
static TEST_PLATFORM: TestPlatform = TestPlatform;

// 静态内存分配器实例
static DEFAULT_ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;

// 静态测试配置
static TEST_CONFIG: std::sync::LazyLock<config::DbConfig> = std::sync::LazyLock::new(|| {
    config::DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100, // 减小值以避免内存不足
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
        time_series_defaults: config::TimeSeriesConfig::DEFAULT,
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
    }
});

// 互斥锁，确保测试串行执行
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// 定义测试用的内存缓冲区
static mut DB_MEMORY: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024]; // 4MB内存

#[test]
fn test_alter_table_add_column() {
    let _guard = TEST_MUTEX.lock().unwrap();

    // 重置内存缓冲区
    unsafe {
        core::ptr::write_bytes(DB_MEMORY.as_mut_ptr(), 0, DB_MEMORY.len());
    }

    // 初始化平台
    platform::init_platform(&TEST_PLATFORM);

    // 使用共享内存缓冲区初始化全局内存分配器
    let result = memory::allocator::init_global_allocator(
        unsafe { DB_MEMORY.as_mut_ptr() },
        unsafe { DB_MEMORY.len() }
    );
    assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());

    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);

    // 测试创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, 4, None, None),
            ("name", DataType::String, 32, None, None),
            ("age", DataType::UInt8, 1, None, None),
        ],
        Some(0), // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());

    // 测试添加列
    let result = db.sql_query("ALTER TABLE users ADD COLUMN active BOOL");
    assert!(result.is_ok(), "Failed to add column: {:?}", result.err());
}

#[test]
fn test_alter_table_drop_column() {
    let _guard = TEST_MUTEX.lock().unwrap();

    // 初始化平台
    platform::init_platform(&TEST_PLATFORM);

    // 使用共享内存缓冲区初始化全局内存分配器
    let result = memory::allocator::init_global_allocator(
        unsafe { DB_MEMORY.as_mut_ptr() },
        unsafe { DB_MEMORY.len() }
    );
    assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());

    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);

    // 测试创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, 4, None, None),
            ("name", DataType::String, 32, None, None),
            ("age", DataType::UInt8, 1, None, None),
            ("active", DataType::Bool, 1, None, None),
        ],
        Some(0), // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());

    // 测试删除列
    let result = db.sql_query("ALTER TABLE users DROP COLUMN active");
    assert!(result.is_ok(), "Failed to drop column: {:?}", result.err());
}

#[test]
fn test_alter_table_modify_column() {
    let _guard = TEST_MUTEX.lock().unwrap();

    // 初始化平台
    platform::init_platform(&TEST_PLATFORM);

    // 使用共享内存缓冲区初始化全局内存分配器
    let result = memory::allocator::init_global_allocator(
        unsafe { DB_MEMORY.as_mut_ptr() },
        unsafe { DB_MEMORY.len() }
    );
    assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());

    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);

    // 测试创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, 4, None, None),
            ("name", DataType::String, 32, None, None),
            ("age", DataType::UInt8, 1, None, None),
        ],
        Some(0), // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());

    // 测试修改列
    let result = db.sql_query("ALTER TABLE users MODIFY COLUMN age UInt16");
    assert!(result.is_ok(), "Failed to modify column: {:?}", result.err());
}

#[test]
fn test_alter_table_rename_column() {
    let _guard = TEST_MUTEX.lock().unwrap();

    // 初始化平台
    platform::init_platform(&TEST_PLATFORM);

    // 使用共享内存缓冲区初始化全局内存分配器
    let result = memory::allocator::init_global_allocator(
        unsafe { DB_MEMORY.as_mut_ptr() },
        unsafe { DB_MEMORY.len() }
    );
    assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());

    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);

    // 测试创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, 4, None, None),
            ("name", DataType::String, 32, None, None),
            ("age", DataType::UInt8, 1, None, None),
        ],
        Some(0), // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());

    // 测试重命名列
    let result = db.sql_query("ALTER TABLE users RENAME COLUMN age TO user_age");
    assert!(result.is_ok(), "Failed to rename column: {:?}", result.err());
}
