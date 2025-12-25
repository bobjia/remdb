//! SQL查询单元测试
//! 
//! 该测试文件验证SQL查询功能的正确性。

#![cfg(feature = "std")]

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

// 定义测试表结构
remdb::table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(32),
        age: i8,
        active: bool,
        created_at: u64
    }
);

// 定义测试数据库配置
remdb::database!(
    TEST_DB,
    tables: [TEST_TABLE]
);

#[test]
fn test_sql_query() {
    // 初始化内存缓冲区
    let mut db_memory = [0u8; 65536];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            db_memory.as_mut_ptr(),
            db_memory.len()
        );
    }
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 计算所需内存大小
    let config = &TEST_DB;
    let table_size = MemoryTable::calculate_memory_size(&config.tables[0]);
    let primary_index_size = PrimaryIndex::calculate_memory_size(
        &config.tables[0],
        128, // 哈希表大小
        100  // 最大索引项数量
    );
    let secondary_index_size = SecondaryIndex::calculate_memory_size(100);
    
    // 分配内存
    let table_ptr = unsafe {
        remdb::memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8
    };
    let status_ptr = unsafe {
        remdb::memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader
    };
    let free_slots_ptr = unsafe {
        remdb::memory::allocator::alloc(
            core::mem::size_of::<usize>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut usize
    };
    let hash_table_ptr = unsafe {
        remdb::memory::allocator::alloc(
            128 * core::mem::size_of::<Option<core::ptr::NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<core::ptr::NonNull<index::PrimaryIndexItem>>
    };
    let primary_index_items_ptr = unsafe {
        remdb::memory::allocator::alloc(
            100 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem
    };
    let secondary_index_items_ptr = unsafe {
        remdb::memory::allocator::alloc(
            100 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem
    };
    
    // 创建表和索引
    let mut table = unsafe {
        MemoryTable::new(&config.tables[0], table_ptr, status_ptr, free_slots_ptr).unwrap()
    };
    let mut primary_index = unsafe {
        PrimaryIndex::new(
            &config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            128,
            100
        )
    };
    let mut secondary_index = unsafe {
        SecondaryIndex::new(
            &config.tables[0],
            secondary_index_items_ptr,
            100
        )
    };
    
    // 初始化表和索引数组
    static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
    static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
    static mut SECONDARY_INDICES: [Option<AnySecondaryIndex>; 1] = [None; 1];
    
    unsafe {
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(AnySecondaryIndex::SortedArray(secondary_index));
    }
    
    // 初始化数据库
    let db = unsafe {
        init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap()
    };
    
    // 插入测试数据
    #[repr(C)]
    struct TestRecord {
        id: i32,
        name: [u8; 32],
        age: i8,
        active: bool,
        created_at: u64,
    }
    
    // 准备测试数据
    let test_data = [
        (1, "Alice", 25, true, 1620000000000),
        (2, "Bob", 30, true, 1620000001000),
        (3, "Charlie", 35, false, 1620000002000),
        (4, "David", 22, true, 1620000003000),
        (5, "Eve", 28, false, 1620000004000),
    ];
    
    for (id, name, age, active, created_at) in test_data {
        let mut record = TestRecord {
            id,
            name: [0u8; 32],
            age,
            active,
            created_at,
        };
        
        let name_bytes = name.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);
        
        let insert_id = unsafe {
            db.get_table_mut(0).unwrap().insert(&record as *const _ as *const u8).unwrap()
        };
        // insert返回的是槽位索引，不是记录的id字段值
        assert!(insert_id < config.tables[0].max_records);
    }
    
    // 测试SQL查询
    
    // 1. 测试基本SELECT查询
    let result = db.sql_query("SELECT * FROM TEST_TABLE").unwrap();
    assert_eq!(result.row_count(), 5);
    assert_eq!(result.column_count(), 5);
    
    // 2. 测试SELECT特定列
    let result = db.sql_query("SELECT name, age FROM TEST_TABLE").unwrap();
    assert_eq!(result.row_count(), 5);
    assert_eq!(result.column_count(), 2);
    
    // 3. 测试SELECT带WHERE条件
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE age > 25").unwrap();
    assert_eq!(result.row_count(), 3);
    
    // 4. 测试SELECT带WHERE条件和ORDER BY
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE active = true ORDER BY age ASC").unwrap();
    assert_eq!(result.row_count(), 3);
    
    // 5. 测试SELECT带LIMIT
    let result = db.sql_query("SELECT * FROM TEST_TABLE LIMIT 2").unwrap();
    assert_eq!(result.row_count(), 2);
    
    // 6. 测试SELECT带WHERE条件和LIMIT
    let result = db.sql_query("SELECT * FROM TEST_TABLE WHERE active = false LIMIT 1").unwrap();
    assert_eq!(result.row_count(), 1);
    
    // 7. 测试无效表名
    let result = db.sql_query("SELECT * FROM invalid_table");
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::TableNotFound));
    }
    
    // 8. 测试无效字段名
    let result = db.sql_query("SELECT invalid_field FROM TEST_TABLE");
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(matches!(err, RemDbError::FieldNotFound));
    }
}

