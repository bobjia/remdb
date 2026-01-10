extern crate alloc;

use remdb::types::Result;
use remdb::platform::{Platform, FileMode, FileHandle, FileResult, SeekWhence};

// 定义测试用的内存缓冲区
static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024]; // 1MB内存

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
                .compare_exchange(0, 1, 
                                 core::sync::atomic::Ordering::Acquire,
                                 core::sync::atomic::Ordering::Relaxed)
                .is_err() {
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
    
    fn file_read(&self, _handle: FileHandle, _buf: *mut u8, _size: usize) -> FileResult<usize> {
        Ok(0)
    }
    
    fn file_write(&self, _handle: FileHandle, _buf: *const u8, _size: usize) -> FileResult<usize> {
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

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 测试索引恢复功能
#[test]
fn test_index_recovery() -> Result<()> {
    // 初始化平台
    unsafe {
        remdb::platform::init_platform(&TEST_PLATFORM);
    }
    
    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        )?;
    }
    
    // 使用全局初始化函数初始化数据库
    let db = unsafe {
        remdb::init_global_db(&remdb::config::DbConfig {
            tables: &[],
            total_memory: 1024 * 1024, // 1MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 100,
            memory_allocator: &remdb::config::DefaultMemoryAllocator {},
            log_path: "index_recovery_test.wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
            time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: Some(remdb::config::HAConfig {
                ha_role: remdb::ha::HARole::Master,
                replication_mode: remdb::ha::ReplicationMode::Async,
                heartbeat_interval_ms: 1000,
                failure_detection_ms: 5000,
                sync_timeout_ms: 1000,
                master_address: None,
                master_port: None,
                replication_port: 5556,
                heartbeat_port: 5557,
            }),
        })
    }?;
    
    // 创建表
    let fields = &[
        ("id", remdb::DataType::UInt64, None),
        ("name", remdb::DataType::String, None),
        ("value", remdb::DataType::UInt32, None),
    ];
    
    db.create_table("test_table", fields, Some(0))?;
    
    // 为name字段创建索引
    db.create_index("test_table", "name", remdb::IndexType::BTree)?;
    
    // 插入测试数据
    for i in 0..5 {
        let sql = format!("INSERT INTO test_table (id, name, value) VALUES ({}, 'item_{}', {})", i, i, i * 100);
        db.sql_query(&sql)?;
    }
    
    Ok(())
}
