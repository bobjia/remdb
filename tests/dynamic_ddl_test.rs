use remdb::*;
use remdb::platform::{Platform, FileMode, FileHandle, FileResult, SeekWhence};

// 定义测试用的内存缓冲区
static mut DB_MEMORY: [u8; 1024 * 1024] = [0u8; 1024 * 1024]; // 1MB内存

// 静态配置，用于测试
static mut DEFAULT_ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;
static TEST_CONFIG: config::DbConfig = unsafe {
    config::DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100, // 减小值以避免内存不足
        memory_allocator: &mut DEFAULT_ALLOCATOR,
        log_mode: config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: config::HARole::Auto,
        replication_mode: config::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
        time_series_defaults: config::TimeSeriesConfig::DEFAULT,
    }
};

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
    
    fn file_write(&self, _handle: FileHandle, _buffer: *const u8, _size: usize) -> FileResult<usize> {
        Ok(0)
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

static TEST_PLATFORM: TestPlatform = TestPlatform;

#[test]
fn test_create_table() {
    // 初始化平台
    unsafe {
        platform::init_platform(&TEST_PLATFORM);
    }
    
    // 初始化全局内存分配器
    unsafe {
        let result = memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_CONFIG);
    
    // 测试创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, None),
            ("name", DataType::String, None),
            ("age", DataType::UInt8, None),
            ("active", DataType::Bool, None),
        ],
        Some(0) // 主键为id字段
    );
    
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());
}

#[test]
fn test_create_table_invalid() {
    // 无需平台初始化，直接测试参数验证逻辑
    let mut db = RemDb::new(&TEST_CONFIG);
    
    // 测试创建空字段表（应该失败）
    let result = db.create_table("empty_table", &[], None);
    assert!(result.is_err(), "Creating table with empty fields should fail");
    
    // 测试创建主键索引超出范围的表（应该失败）
    let result = db.create_table(
        "invalid_pk_table",
        &[("id", DataType::UInt32, None)],
        Some(1) // 主键索引超出范围
    );
    assert!(result.is_err(), "Creating table with invalid primary key should fail");
}

