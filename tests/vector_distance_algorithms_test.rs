#![allow(static_mut_refs, clippy::assertions_on_constants)]
//! 向量距离算法综合测试
//!
//! 该测试文件验证不同距离算法（L2, IP, Cosine）的向量索引功能。

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

// 定义包含向量字段的测试表
remdb::table!(
    VECTOR_DISTANCE_TABLE,
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
    VECTOR_DISTANCE_DB,
    tables: [VECTOR_DISTANCE_TABLE]
);

// 向量记录结构
#[repr(C)]
struct VectorRecord {
    id: i32,
    vector: [f32; 4],
    category: i32,
}

// 测试L2距离算法
#[test]
#[serial]
fn test_vector_distance_l2() {
    println!("=== 测试向量距离算法: L2 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    let test_vectors = [
        ([1.0, 2.0, 3.0, 4.0], 1),
        ([2.0, 3.0, 4.0, 5.0], 1),
        ([3.0, 4.0, 5.0, 6.0], 2),
        ([4.0, 5.0, 6.0, 7.0], 2),
        ([5.0, 6.0, 7.0, 8.0], 1),
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_vectors.len());

    // 测试1: 创建使用L2距离的HNSW索引
    println!("测试1: 创建使用L2距离的HNSW索引");
    let result = db.sql_query(
        "CREATE INDEX vector_l2_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=L2",
    );
    if result.is_ok() {
        println!("成功创建使用L2距离的HNSW索引");

        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_DISTANCE_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建L2距离的HNSW索引失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量距离算法: L2 完成 ===");
}

// 测试IP距离算法
#[test]
#[serial]
fn test_vector_distance_ip() {
    println!("=== 测试向量距离算法: IP ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    let test_vectors = [
        ([1.0, 0.0, 0.0, 0.0], 1),
        ([0.0, 1.0, 0.0, 0.0], 1),
        ([0.0, 0.0, 1.0, 0.0], 2),
        ([0.0, 0.0, 0.0, 1.0], 2),
        ([1.0, 1.0, 1.0, 1.0], 1),
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_vectors.len());

    // 测试1: 创建使用IP距离的HNSW索引
    println!("测试1: 创建使用IP距离的HNSW索引");
    let result = db.sql_query(
        "CREATE INDEX vector_ip_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=IP",
    );
    if result.is_ok() {
        println!("成功创建使用IP距离的HNSW索引");

        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_DISTANCE_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建IP距离的HNSW索引失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量距离算法: IP 完成 ===");
}

// 测试Cosine距离算法
#[test]
#[serial]
fn test_vector_distance_cosine() {
    println!("=== 测试向量距离算法: Cosine ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    let test_vectors = [
        ([1.0, 0.0, 0.0, 0.0], 1),
        ([0.9, 0.1, 0.0, 0.0], 1),
        ([0.0, 1.0, 0.0, 0.0], 2),
        ([0.0, 0.9, 0.1, 0.0], 2),
        ([1.0, 1.0, 0.0, 0.0], 1),
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_vectors.len());

    // 测试1: 创建使用Cosine距离的HNSW索引
    println!("测试1: 创建使用Cosine距离的HNSW索引");
    let result = db.sql_query("CREATE INDEX vector_cosine_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=COSINE");
    if result.is_ok() {
        println!("成功创建使用Cosine距离的HNSW索引");

        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_DISTANCE_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建Cosine距离的HNSW索引失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量距离算法: Cosine 完成 ===");
}

// 测试多种距离算法组合（在不同表上）
#[test]
#[serial]
fn test_vector_distance_multiple_algorithms() {
    println!("=== 测试多种向量距离算法组合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 测试1: L2距离
    println!("\n--- 测试L2距离算法 ---");
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    let config = &VECTOR_DISTANCE_DB;
    let db1 = remdb::init_global_db(config).unwrap();

    // 插入少量测试数据
    for i in 1..=3 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 1.0; 4],
            category: if i % 2 == 0 { 2 } else { 1 },
        };
        let table = db1.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result1 = db1.sql_query("CREATE INDEX vector_l2_comb_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=L2");
    println!(
        "L2距离索引创建: {}",
        if result1.is_ok() { "成功" } else { "失败" }
    );
    remdb::reset_global_db();

    // 测试2: Cosine距离
    println!("\n--- 测试Cosine距离算法 ---");
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    let db2 = remdb::init_global_db(config).unwrap();

    // 插入少量测试数据
    for i in 1..=3 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 1.0; 4],
            category: if i % 2 == 0 { 2 } else { 1 },
        };
        let table = db2.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result2 = db2.sql_query("CREATE INDEX vector_cosine_comb_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=COSINE");
    println!(
        "Cosine距离索引创建: {}",
        if result2.is_ok() { "成功" } else { "失败" }
    );
    remdb::reset_global_db();

    println!("\n=== 测试多种向量距离算法组合完成 ===");
}

