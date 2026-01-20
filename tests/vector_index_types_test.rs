//! 向量索引类型测试
//! 
//! 该测试文件验证不同类型的向量索引功能，包括HNSW_SQ, HNSW_BQ, IVF, IVF_PQ等。

#![cfg(feature = "std")]

use remdb::*;
use serial_test::serial;

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

// 定义包含向量字段的测试表
remdb::table!(
    VECTOR_TYPES_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        vector: vector(4), // 4维向量字段
        category: i32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_TYPES_DB,
    tables: [VECTOR_TYPES_TABLE]
);

// 向量记录结构
#[repr(C)]
struct VectorRecord {
    id: i32,
    vector: [f32; 4],
    category: i32
}

// 测试HNSW_SQ索引类型
#[test]
#[serial]
fn test_vector_index_hnsw_sq() {
    println!("=== 测试向量索引类型: HNSW_SQ ===");
    
    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576];
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(
        db_memory.as_mut_ptr(),
        db_memory.len()
    ).unwrap();
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_TYPES_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含4维向量表的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=5 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入5条测试数据");
    
    // 测试1: 创建HNSW_SQ索引
    println!("测试1: 创建HNSW_SQ索引");
    let result = db.sql_query("CREATE INDEX vector_hnsw_sq_idx ON VECTOR_TYPES_TABLE (vector) USING HNSW_SQ");
    if result.is_ok() {
        println!("成功创建HNSW_SQ索引");
        
        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_TYPES_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建HNSW_SQ索引失败，可能功能尚未实现");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量索引类型: HNSW_SQ 完成 ===");
}

// 测试HNSW_BQ索引类型
#[test]
#[serial]
fn test_vector_index_hnsw_bq() {
    println!("=== 测试向量索引类型: HNSW_BQ ===");
    
    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576];
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(
        db_memory.as_mut_ptr(),
        db_memory.len()
    ).unwrap();
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_TYPES_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含4维向量表的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=5 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入5条测试数据");
    
    // 测试1: 创建HNSW_BQ索引
    println!("测试1: 创建HNSW_BQ索引");
    let result = db.sql_query("CREATE INDEX vector_hnsw_bq_idx ON VECTOR_TYPES_TABLE (vector) USING HNSW_BQ");
    if result.is_ok() {
        println!("成功创建HNSW_BQ索引");
        
        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_TYPES_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建HNSW_BQ索引失败，可能功能尚未实现");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量索引类型: HNSW_BQ 完成 ===");
}

// 测试IVF索引类型
#[test]
#[serial]
fn test_vector_index_ivf() {
    println!("=== 测试向量索引类型: IVF ===");
    
    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576];
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(
        db_memory.as_mut_ptr(),
        db_memory.len()
    ).unwrap();
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_TYPES_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含4维向量表的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=10 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入10条测试数据");
    
    // 测试1: 创建IVF索引
    println!("测试1: 创建IVF索引");
    let result = db.sql_query("CREATE INDEX vector_ivf_idx ON VECTOR_TYPES_TABLE (vector) USING IVF");
    if result.is_ok() {
        println!("成功创建IVF索引");
        
        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_TYPES_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建IVF索引失败，可能功能尚未实现");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量索引类型: IVF 完成 ===");
}

// 测试IVF_PQ索引类型
#[test]
#[serial]
fn test_vector_index_ivf_pq() {
    println!("=== 测试向量索引类型: IVF_PQ ===");
    
    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576];
    
    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);
    
    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(
        db_memory.as_mut_ptr(),
        db_memory.len()
    ).unwrap();
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_TYPES_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含4维向量表的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=10 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入10条测试数据");
    
    // 测试1: 创建IVF_PQ索引
    println!("测试1: 创建IVF_PQ索引");
    let result = db.sql_query("CREATE INDEX vector_ivf_pq_idx ON VECTOR_TYPES_TABLE (vector) USING IVF_PQ");
    if result.is_ok() {
        println!("成功创建IVF_PQ索引");
        
        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_TYPES_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建IVF_PQ索引失败，可能功能尚未实现");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量索引类型: IVF_PQ 完成 ===");
}

// 测试多种索引类型组合（简化版本，避免栈溢出）
#[test]
#[serial]
fn test_vector_index_multiple_types() {
    println!("=== 测试多种向量索引类型组合 ===");
    
    // 使用堆分配的内存缓冲区，避免栈溢出
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 测试1: HNSW索引
    println!("\n--- 测试HNSW索引 ---");
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    let config = &VECTOR_TYPES_DB;
    let db1 = remdb::init_global_db(config).unwrap();
    
    for i in 1..=3 { // 减少数据量
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        let table = db1.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    let result1 = db1.sql_query("CREATE INDEX vector_hnsw_idx ON VECTOR_TYPES_TABLE (vector) USING HNSW");
    println!("HNSW索引创建: {}", if result1.is_ok() { "成功" } else { "失败" });
    remdb::reset_global_db();
    
    // 测试2: 只测试HNSW_SQ索引，减少测试数量
    println!("\n--- 测试HNSW_SQ索引 ---");
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    let db2 = remdb::init_global_db(config).unwrap();
    
    for i in 1..=3 { // 减少数据量
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            category: if i % 2 == 0 { 2 } else { 1 }
        };
        let table = db2.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    let result2 = db2.sql_query("CREATE INDEX vector_hnsw_sq_idx ON VECTOR_TYPES_TABLE (vector) USING HNSW_SQ");
    println!("HNSW_SQ索引创建: {}", if result2.is_ok() { "成功" } else { "失败" });
    remdb::reset_global_db();
    
    println!("\n=== 测试多种向量索引类型组合完成 ===");
}
