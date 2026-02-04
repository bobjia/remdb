use remdb::{init_global_db, reset_global_db, CompressionType, config::{DbConfig, DefaultMemoryAllocator, WALConfig, TimeSeriesConfig, HAConfig, LogMode}, pubsub::PubSubConfig, TableDef, FieldDef, DataType, IndexType};
use serial_test::serial;

// 简单的测试平台实现
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

    fn file_open(
        &self,
        _path: &str,
        _mode: remdb::platform::FileMode,
    ) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
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

    fn file_read(
        &self,
        _handle: remdb::platform::FileHandle,
        _buffer: *mut u8,
        _size: usize,
    ) -> remdb::platform::FileResult<usize> {
        Ok(0)
    }

    fn file_seek(
        &self,
        _handle: remdb::platform::FileHandle,
        _offset: i64,
        _whence: remdb::platform::SeekWhence,
    ) -> remdb::platform::FileResult<u64> {
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

/// 设置测试环境
fn setup_test() {
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 使用堆分配的内存缓冲区，确保测试之间的隔离，避免栈溢出
    let mut db_memory = Vec::with_capacity(8388608); // 8MB内存缓冲区，足够系统表初始化和MVCC使用
    db_memory.resize(8388608, 0);

    // 打印内存缓冲区信息
    println!("Memory buffer: ptr={:p}, len={}", db_memory.as_mut_ptr(), db_memory.len());

    // 初始化内存分配器
    match remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()) {
        Ok(_) => println!("Memory allocator initialized successfully"),
        Err(e) => println!("Failed to initialize memory allocator: {:?}", e),
    }

    // 泄漏内存，使其在测试期间保持有效
    core::mem::forget(db_memory);

    // 打印内存统计信息
    let stats = remdb::memory::allocator::get_memory_stats();
    println!("Initial memory stats: used={}, total={}, fragmentation={:.2}", stats.used, stats.total, stats.fragmentation);

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 打印内存统计信息
    let stats = remdb::memory::allocator::get_memory_stats();
    println!("Memory stats after reset: used={}, total={}, fragmentation={:.2}", stats.used, stats.total, stats.fragmentation);
}

/// 清理测试环境
fn teardown_test() {
    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();
}

// 定义简单的测试表
remdb::table!(
    DOCS_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i64,
        text: str(256)
    }
);

// 定义测试数据库配置
remdb::database!(
    MINIMAL_DB,
    tables: []
);

// 定义包含文档表的测试数据库配置
remdb::database!(
    DOCS_DB,
    tables: [DOCS_TABLE]
);

#[test]
#[serial]
fn test_create_model_statement() {
    setup_test();

    // Initialize a minimal database
    let db = init_global_db(&MINIMAL_DB).unwrap();
    
    // Test CREATE MODEL statement
    let create_model_sql = "CREATE MODEL udf_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);";
    let result = db.sql_query(create_model_sql);
    
    // The statement should execute successfully (even if model file doesn't exist in test)
    assert!(result.is_ok(), "CREATE MODEL should succeed");

    teardown_test();
}

#[test]
#[serial]
fn test_model_udf_in_query() {
    setup_test();

    // Initialize a database with a test table
    let db = init_global_db(&DOCS_DB).unwrap();
    
    // Create test data
    let insert_sql = "INSERT INTO DOCS_TABLE (text) VALUES ('Hello world'), ('Test document');";
    let result = db.sql_query(insert_sql);
    assert!(result.is_ok(), "INSERT should succeed");
    
    // Register model
    let create_model_sql = "CREATE MODEL udf_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);";
    let result = db.sql_query(create_model_sql);
    assert!(result.is_ok(), "CREATE MODEL should succeed");
    
    // Test model UDF in query
    let select_sql = "SELECT id, udf_embedding(text) AS embedding FROM DOCS_TABLE;";
    let result = db.sql_query(select_sql);
    
    // The query should execute (even if model returns dummy data)
    assert!(result.is_ok(), "SELECT with model UDF should succeed");

    teardown_test();
}

