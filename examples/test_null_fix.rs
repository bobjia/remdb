use remdb::RemDb;
use remdb::config::{DbConfig, DefaultMemoryAllocator, WALConfig, LogMode};
use remdb::platform::{init_platform, Platform, FileMode, FileHandle, FileResult, SeekWhence};
use remdb::sql::{parse_sql_query, execute_query};

/// Simple test platform implementation
struct TestPlatform;

impl Platform for TestPlatform {
    fn get_timestamp(&self) -> u64 {
        1234567890
    }

    fn get_timestamp_us(&self) -> u64 {
        1234567890123
    }

    fn spin_lock(&self, lock: &mut u32) {
        // Simple spin lock implementation
        while unsafe {
            core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .compare_exchange(
                    0,
                    1,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
        } {
            core::hint::spin_loop();
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

    fn delay_ms(&self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }

    fn delay_us(&self, us: u32) {
        std::thread::sleep(std::time::Duration::from_micros(us as u64));
    }

    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> {
        Err(())
    }

    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Err(())
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

fn main() {
    // Initialize platform
    static TEST_PLATFORM: TestPlatform = TestPlatform;
    init_platform(&TEST_PLATFORM);
    
    // Initialize global memory allocator
    let memory_size = 1024 * 1024 * 500; // 500MB
    let mut memory = vec![0u8; memory_size];
    let memory_ptr = memory.as_mut_ptr();
    if let Err(e) = remdb::memory::allocator::init_global_allocator(memory_ptr, memory_size) {
        println!("Failed to initialize memory allocator: {:?}", e);
        return;
    }
    
    // Leak the memory to prevent it from being freed
    std::mem::forget(memory);
    
    // Create database config
    static MEMORY_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    static DB_CONFIG: DbConfig = DbConfig {
        tables: vec!(),
        total_memory: 1024 * 1024 * 500, // 500MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100000,
        memory_allocator: &MEMORY_ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig {
            partition_duration_secs: 3600, // 1 hour
            retention_period_secs: 30 * 24 * 3600, // 30 days
            compression: remdb::time_series::CompressionType::None,
            max_partitions: 1000,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
    };
    
    // Create a temporary database for testing
    let mut db = RemDb::new(&DB_CONFIG);
    
    // Create the test table
    let create_table_sql = "CREATE TABLE test_null_values ( 
             id INTEGER PRIMARY KEY, 
             int_val INTEGER, 
             real_val REAL, 
             text_val TEXT, 
             bool_val BOOLEAN, 
             ts_val TIMESTAMP 
         )";
    
    match parse_sql_query(create_table_sql) {
        Ok(query) => {
            match execute_query(&mut db, &query) {
                Ok(_) => println!("Create table result: Success"),
                Err(e) => println!("Create table error: {:?}", e),
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
    
    // Insert test data
    let insert_sql = "INSERT INTO test_null_values (id, int_val, text_val, bool_val) 
             VALUES (2, 100, 'test', TRUE)";
    
    match parse_sql_query(insert_sql) {
        Ok(query) => {
            match execute_query(&mut db, &query) {
                Ok(_) => println!("Insert result: Success"),
                Err(e) => println!("Insert error: {:?}", e),
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
    
    // Test IS NULL operation - this was failing before the fix
    let select_sql = "SELECT int_val IS NULL as int_null, text_val IS NULL as text_null FROM test_null_values ORDER BY id";
    
    match parse_sql_query(select_sql) {
        Ok(query) => {
            match execute_query(&mut db, &query) {
                Ok(_) => println!("Select IS NULL result: Success"),
                Err(e) => println!("Select IS NULL error: {:?}", e),
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
    
    // Also test IS NOT NULL for completeness
    let select_not_null_sql = "SELECT int_val IS NOT NULL as int_not_null, text_val IS NOT NULL as text_not_null FROM test_null_values ORDER BY id";
    
    match parse_sql_query(select_not_null_sql) {
        Ok(query) => {
            match execute_query(&mut db, &query) {
                Ok(_) => println!("Select IS NOT NULL result: Success"),
                Err(e) => println!("Select IS NOT NULL error: {:?}", e),
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
    
    println!("Test completed successfully!");
}