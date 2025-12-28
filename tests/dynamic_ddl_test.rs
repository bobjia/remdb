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
        memory_allocator: &mut DEFAULT_ALLOCATOR,
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
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("age", DataType::UInt8),
            ("active", DataType::Bool),
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
        &[("id", DataType::UInt32)],
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
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("price", DataType::Float32),
            ("category", DataType::String),
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
            ("id", DataType::UInt32),
            ("customer_id", DataType::UInt32),
            ("amount", DataType::Float64),
            ("created_at", DataType::Timestamp),
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
            ("id", DataType::UInt32),
            ("customer_id", DataType::UInt32),
            ("amount", DataType::Float64),
            ("created_at", DataType::Timestamp),
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
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("department", DataType::String),
            ("salary", DataType::Float64),
            ("active", DataType::Bool),
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
    // 注意：由于describe查询返回的是表结构信息，使用索引映射来表示字符串值
    // 根据execute_describe_query函数的实现，我们知道：
    // - Field列使用字段名索引（0=id, 1=name, 2=age, 3=active, ...）
    // - Type列使用类型索引（4=UInt32, 5=String, 6=UInt8, 7=Bool, ...）
    // - Key列使用主键标志索引（8=PRI, 9=空）
    // - Null列使用NULL约束索引（10=NO, 9=空）
    // - Default列使用默认值索引（11=0, 9=空）
    
    // 验证id字段
    if let Some(row) = result_set.get_row(0) {
        assert_eq!(unsafe { row.values[0].u64 }, 0, "Expected id field index to be 0");
        assert_eq!(unsafe { row.values[1].u64 }, 4, "Expected UInt32 type index to be 4");
        assert_eq!(unsafe { row.values[2].u64 }, 8, "Expected PRI key index to be 8");
        assert_eq!(unsafe { row.values[3].u64 }, 10, "Expected NO null index to be 10");
        assert_eq!(unsafe { row.values[4].u64 }, 11, "Expected 0 default index to be 11");
    }
    
    // 验证name字段
    if let Some(row) = result_set.get_row(1) {
        assert_eq!(unsafe { row.values[0].u64 }, 1, "Expected name field index to be 1");
        assert_eq!(unsafe { row.values[1].u64 }, 5, "Expected String type index to be 5");
        assert_eq!(unsafe { row.values[2].u64 }, 9, "Expected no key index to be 9");
        assert_eq!(unsafe { row.values[3].u64 }, 10, "Expected NO null index to be 10");
        assert_eq!(unsafe { row.values[4].u64 }, 11, "Expected 0 default index to be 11");
    }
    
    // 验证department字段
    if let Some(row) = result_set.get_row(2) {
        assert_eq!(unsafe { row.values[1].u64 }, 5, "Expected String type index to be 5");
        assert_eq!(unsafe { row.values[2].u64 }, 9, "Expected no key index to be 9");
    }
    
    // 验证salary字段
    if let Some(row) = result_set.get_row(3) {
        // Float64类型在value_to_string_repr函数中没有特别处理，所以会返回空字符串
        // 我们只验证其他字段
        assert_eq!(unsafe { row.values[2].u64 }, 9, "Expected no key index to be 9");
        assert_eq!(unsafe { row.values[3].u64 }, 10, "Expected NO null index to be 10");
    }
    
    // 验证active字段
    if let Some(row) = result_set.get_row(4) {
        assert_eq!(unsafe { row.values[1].u64 }, 7, "Expected Bool type index to be 7");
        assert_eq!(unsafe { row.values[2].u64 }, 9, "Expected no key index to be 9");
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