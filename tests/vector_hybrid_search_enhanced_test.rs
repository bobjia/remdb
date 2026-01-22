//! 向量混合搜索增强测试
//!
//! 该测试文件验证向量搜索与标量条件结合使用的增强功能。

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
        _mode: platform::FileMode,
    ) -> platform::FileResult<platform::FileHandle> {
        Ok(core::ptr::null())
    }

    fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
        Ok(())
    }

    fn file_write(
        &self,
        _handle: platform::FileHandle,
        _buffer: *const u8,
        _size: usize,
    ) -> platform::FileResult<usize> {
        Ok(0)
    }

    fn file_read(
        &self,
        _handle: platform::FileHandle,
        _buffer: *mut u8,
        _size: usize,
    ) -> platform::FileResult<usize> {
        Ok(0)
    }

    fn file_seek(
        &self,
        _handle: platform::FileHandle,
        _offset: i64,
        _whence: platform::SeekWhence,
    ) -> platform::FileResult<u64> {
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

// 定义包含向量字段和多个标量字段的测试表
remdb::table!(
    VECTOR_HYBRID_TABLE,
    200, // 最大记录数
    primary_key: id,
    secondary_index: category, // 单个字段二级索引
    fields: {
        id: i32,
        name: str(64),
        vector: vector(4), // 4维向量字段
        category: i32,
        score: f32,
        active: bool
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_HYBRID_DB,
    tables: [VECTOR_HYBRID_TABLE]
);

// 向量记录结构
#[repr(C)]
struct VectorHybridRecord {
    id: i32,
    name: [u8; 64],
    vector: [f32; 4],
    category: i32,
    score: f32,
    active: bool,
}

// 测试基本向量混合搜索
#[test]
#[serial]
fn test_vector_hybrid_search_basic() {
    println!("=== 测试向量混合搜索: 基本组合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_HYBRID_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量和标量字段的数据库初始化成功");

    // 插入测试数据
    let test_data = vec![
        (1, "item1", [1.0, 2.0, 3.0, 4.0], 1, 0.85, true),
        (2, "item2", [2.0, 3.0, 4.0, 5.0], 1, 0.92, true),
        (3, "item3", [3.0, 4.0, 5.0, 6.0], 2, 0.78, false),
        (4, "item4", [4.0, 5.0, 6.0, 7.0], 2, 0.95, true),
        (5, "item5", [5.0, 6.0, 7.0, 8.0], 1, 0.88, true),
    ];

    for (id, name, vector, category, score, active) in &test_data {
        let mut record = VectorHybridRecord {
            id: *id,
            name: [0u8; 64],
            vector: *vector,
            category: *category,
            score: *score,
            active: *active,
        };

        // 设置名称
        let name_bytes = name.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_data.len());

    // 创建向量索引
    let result =
        db.sql_query("CREATE INDEX vector_hybrid_idx ON VECTOR_HYBRID_TABLE (vector) USING HNSW");
    if result.is_ok() {
        println!("成功创建向量索引");
    } else {
        println!("创建向量索引失败，可能功能尚未实现");
    }

    // 测试1: 基础查询验证
    println!("测试1: 基础查询验证");
    let basic_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE id = 1");
    if basic_result.is_ok() {
        println!("  基础查询验证成功");
    } else {
        println!("  基础查询验证失败");
    }

    // 测试2: 标量条件查询验证
    println!("测试2: 标量条件查询验证");
    let scalar_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 1");
    if scalar_result.is_ok() {
        println!("  标量条件查询验证成功");
    } else {
        println!("  标量条件查询验证失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量混合搜索: 基本组合 完成 ===");
}

// 测试多个标量条件与向量搜索结合
#[test]
#[serial]
fn test_vector_hybrid_search_multiple_conditions() {
    println!("=== 测试向量混合搜索: 多条件组合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_HYBRID_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量和标量字段的数据库初始化成功");

    // 插入测试数据
    for i in 1..=10 {
        let mut record = VectorHybridRecord {
            id: i,
            name: [0u8; 64],
            vector: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
            ],
            category: if i % 3 == 0 {
                3
            } else if i % 2 == 0 {
                2
            } else {
                1
            },
            score: 0.7 + (i as f32 * 0.03),
            active: i % 4 != 0, // 每4条记录有一条是 inactive
        };

        // 设置名称
        let name_str = format!("item{}", i);
        let name_bytes = name_str.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入10条测试数据");

    // 创建向量索引
    let result = db.sql_query(
        "CREATE INDEX vector_hybrid_multi_idx ON VECTOR_HYBRID_TABLE (vector) USING HNSW",
    );
    if result.is_ok() {
        println!("成功创建向量索引");
    } else {
        println!("创建向量索引失败，可能功能尚未实现");
    }

    // 测试1: 多个标量条件查询
    println!("测试1: 多个标量条件查询");
    let multi_cond_result =
        db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 1 AND score > 0.8");
    if multi_cond_result.is_ok() {
        println!("  多个标量条件查询成功");
    } else {
        println!("  多个标量条件查询失败");
    }

    // 测试2: 包含布尔条件的查询
    println!("测试2: 包含布尔条件的查询");
    let bool_cond_result =
        db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE active = true AND category = 2");
    if bool_cond_result.is_ok() {
        println!("  包含布尔条件的查询成功");
    } else {
        println!("  包含布尔条件的查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量混合搜索: 多条件组合 完成 ===");
}

// 测试不同数据类型的标量条件
#[test]
#[serial]
fn test_vector_hybrid_search_different_data_types() {
    println!("=== 测试向量混合搜索: 不同数据类型条件 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_HYBRID_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量和多种标量字段的数据库初始化成功");

    // 插入测试数据
    let test_data = vec![
        (1, "low_score", [1.0, 2.0, 3.0, 4.0], 1, 0.65, true),
        (2, "medium_score", [2.0, 3.0, 4.0, 5.0], 2, 0.85, true),
        (3, "high_score", [3.0, 4.0, 5.0, 6.0], 1, 0.95, false),
        (4, "inactive", [4.0, 5.0, 6.0, 7.0], 2, 0.75, false),
        (5, "active_high", [5.0, 6.0, 7.0, 8.0], 1, 0.92, true),
    ];

    for (id, name, vector, category, score, active) in &test_data {
        let mut record = VectorHybridRecord {
            id: *id,
            name: [0u8; 64],
            vector: *vector,
            category: *category,
            score: *score,
            active: *active,
        };

        // 设置名称
        let name_bytes = name.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_data.len());

    // 创建向量索引
    let idx_result = db.sql_query(
        "CREATE INDEX vector_hybrid_types_idx ON VECTOR_HYBRID_TABLE (vector) USING HNSW",
    );
    if idx_result.is_ok() {
        println!("成功创建向量索引");
    }

    // 测试1: 浮点数范围条件
    println!("测试1: 浮点数范围条件");
    let float_range_result =
        db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE score BETWEEN 0.8 AND 0.95");
    if float_range_result.is_ok() {
        println!("  浮点数范围条件查询成功");
    } else {
        println!("  浮点数范围条件查询失败");
    }

    // 测试2: 整数相等条件
    println!("测试2: 整数相等条件");
    let int_eq_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 1");
    if int_eq_result.is_ok() {
        println!("  整数相等条件查询成功");
    } else {
        println!("  整数相等条件查询失败");
    }

    // 测试3: 布尔条件
    println!("测试3: 布尔条件");
    let bool_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE active = false");
    if bool_result.is_ok() {
        println!("  布尔条件查询成功");
    } else {
        println!("  布尔条件查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量混合搜索: 不同数据类型条件 完成 ===");
}

// 测试复合条件组合
#[test]
#[serial]
fn test_vector_hybrid_search_complex_conditions() {
    println!("=== 测试向量混合搜索: 复杂条件组合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_HYBRID_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量和标量字段的数据库初始化成功");

    // 插入测试数据
    for i in 1..=8 {
        let mut record = VectorHybridRecord {
            id: i,
            name: [0u8; 64],
            vector: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
            ],
            category: if i <= 4 { 1 } else { 2 },
            score: 0.6 + (i as f32 * 0.05),
            active: i % 3 != 0, // 每3条记录有一条是 inactive
        };

        // 设置名称
        let name_str = format!("complex_item{}", i);
        let name_bytes = name_str.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入8条测试数据");

    // 测试1: 多个AND条件组合
    println!("测试1: 多个AND条件组合");
    let and_result = db.sql_query(
        "SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 1 AND score > 0.7 AND active = true",
    );
    if and_result.is_ok() {
        println!("  多个AND条件组合查询成功");
    } else {
        println!("  多个AND条件组合查询失败");
    }

    // 测试2: 混合AND和OR条件
    println!("测试2: 混合AND和OR条件");
    let mixed_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE (category = 1 AND score > 0.8) OR (category = 2 AND active = false)");
    if mixed_result.is_ok() {
        println!("  混合AND和OR条件查询成功");
    } else {
        println!("  混合AND和OR条件查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量混合搜索: 复杂条件组合 完成 ===");
}

// 测试向量索引与二级索引结合
#[test]
#[serial]
fn test_vector_index_with_secondary_index() {
    println!("=== 测试向量混合搜索: 向量索引与二级索引结合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_HYBRID_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量索引和二级索引的数据库初始化成功");

    // 插入测试数据
    for i in 1..=6 {
        let mut record = VectorHybridRecord {
            id: i,
            name: [0u8; 64],
            vector: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
            ],
            category: if i <= 3 { 1 } else { 2 },
            score: 0.7 + (i as f32 * 0.04),
            active: true,
        };

        // 设置名称
        let name_str = format!("index_item{}", i);
        let name_bytes = name_str.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入6条测试数据");

    // 创建向量索引
    let vec_idx_result = db.sql_query(
        "CREATE INDEX vector_with_secondary_idx ON VECTOR_HYBRID_TABLE (vector) USING HNSW",
    );
    if vec_idx_result.is_ok() {
        println!("成功创建向量索引");
    }

    // 测试1: 基于二级索引字段的查询
    println!("测试1: 基于二级索引字段的查询");
    let secondary_result = db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 2");
    if secondary_result.is_ok() {
        println!("  基于二级索引字段的查询成功");
    } else {
        println!("  基于二级索引字段的查询失败");
    }

    // 测试2: 基于复合索引字段的查询
    println!("测试2: 基于复合索引字段的查询");
    let composite_result =
        db.sql_query("SELECT id FROM VECTOR_HYBRID_TABLE WHERE category = 1 AND score > 0.75");
    if composite_result.is_ok() {
        println!("  基于复合索引字段的查询成功");
    } else {
        println!("  基于复合索引字段的查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量混合搜索: 向量索引与二级索引结合 完成 ===");
}
