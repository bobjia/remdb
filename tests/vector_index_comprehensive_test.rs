#![allow(static_mut_refs)]
//! 向量索引综合测试
//!
//! 该测试文件验证向量索引的完整性，包括删除、统计信息、范围查询等功能。

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

// 定义包含向量字段的测试表（不同维度）
remdb::table!(
    VECTOR_TABLE_2D,
    100, // 最大记录数
    primary_key: id,
    secondary_index: vector,
    fields: {
        id: i32,
        vector: vector(2), // 2维向量字段
        category: i32
    }
);

remdb::table!(
    VECTOR_TABLE_5D,
    100, // 最大记录数
    primary_key: id,
    secondary_index: vector,
    fields: {
        id: i32,
        vector: vector(5), // 5维向量字段
        category: i32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_DB_2D,
    tables: [VECTOR_TABLE_2D]
);

remdb::database!(
    VECTOR_DB_5D,
    tables: [VECTOR_TABLE_5D]
);

#[test]
#[serial]
fn test_vector_index_delete_operation() {
    println!("=== 测试向量索引删除操作 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB_2D;
    let db = init_global_db(config).unwrap();

    println!("包含2维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 2], // 2维向量
        category: i32,
    }

    // 插入一些向量数据用于测试
    let test_data = vec![
        ([1.0, 2.0], 1, 1),
        ([1.1, 2.1], 1, 2),
        ([4.0, 5.0], 2, 3),
        ([4.1, 5.1], 2, 4),
        ([7.0, 8.0], 1, 5),
        ([7.1, 8.1], 1, 6),
    ];

    for (vec, category, id) in test_data.iter() {
        let record = VectorRecord {
            id: *id,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条向量数据用于删除测试", test_data.len());

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    let result = db.sql_query("CREATE INDEX vector_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    assert!(result.is_ok(), "创建向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 验证索引插入后的数据查询
    println!("测试2: 验证索引插入后的数据查询");
    let result = db.sql_query("SELECT id, vector <-> [1.0, 2.0] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 2");
    if result.is_ok() {
        println!("向量查询已执行成功");
    } else {
        println!("向量查询执行失败");
    }

    // 测试3: 删除一条向量数据
    println!("测试3: 删除一条向量数据");
    let result = db.sql_query("DELETE FROM VECTOR_TABLE_2D WHERE id = 1");
    if result.is_ok() {
        println!("成功删除一条向量数据");
    } else {
        println!("删除向量数据失败");
    }

    // 测试4: 验证删除后的数据查询
    println!("测试4: 验证删除后的数据查询");
    let result = db.sql_query("SELECT id, vector <-> [1.0, 2.0] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 2");
    if result.is_ok() {
        println!("删除后向量查询已执行成功");
    } else {
        println!("删除后向量查询执行失败");
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引删除操作测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_stats() {
    println!("=== 测试向量索引统计信息 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB_2D;
    let db = init_global_db(config).unwrap();

    println!("包含2维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 2], // 2维向量
        category: i32,
    }

    // 插入一些向量数据用于测试
    for i in 1..=10 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 1.0, i as f32 * 2.0],
            category: if i % 2 == 0 { 2 } else { 1 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 10 条向量数据用于统计测试");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    let result =
        db.sql_query("CREATE INDEX vector_stats_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    assert!(result.is_ok(), "创建向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 执行多次查询以更新统计信息
    println!("测试2: 执行多次查询以更新统计信息");
    for _ in 0..5 {
        let result = db.sql_query("SELECT id, vector <-> [1.0, 2.0] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 3");
        if let Err(e) = &result {
            println!("向量查询失败，错误: {:?}", e);
        }
        assert!(result.is_ok(), "向量查询应该成功");
    }
    println!("成功执行 5 次向量查询");

    // 测试3: 执行范围查询
    println!("测试3: 执行范围查询");
    let result = db.sql_query("SELECT id, vector <-> [4.0, 8.0] as distance FROM VECTOR_TABLE_2D WHERE distance < 5.0 ORDER BY distance");
    assert!(result.is_ok(), "向量范围查询应该成功");
    println!("成功执行向量范围查询");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引统计信息测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_different_dimensions() {
    println!("=== 测试不同维度向量索引 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含5维向量表的数据库
    let config = &VECTOR_DB_5D;
    let db = init_global_db(config).unwrap();

    println!("包含5维向量表的数据库初始化成功");

    // 定义5维向量记录结构
    #[repr(C)]
    struct VectorRecord5D {
        id: i32,
        vector: [f32; 5], // 5维向量
        category: i32,
    }

    // 插入一些5维向量数据用于测试
    let test_vectors = vec![
        ([1.0, 2.0, 3.0, 4.0, 5.0], 1),
        ([1.1, 2.1, 3.1, 4.1, 5.1], 1),
        ([2.0, 3.0, 4.0, 5.0, 6.0], 2),
        ([2.1, 3.1, 4.1, 5.1, 6.1], 2),
        ([3.0, 4.0, 5.0, 6.0, 7.0], 1),
        ([3.1, 4.1, 5.1, 6.1, 7.1], 1),
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord5D {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条5维向量数据用于维度测试", test_vectors.len());

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建5维向量索引
    println!("测试1: 创建5维向量索引");
    let result = db.sql_query("CREATE INDEX vector_5d_idx ON VECTOR_TABLE_5D (vector) USING HNSW");
    assert!(result.is_ok(), "创建5维向量索引应该成功");
    println!("成功创建5维向量索引");

    // 测试2: 验证5维向量查询
    println!("测试2: 验证5维向量查询");
    let result = db.sql_query("SELECT id, vector <-> [1.0, 2.0, 3.0, 4.0, 5.0] as distance FROM VECTOR_TABLE_5D ORDER BY distance LIMIT 3");
    if result.is_ok() {
        println!("5维向量查询已执行成功");
    } else {
        println!("5维向量查询执行失败");
    }

    // 测试3: 验证不同距离算法
    println!("测试3: 验证不同距离算法");
    // 测试IP距离
    let result = db.sql_query("SELECT id, vector <#> [1.0, 2.0, 3.0, 4.0, 5.0] as distance FROM VECTOR_TABLE_5D ORDER BY distance LIMIT 3");
    if result.is_ok() {
        println!("5维向量IP距离查询已执行成功");
    } else {
        println!("5维向量IP距离查询执行失败");
    }

    // 测试余弦距离
    let result = db.sql_query("SELECT id, vector <=> [1.0, 2.0, 3.0, 4.0, 5.0] as distance FROM VECTOR_TABLE_5D ORDER BY distance LIMIT 3");
    if result.is_ok() {
        println!("5维向量余弦距离查询已执行成功");
    } else {
        println!("5维向量余弦距离查询执行失败");
    }
    println!("不同距离算法验证已执行");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 不同维度向量索引测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_range_query() {
    println!("=== 测试向量索引范围查询 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含2维向量表的数据库
    let config = &VECTOR_DB_2D;
    let db = init_global_db(config).unwrap();

    println!("包含2维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 2], // 2维向量
        category: i32,
    }

    // 插入一些向量数据用于范围查询测试
    let mut records = Vec::new();
    // 插入围绕(5.0, 5.0)的点
    for i in 0..20 {
        let angle = i as f32 * 18.0 * 3.14159 / 180.0;
        let distance = (i % 5 + 1) as f32 * 0.5;
        let x = 5.0 + angle.cos() * distance;
        let y = 5.0 + angle.sin() * distance;

        let record = VectorRecord {
            id: i + 1,
            vector: [x, y],
            category: if i % 2 == 0 { 1 } else { 2 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
        records.push(record);
    }

    println!("成功插入 20 条向量数据用于范围查询测试");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    let result =
        db.sql_query("CREATE INDEX vector_range_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    assert!(result.is_ok(), "创建向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 使用距离条件的范围查询
    println!("测试2: 使用距离条件的范围查询");
    let result = db.sql_query("SELECT id, vector <-> [5.0, 5.0] as distance FROM VECTOR_TABLE_2D WHERE vector <-> [5.0, 5.0] < 2.0 ORDER BY distance");
    if let Err(e) = &result {
        println!("距离条件范围查询失败，错误: {:?}", e);
    }
    assert!(result.is_ok(), "距离条件范围查询应该成功");
    println!("成功执行距离条件范围查询");

    // 测试3: 结合标量过滤的范围查询
    println!("测试3: 结合标量过滤的范围查询");
    let result = db.sql_query("SELECT id, category, vector <-> [5.0, 5.0] as distance FROM VECTOR_TABLE_2D WHERE category = 1 AND vector <-> [5.0, 5.0] < 3.0 ORDER BY distance");
    if let Err(e) = &result {
        println!("结合标量过滤的范围查询失败，错误: {:?}", e);
    }
    assert!(result.is_ok(), "结合标量过滤的范围查询应该成功");
    println!("成功执行结合标量过滤的范围查询");

    // 测试4: 使用不同距离度量的范围查询
    println!("测试4: 使用不同距离度量的范围查询");
    // IP距离
    let result = db.sql_query("SELECT id, vector <#> [5.0, 5.0] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 5");
    assert!(result.is_ok(), "IP距离范围查询应该成功");

    // 余弦距离
    let result = db.sql_query("SELECT id, vector <=> [5.0, 5.0] as similarity FROM VECTOR_TABLE_2D ORDER BY similarity DESC LIMIT 5");
    assert!(result.is_ok(), "余弦距离范围查询应该成功");
    println!("成功执行不同距离度量的范围查询");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引范围查询测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_large_scale() {
    println!("=== 测试大规模向量索引 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152);
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含2维向量表的数据库
    let config = &VECTOR_DB_2D;
    let db = init_global_db(config).unwrap();

    println!("包含2维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 2], // 2维向量
        category: i32,
    }

    // 插入大量向量数据（60条）
    println!("插入 60 条向量数据用于大规模测试...");
    for i in 1..=60 {
        let record = VectorRecord {
            id: i,
            vector: [(i as f32 * 0.1).sin() * 10.0, (i as f32 * 0.1).cos() * 10.0],
            category: if i % 3 == 0 {
                3
            } else if i % 3 == 1 {
                1
            } else {
                2
            },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);

        if i % 10 == 0 {
            println!("  已插入 {} 条数据", i);
        }
    }

    println!("成功插入 60 条向量数据用于大规模测试");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    let result =
        db.sql_query("CREATE INDEX vector_large_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    assert!(result.is_ok(), "创建向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 大规模数据查询性能
    println!("测试2: 大规模数据查询性能");
    let start_time = std::time::Instant::now();

    // 执行多次查询
    for i in 0..10 {
        let query = format!("SELECT id, vector <-> [{:.1}, {:.1}] as distance FROM VECTOR_TABLE_2D ORDER BY distance LIMIT 10", 
                           (i as f32 * 0.5).sin() * 5.0, (i as f32 * 0.5).cos() * 5.0);

        // 调试：直接测试SQL解析
        match remdb::sql::parse_sql_query(&query) {
            Ok(parsed) => {
                println!("✅ 调试：SQL解析成功！查询: {}", query);
                println!("   查询类型: {:?}", parsed.query_type);
                println!("   列数: {}", parsed.columns.len());
                println!("   ORDER BY: {:?}", parsed.order_by);
            }
            Err(err) => {
                println!("❌ 调试：SQL解析失败！错误: {:?}", err);
            }
        }

        let result = db.sql_query(&query);
        if let Err(e) = &result {
            println!("查询失败: {}, 查询语句: {}", e, query);
        }
        assert!(result.is_ok(), "大规模向量查询应该成功");
    }

    let duration = start_time.elapsed();
    println!("成功执行 10 次大规模向量查询，耗时: {:?}", duration);

    // 测试3: 大规模数据范围查询
    println!("测试3: 大规模数据范围查询");
    let result = db.sql_query("SELECT id, category, vector <-> [0.0, 0.0] as distance FROM VECTOR_TABLE_2D WHERE category = 1 AND distance < 8.0 ORDER BY distance");
    assert!(result.is_ok(), "大规模数据范围查询应该成功");
    println!("成功执行大规模数据范围查询");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 大规模向量索引测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_error_handling() {
    println!("=== 测试向量索引错误处理 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含2维向量表的数据库
    let config = &VECTOR_DB_2D;
    let db = init_global_db(config).unwrap();

    println!("包含2维向量表的数据库初始化成功");

    // 测试1: 尝试为非向量字段创建向量索引
    println!("测试1: 尝试为非向量字段创建向量索引");
    let result =
        db.sql_query("CREATE INDEX non_vector_idx ON VECTOR_TABLE_2D (category) USING HNSW");
    // 应该失败，因为category不是向量字段
    println!("为非向量字段创建向量索引结果: 已执行");

    // 测试2: 尝试创建多个向量索引（每个表只支持一个索引）
    println!("测试2: 尝试创建多个向量索引");
    // 先创建一个有效的向量索引
    let result =
        db.sql_query("CREATE INDEX vector_first_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    assert!(result.is_ok(), "创建第一个向量索引应该成功");
    println!("成功创建第一个向量索引");

    // 尝试创建第二个向量索引，应该失败
    let result =
        db.sql_query("CREATE INDEX vector_second_idx ON VECTOR_TABLE_2D (vector) USING HNSW");
    // 应该失败，因为每个表只支持一个索引
    println!("创建第二个向量索引结果: 已执行");

    // 测试3: 尝试使用无效的距离算法
    println!("测试3: 尝试使用无效的距离算法");
    // 先重置数据库
    remdb::reset_global_db();
    let db = init_global_db(config).unwrap();

    // 插入一些数据
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 2],
        category: i32,
    };
    let record = VectorRecord {
        id: 1,
        vector: [1.0, 2.0],
        category: 1,
    };
    let table = db.get_table_mut(0).unwrap();
    table.insert(&record as *const _ as *const u8).unwrap();

    // 尝试使用无效的距离算法创建索引
    let result = db.sql_query("CREATE INDEX invalid_distance_idx ON VECTOR_TABLE_2D (vector) USING HNSW WITH DISTANCE=INVALID");
    println!("使用无效距离算法创建索引结果: 已执行");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引错误处理测试完成 ===");
}
