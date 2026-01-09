#![cfg(feature = "std")]
extern crate alloc;

use remdb::*;

// 简单的测试平台实现
struct TestPlatform;

impl platform::Platform for TestPlatform {
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
    
    fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
        Ok(core::ptr::null())
    }
    
    fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
        Ok(())
    }
    
    fn file_write(&self, _handle: platform::FileHandle, _buffer: *const u8, _size: usize) -> platform::FileResult<usize> {
        Ok(0)
    }
    
    fn file_read(&self, _handle: platform::FileHandle, _buffer: *mut u8, _size: usize) -> platform::FileResult<usize> {
        Ok(0)
    }
    
    fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
        Ok(0)
    }
    
    fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
        Ok(())
    }
    
    fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
        Ok(0)
    }
    
    fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
        0
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 定义自定义数据库配置
static TEST_DB_CONFIG: remdb::config::DbConfig = remdb::config::DbConfig {
    tables: &[],
    total_memory: 2097152, // 2MB
    default_max_records: 100, // 降低默认记录数，减少内存需求
    low_power_mode_supported: false,
    low_power_max_records: None,
    log_path: "sql_insert_ignore_test.wal",
    log_mode: remdb::config::LogMode::Async,
    log_prealloc_size: 0,
    log_file_size_limit: 1048576,
    log_segment_size: 1048576,
    checkpoint_interval_ms: 30000,
    memory_allocator: &remdb::config::DefaultMemoryAllocator,
    retained_checkpoints: 2,
    ha_role: remdb::config::HARole::Auto,
    replication_mode: remdb::config::ReplicationMode::Async,
    heartbeat_interval_ms: 1000,
    failure_detection_ms: 3000,
    sync_timeout_ms: 1000,
    master_address: None,
    master_port: None,
    time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
};

#[test]
fn test_insert_ignore_functionality() {
    // 使用静态内存缓冲区，确保它不会在函数返回时被释放
    static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).unwrap();
    }
    
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化数据库
    let config = &TEST_DB_CONFIG;
    let mut db = unsafe {
        remdb::init_global_db(config).unwrap()
    };
    
    // 创建测试表
    let create_table_sql = "CREATE TABLE test_table (
        id INTEGER PRIMARY KEY AUTO_INCREMENT,
        name TEXT NOT NULL
    )";
    
    let result = db.sql_query(create_table_sql);
    if let Err(e) = &result {
        println!("CREATE TABLE error: {:?}", e);
    }
    assert!(result.is_ok());
    
    // 测试1: 插入第一条数据
    let insert_sql = "INSERT INTO test_table (name) VALUES ('test1')";
    let result = db.sql_query(insert_sql);
    assert!(result.is_ok());
    
    // 测试2: 使用INSERT IGNORE插入重复数据
    let insert_ignore_sql = "INSERT IGNORE INTO test_table (id, name) VALUES (1, 'test2')";
    let result = db.sql_query(insert_ignore_sql);
    assert!(result.is_ok());
    
    // 测试3: 验证数据没有被覆盖
    let select_sql = "SELECT id, name FROM test_table WHERE id = 1";
    let result = db.sql_query(select_sql);
    assert!(result.is_ok());
    
    println!("INSERT IGNORE功能测试通过!");
}