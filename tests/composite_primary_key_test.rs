extern crate alloc;

use remdb::{DataType, RemDb, Result};
use std::sync::LazyLock;

// 测试平台实现
struct TestPlatform;

impl remdb::platform::Platform for TestPlatform {
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

    fn file_open(&self, _path: &str, _mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
        Ok(core::ptr::null())
    }

    fn file_close(&self, _handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
        Ok(())
    }

    fn file_write(
        &self,
        _handle: remdb::platform::FileHandle,
        _buffer: *const u8,
        _size: usize,
    ) -> remdb::platform::FileResult<usize> {
        Ok(0)
    }

    fn file_read(&self, _handle: remdb::platform::FileHandle, _buffer: *mut u8, _size: usize) -> remdb::platform::FileResult<usize> {
        Ok(0)
    }

    fn file_seek(&self, _handle: remdb::platform::FileHandle, _offset: i64, _whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
        Ok(0)
    }

    fn file_remove(&self, _path: &str) -> remdb::platform::FileResult<()> {
        Ok(())
    }

    fn file_size(&self, _path: &str) -> remdb::platform::FileResult<usize> {
        Ok(0)
    }

    fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
        0
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 静态内存缓冲区
static mut DB_MEMORY: Vec<u8> = Vec::new();

// 静态内存分配器实例
static DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;

// 创建一个静态测试配置
static TEST_CONFIG: LazyLock<remdb::config::DbConfig> = LazyLock::new(|| {
    remdb::config::DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024 * 10, // 10MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &DEFAULT_ALLOCATOR,
        wal_config: remdb::config::WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            max_consecutive_invalid: 100,
            retained_checkpoints: 2,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
        },
        time_series_defaults: remdb::config::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
    }
});

#[test]
fn test_create_table_with_composite_pk() -> Result<()> {
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 创建新的内存缓冲区
    let mut new_memory = vec![0u8; 8388608]; // 8MB
    
    // 初始化全局分配器
    remdb::memory::allocator::init_global_allocator(new_memory.as_mut_ptr(), new_memory.len())?;
    
    // 更新全局内存缓冲区
    unsafe {
        DB_MEMORY = new_memory;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有复合主键的表
    let fields = [
        ("id1", DataType::UInt32, 4, None, None),
        ("id2", DataType::UInt32, 4, None, None),
        ("name", DataType::VarChar, 64, None, None),
        ("value", DataType::Float64, 8, None, None),
    ];
    
    // 定义主键为(id1, id2)
    let primary_key = Some(vec![0, 1]);
    
    db.create_table("test_composite", &fields, primary_key)?;
    
    Ok(())
}

#[test]
fn test_insert_and_query_with_composite_pk() -> Result<()> {
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 创建新的内存缓冲区
    let mut new_memory = vec![0u8; 8388608]; // 8MB
    
    // 初始化全局分配器
    remdb::memory::allocator::init_global_allocator(new_memory.as_mut_ptr(), new_memory.len())?;
    
    // 更新全局内存缓冲区
    unsafe {
        DB_MEMORY = new_memory;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有复合主键的表
    let fields = [
        ("id1", DataType::UInt32, 4, None, None),
        ("id2", DataType::UInt32, 4, None, None),
        ("name", DataType::VarChar, 64, None, None),
        ("value", DataType::Float64, 8, None, None),
    ];
    
    // 定义主键为(id1, id2)
    let primary_key = Some(vec![0, 1]);
    
    db.create_table("test_composite", &fields, primary_key)?;
    
    // 插入数据
    let table_id = 1; // 系统表占用0，所以新表ID为1
    let table = db.get_table_mut(table_id)?;
    
    // 准备记录数据
    let mut record = [0u8; 4 + 4 + 64 + 8]; // id1(4) + id2(4) + name(64) + value(8)
    
    // 插入第一条记录
    let id1: u32 = 1;
    let id2: u32 = 1;
    let name = "test1";
    let value: f64 = 100.5;
    
    // 设置id1
    record[0..4].copy_from_slice(&id1.to_le_bytes());
    // 设置id2
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    // 设置name
    let name_bytes = name.as_bytes();
    record[8..8+name_bytes.len()].copy_from_slice(name_bytes);
    // 设置value
    record[8+64..8+64+8].copy_from_slice(&value.to_le_bytes());
    
    // 插入记录
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 插入第二条记录，不同的id2
    let id2: u32 = 2;
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 插入第三条记录，不同的id1
    let id1: u32 = 2;
    let id2: u32 = 1;
    record[0..4].copy_from_slice(&id1.to_le_bytes());
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 尝试插入重复主键记录，应该失败
    let result = table.insert(record.as_ptr() as *const u8);
    assert!(result.is_err());
    
    Ok(())
}

#[test]
fn test_composite_pk_with_three_fields() -> Result<()> {
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 创建新的内存缓冲区
    let mut new_memory = vec![0u8; 8388608]; // 8MB
    
    // 初始化全局分配器
    remdb::memory::allocator::init_global_allocator(new_memory.as_mut_ptr(), new_memory.len())?;
    
    // 更新全局内存缓冲区
    unsafe {
        DB_MEMORY = new_memory;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有三字段复合主键的表
    let fields = [
        ("device_id", DataType::UInt32, 0, None, None),
        ("metric_id", DataType::UInt32, 0, None, None),
        ("timestamp", DataType::UInt64, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    // 定义复合主键：(device_id, metric_id, timestamp)
    let primary_key = Some(vec![0, 1, 2]);
    
    db.create_table("metrics", &fields, primary_key)?;
    
    Ok(())
}