#[test]
fn test_create_index() {
    // 初始化平台
    unsafe {
        platform::init_platform(&TEST_PLATFORM);
    }
    
    // 初始化全局内存分配器
    unsafe {
        let result = memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_CONFIG);
    
    // 先创建表
    let result = db.create_table(
        "products",
        &[
            ("id", DataType::UInt32, None),
            ("name", DataType::String, None),
            ("price", DataType::Float32, None),
            ("category", DataType::String, None),
        ],
        Some(0) // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());
    
    // 测试创建BTree索引
    let result = db.create_index(
        "products",
        "name",
        IndexType::BTree
    );
    assert!(result.is_ok(), "Failed to create BTree index: {:?}", result.err());
    
    // 测试创建不同类型的索引
    // 为TTree索引创建一个新表
    let result = db.create_table(
        "orders_ttree",
        &[
            ("id", DataType::UInt32, None),
            ("customer_id", DataType::UInt32, None),
            ("amount", DataType::Float64, None),
            ("created_at", DataType::Timestamp, None),
        ],
        Some(0) // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create orders_ttree table: {:?}", result.err());
    
    // 测试创建TTree索引
    let result = db.create_index(
        "orders_ttree",
        "created_at",
        IndexType::TTree
    );
    assert!(result.is_ok(), "Failed to create TTree index: {:?}", result.err());
    
    // 为SortedArray索引创建另一个新表
    let result = db.create_table(
        "orders_sorted",
        &[
            ("id", DataType::UInt32, None),
            ("customer_id", DataType::UInt32, None),
            ("amount", DataType::Float64, None),
            ("created_at", DataType::Timestamp, None),
        ],
        Some(0) // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create orders_sorted table: {:?}", result.err());
    
    // 测试创建SortedArray索引
    let result = db.create_index(
        "orders_sorted",
        "amount",
        IndexType::SortedArray
    );
    assert!(result.is_ok(), "Failed to create SortedArray index: {:?}", result.err());
}

#[test]
fn test_describe_table() {
    // 初始化平台
    unsafe {
        platform::init_platform(&TEST_PLATFORM);
    }
    
    // 初始化全局内存分配器
    unsafe {
        let result = memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        assert!(result.is_ok(), "Failed to initialize global allocator: {:?}", result.err());
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&TEST_CONFIG);
    
    // 先创建表
    let result = db.create_table(
        "employees",
        &[
            ("id", DataType::UInt32, None),
            ("name", DataType::String, None),
            ("department", DataType::String, None),
            ("salary", DataType::Float64, None),
            ("active", DataType::Bool, None),
        ],
        Some(0) // 主键为id字段
    );
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());
    
    // 测试DESCRIBE TABLE指令
    let result = db.sql_query("DESCRIBE TABLE employees");
    assert!(result.is_ok(), "Failed to execute DESCRIBE TABLE: {:?}", result.err());
    
    // 详细验证DESCRIBE结果
    let result_set = result.unwrap();
    
    // 验证结果集列名
    assert_eq!(result_set.columns, ["Field", "Type", "Key", "Null", "Default"]);
    
    // 验证结果集行数（应该等于字段数）
    assert_eq!(result_set.row_count(), 5, "Expected 5 fields in employees table, got {}", result_set.row_count());
    
    // 验证结果集中的字段信息
    // execute_describe_query函数直接将字段信息作为字符串写入结果集
    
    // 辅助函数：将Value中的string转换为&str
    unsafe fn value_to_str(value: &crate::Value) -> &str {
        core::str::from_utf8(&value.string).unwrap().trim_end_matches(char::from(0))
    }
    
    // 辅助函数：查找指定字段名的行
    fn find_row_by_field_name<'a>(result_set: &'a crate::sql::ResultSet, field_name: &str) -> Option<&'a crate::sql::ResultRow> {
        for i in 0..result_set.row_count() {
            if let Some(row) = result_set.get_row(i) {
                let current_field_name = unsafe { value_to_str(&row.values[0].value) };
                if current_field_name == field_name {
                    return Some(row);
                }
            }
        }
        None
    }
    
    // 验证id字段
    if let Some(row) = find_row_by_field_name(&result_set, "id") {
        assert_eq!(unsafe { value_to_str(&row.values[1].value) }, "int", "Expected UInt32 type to be int");
        assert_eq!(unsafe { value_to_str(&row.values[2].value) }, "PRI", "Expected id to be primary key");
        assert_eq!(unsafe { value_to_str(&row.values[3].value) }, "NO", "Expected id to be NOT NULL");
        assert_eq!(unsafe { value_to_str(&row.values[4].value) }, "0", "Expected id default to be 0");
    } else {
        panic!("Could not find id field in describe result");
    }
    
    // 验证name字段
    if let Some(row) = find_row_by_field_name(&result_set, "name") {
        assert_eq!(unsafe { value_to_str(&row.values[1].value) }, "varchar(64)", "Expected name type to be varchar(64)");
        assert_eq!(unsafe { value_to_str(&row.values[2].value) }, "", "Expected name to not be primary key");
        assert_eq!(unsafe { value_to_str(&row.values[3].value) }, "NO", "Expected name to be NOT NULL");
        assert_eq!(unsafe { value_to_str(&row.values[4].value) }, "0", "Expected name default to be 0");
    } else {
        panic!("Could not find name field in describe result");
    }
    
    // 验证salary字段
    if let Some(row) = find_row_by_field_name(&result_set, "salary") {
        assert_eq!(unsafe { value_to_str(&row.values[1].value) }, "double", "Expected salary type to be double");
        assert_eq!(unsafe { value_to_str(&row.values[2].value) }, "", "Expected salary to not be primary key");
        assert_eq!(unsafe { value_to_str(&row.values[3].value) }, "NO", "Expected salary to be NOT NULL");
        assert_eq!(unsafe { value_to_str(&row.values[4].value) }, "0", "Expected salary default to be 0");
    } else {
        panic!("Could not find salary field in describe result");
    }
    
    // 验证active字段
    if let Some(row) = find_row_by_field_name(&result_set, "active") {
        assert_eq!(unsafe { value_to_str(&row.values[1].value) }, "bool", "Expected active type to be bool");
        assert_eq!(unsafe { value_to_str(&row.values[2].value) }, "", "Expected active to not be primary key");
        assert_eq!(unsafe { value_to_str(&row.values[3].value) }, "NO", "Expected active to be NOT NULL");
        assert_eq!(unsafe { value_to_str(&row.values[4].value) }, "0", "Expected active default to be 0");
    } else {
        panic!("Could not find active field in describe result");
    }
    
    // 验证department字段
    if let Some(row) = find_row_by_field_name(&result_set, "department") {
        assert_eq!(unsafe { value_to_str(&row.values[1].value) }, "varchar(64)", "Expected department type to be varchar(64)");
        assert_eq!(unsafe { value_to_str(&row.values[2].value) }, "", "Expected department to not be primary key");
        assert_eq!(unsafe { value_to_str(&row.values[3].value) }, "NO", "Expected department to be NOT NULL");
        assert_eq!(unsafe { value_to_str(&row.values[4].value) }, "0", "Expected department default to be 0");
    } else {
        panic!("Could not find department field in describe result");
    }
    
    // 测试简写形式DESCRIBE employees
    let result = db.sql_query("DESCRIBE employees");
    assert!(result.is_ok(), "Failed to execute DESCRIBE employees: {:?}", result.err());
    
    // 验证简写形式的结果
    let short_result_set = result.unwrap();
    assert_eq!(short_result_set.columns, ["Field", "Type", "Key", "Null", "Default"]);
    assert_eq!(short_result_set.row_count(), 5, "Expected 5 fields in employees table, got {}", short_result_set.row_count());
    
    // 测试对不存在的表执行DESCRIBE（应该失败）
    let result = db.sql_query("DESCRIBE non_existent_table");
    assert!(result.is_err(), "DESCRIBE on non-existent table should fail");
}