#[test]
#[serial]
fn test_invalid_model() {
    setup_test();

    // Initialize a minimal database
    let db = init_global_db(&MINIMAL_DB).unwrap();
    
    // Test using non-existent model
    let select_sql = "SELECT non_existent_model(text) AS embedding FROM DOCS_TABLE;";
    let result = db.sql_query(select_sql);
    
    // The query should fail with unsupported function error
    assert!(result.is_err(), "SELECT with non-existent model should fail");

    teardown_test();
}

#[test]
#[serial]
fn test_model_with_multiple_inputs() {
    setup_test();

    // Initialize a minimal database
    let db = init_global_db(&MINIMAL_DB).unwrap();
    
    // Test CREATE MODEL with multiple inputs
    let create_model_sql = "CREATE MODEL multi_input_model USING 'multi-input.onnx' AS (text1 STRING, text2 STRING) RETURNS VECTOR(768);";
    let result = db.sql_query(create_model_sql);
    
    // The statement should execute successfully
    assert!(result.is_ok(), "CREATE MODEL with multiple inputs should succeed");

    teardown_test();
}

#[test]
#[serial]
fn test_duplicate_model_name() {
    setup_test();

    // Initialize a minimal database
    let db = init_global_db(&MINIMAL_DB).unwrap();
    
    // First CREATE MODEL should succeed
    let create_model_sql = "CREATE MODEL my_model USING 'model.onnx' AS (text STRING) RETURNS VECTOR(768);";
    let result = db.sql_query(create_model_sql);
    assert!(result.is_ok(), "First CREATE MODEL should succeed");
    
    // Second CREATE MODEL with same name should fail
    let result2 = db.sql_query(create_model_sql);
    assert!(result2.is_err(), "Duplicate CREATE MODEL should fail");

    teardown_test();
}

#[test]
#[serial]
fn test_model_udf_incorrect_arguments() {
    setup_test();

    // Initialize a minimal database
    let db = init_global_db(&MINIMAL_DB).unwrap();
    
    // Register model
    let create_model_sql = "CREATE MODEL udf_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);";
    let result = db.sql_query(create_model_sql);
    assert!(result.is_ok(), "CREATE MODEL should succeed");
    
    // Test with wrong number of arguments - should fail
    let select_sql = "SELECT udf_embedding(text, extra) FROM DOCS_TABLE;";
    let result = db.sql_query(select_sql);
    assert!(result.is_err(), "Model UDF with wrong argument count should fail");
    
    // Test with no arguments - should fail
    let select_sql2 = "SELECT udf_embedding() FROM DOCS_TABLE;";
    let result2 = db.sql_query(select_sql2);
    assert!(result2.is_err(), "Model UDF with no arguments should fail");

    teardown_test();
}

#[test]
#[serial]
fn test_model_udf_in_where_clause() {
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            setup_test();

            // Initialize a database with a test table
            let db = init_global_db(&DOCS_DB).unwrap();
            
            // Create test data
            let insert_sql = "INSERT INTO DOCS_TABLE (text) VALUES ('Hello world'), ('Test document');";
            let result = db.sql_query(insert_sql);
            assert!(result.is_ok(), "INSERT should succeed");
            
            // Register model
            let create_model_sql = "CREATE MODEL udf_embedding USING 'bge-m3.onnx' AS (text STRING) RETURNS VECTOR(768);";
            let result = db.sql_query(create_model_sql);
            assert!(result.is_ok(), "CREATE MODEL should succeed");
            
            // Test model UDF in WHERE clause - should execute (even if dummy results)
            let select_sql = "SELECT id FROM DOCS_TABLE WHERE udf_embedding(text) = udf_embedding(text);";
            let result = db.sql_query(select_sql);
            assert!(result.is_ok(), "SELECT with model UDF in WHERE clause should succeed");

            teardown_test();
        })
        .unwrap()
        .join()
        .unwrap();
}