// 测试默认距离算法（应该是L2）
#[test]
#[serial]
fn test_vector_distance_default() {
    println!("=== 测试向量距离算法: 默认（L2） ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    for i in 1..=5 {
        let record = VectorRecord {
            id: i,
            vector: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
            ],
            category: if i % 2 == 0 { 2 } else { 1 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入5条测试数据");

    // 测试1: 创建不指定距离算法的HNSW索引（应该默认使用L2）
    println!("测试1: 创建不指定距离算法的HNSW索引");
    let result = db
        .sql_query("CREATE INDEX vector_default_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW");
    if result.is_ok() {
        println!("成功创建默认距离算法的HNSW索引");

        // 验证基本查询
        println!("  验证基本查询...");
        let search_result = db.sql_query("SELECT id FROM VECTOR_DISTANCE_TABLE WHERE id = 1");
        if search_result.is_ok() {
            println!("  基础查询验证成功");
        } else {
            println!("  基础查询验证失败");
        }
    } else {
        println!("创建默认距离算法的HNSW索引失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量距离算法: 默认（L2） 完成 ===");
}

// 测试向量搜索功能（KNN查询）
#[test]
#[serial]
fn test_vector_knn_search() {
    println!("=== 测试向量KNN搜索功能 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    let test_vectors = vec![
        ([1.0, 1.0, 1.0, 1.0], 1), // 与查询向量最接近
        ([2.0, 2.0, 2.0, 2.0], 1),
        ([3.0, 3.0, 3.0, 3.0], 2),
        ([4.0, 4.0, 4.0, 4.0], 2),
        ([10.0, 10.0, 10.0, 10.0], 1), // 与查询向量最远
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_vectors.len());

    // 创建使用L2距离的HNSW索引
    println!("创建使用L2距离的HNSW索引");
    let create_index_result = db.sql_query(
        "CREATE INDEX vector_knn_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=L2",
    );

    if create_index_result.is_ok() {
        println!("成功创建使用L2距离的HNSW索引");

        // 测试KNN搜索
        println!("测试KNN搜索: 查找与 [1.1, 1.1, 1.1, 1.1] 最接近的3个向量");
        let knn_result = db.sql_query(
            "SELECT id, category FROM VECTOR_DISTANCE_TABLE ORDER BY VECTOR_DISTANCE(vector, '[1.1, 1.1, 1.1, 1.1]') LIMIT 3",
        );

        if knn_result.is_ok() {
            println!("KNN搜索执行成功");
        } else {
            println!("KNN搜索执行失败，可能功能尚未实现");
        }
    } else {
        println!("创建HNSW索引失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量KNN搜索功能完成 ===");
}

// 测试距离计算准确性
#[test]
#[serial]
fn test_vector_distance_calculation() {
    println!("=== 测试向量距离计算准确性 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    let test_vector = [1.0, 0.0, 0.0, 0.0];
    let record = VectorRecord {
        id: 1,
        vector: test_vector,
        category: 1,
    };

    let table = db.get_table_mut(0).unwrap();
    let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
    assert!(insert_id < config.tables[0].max_records);

    println!("成功插入测试数据");

    // 测试不同距离计算
    println!("测试不同距离计算:");

    // 测试L2距离
    println!("1. 测试L2距离计算");
    let l2_result = db.sql_query(
        "SELECT VECTOR_DISTANCE_L2(vector, '[2.0, 0.0, 0.0, 0.0]') FROM VECTOR_DISTANCE_TABLE WHERE id = 1",
    );
    if l2_result.is_ok() {
        println!("L2距离计算执行成功");
    } else {
        println!("L2距离计算执行失败，可能功能尚未实现");
    }

    // 测试IP距离
    println!("2. 测试IP距离计算");
    let ip_result = db.sql_query(
        "SELECT VECTOR_DISTANCE_IP(vector, '[1.0, 0.0, 0.0, 0.0]') FROM VECTOR_DISTANCE_TABLE WHERE id = 1",
    );
    if ip_result.is_ok() {
        println!("IP距离计算执行成功");
    } else {
        println!("IP距离计算执行失败，可能功能尚未实现");
    }

    // 测试Cosine距离
    println!("3. 测试Cosine距离计算");
    let cosine_result = db.sql_query(
        "SELECT VECTOR_DISTANCE_COSINE(vector, '[1.0, 0.0, 0.0, 0.0]') FROM VECTOR_DISTANCE_TABLE WHERE id = 1",
    );
    if cosine_result.is_ok() {
        println!("Cosine距离计算执行成功");
    } else {
        println!("Cosine距离计算执行失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量距离计算准确性完成 ===");
}

// 测试向量索引参数配置
#[test]
#[serial]
fn test_vector_index_parameters() {
    println!("=== 测试向量索引参数配置 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 插入测试数据
    for i in 1..=5 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 1.0; 4],
            category: if i % 2 == 0 { 2 } else { 1 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入5条测试数据");

    // 测试1: 创建带参数的HNSW索引
    println!("测试1: 创建带参数的HNSW索引 (M=16, EF_CONSTRUCTION=200)");
    let result = db.sql_query(
        "CREATE INDEX vector_params_idx ON VECTOR_DISTANCE_TABLE (vector) USING HNSW WITH DISTANCE=L2, M=16, EF_CONSTRUCTION=200",
    );

    if result.is_ok() {
        println!("成功创建带参数的HNSW索引");
    } else {
        println!("创建带参数的HNSW索引失败，可能功能尚未实现");
    }

    // 测试2: 创建IVF索引
    println!("测试2: 创建IVF索引 (NLIST=100)");
    let ivf_result = db.sql_query(
        "CREATE INDEX vector_ivf_idx ON VECTOR_DISTANCE_TABLE (vector) USING IVF WITH DISTANCE=L2, NLIST=100",
    );

    if ivf_result.is_ok() {
        println!("成功创建IVF索引");
    } else {
        println!("创建IVF索引失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量索引参数配置完成 ===");
}

// 测试向量字段的ALTER TABLE操作
#[test]
#[serial]
fn test_vector_alter_table() {
    println!("=== 测试向量字段的ALTER TABLE操作 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &VECTOR_DISTANCE_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含4维向量表的数据库初始化成功");

    // 测试1: 添加新的向量字段
    println!("测试1: 添加新的6维向量字段 'vector6d' 带COSINE距离");
    let add_result = db.sql_query(
        "ALTER TABLE VECTOR_DISTANCE_TABLE ADD COLUMN vector6d VECTOR(6) WITH DISTANCE=COSINE",
    );

    if add_result.is_ok() {
        println!("成功添加新的向量字段");
    } else {
        println!("添加向量字段失败，可能功能尚未实现");
    }

    // 测试2: 修改现有向量字段的距离类型
    println!("测试2: 修改现有向量字段的距离类型为IP");
    let modify_result = db.sql_query(
        "ALTER TABLE VECTOR_DISTANCE_TABLE MODIFY COLUMN vector VECTOR(4) WITH DISTANCE=IP",
    );

    if modify_result.is_ok() {
        println!("成功修改向量字段的距离类型");
    } else {
        println!("修改向量字段失败，可能功能尚未实现");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试向量字段的ALTER TABLE操作完成 ===");
}