#[test]
fn test_sql_query_syntax() {
    // 初始化内存缓冲区
    let mut db_memory = [0u8; 65536];
    
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(
            db_memory.as_mut_ptr(),
            db_memory.len()
        );
    }
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 计算所需内存大小
    let config = &TEST_DB;
    let table_size = MemoryTable::calculate_memory_size(&config.tables[0]);
    let primary_index_size = PrimaryIndex::calculate_memory_size(
        &config.tables[0],
        128,
        100
    );
    let secondary_index_size = SecondaryIndex::calculate_memory_size(100);
    
    // 分配内存
    let table_ptr = unsafe {
        remdb::memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8
    };
    let status_ptr = unsafe {
        remdb::memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader
    };
    let free_slots_ptr = unsafe {
        remdb::memory::allocator::alloc(
            core::mem::size_of::<usize>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut usize
    };
    let hash_table_ptr = unsafe {
        remdb::memory::allocator::alloc(
            128 * core::mem::size_of::<Option<core::ptr::NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<core::ptr::NonNull<index::PrimaryIndexItem>>
    };
    let primary_index_items_ptr = unsafe {
        remdb::memory::allocator::alloc(
            100 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem
    };
    let secondary_index_items_ptr = unsafe {
        remdb::memory::allocator::alloc(
            100 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem
    };
    
    // 创建表和索引
    let mut table = unsafe {
        MemoryTable::new(&config.tables[0], table_ptr, status_ptr, free_slots_ptr).unwrap()
    };
    let mut primary_index = unsafe {
        PrimaryIndex::new(
            &config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            128,
            100
        )
    };
    let mut secondary_index = unsafe {
        SecondaryIndex::new(
            &config.tables[0],
            secondary_index_items_ptr,
            100
        )
    };
    
    // 初始化表和索引数组
    static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
    static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
    static mut SECONDARY_INDICES: [Option<AnySecondaryIndex>; 1] = [None; 1];
    
    unsafe {
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(AnySecondaryIndex::SortedArray(secondary_index));
    }
    
    // 初始化数据库
    let db = unsafe {
        init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap()
    };
    
    // 测试各种SQL语法
    
    // 1. 测试有效SQL语法
    let valid_queries = [
        "SELECT * FROM TEST_TABLE",
        "SELECT name, age FROM TEST_TABLE",
        "SELECT * FROM TEST_TABLE WHERE id = 1",
        "SELECT * FROM TEST_TABLE WHERE age > 25 AND active = true",
        "SELECT * FROM TEST_TABLE ORDER BY name ASC",
        "SELECT * FROM TEST_TABLE ORDER BY age DESC",
        "SELECT * FROM TEST_TABLE LIMIT 5",
        "SELECT * FROM TEST_TABLE WHERE active = false LIMIT 2",
    ];
    
    for query in valid_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok(), "查询 '{}' 应该成功执行", query);
    }
    
    // 2. 测试无效SQL语法
    let invalid_queries = [
        "SELECT", // 缺少FROM子句
        "SELECT *", // 缺少FROM子句
        "SELECT * FROM", // 缺少表名
        "SELECT * FROM WHERE id = 1", // 缺少表名
        "SELECT * FROM TEST_TABLE WHERE", // 缺少条件
        "SELECT * FROM TEST_TABLE WHERE id", // 缺少比较运算符和值
        "SELECT * FROM TEST_TABLE ORDER BY", // 缺少排序列
        "SELECT * FROM TEST_TABLE LIMIT", // 缺少LIMIT值
    ];
    
    for query in invalid_queries {
        let result = db.sql_query(query);
        assert!(result.is_err(), "查询 '{}' 应该失败", query);
    }
}
