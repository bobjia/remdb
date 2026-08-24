#![allow(static_mut_refs, clippy::assertions_on_constants)]
//! IVF变体索引性能测试
//!
//! 该测试文件验证不同IVF变体索引的性能，包括IVF和IVF_PQ等。

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
    IVF_VARIANTS_TABLE,
    200, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        vector: vector(6), // 6维向量字段，适合IVF变体测试
        category: i32,
        value: f32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    IVF_VARIANTS_DB,
    tables: [IVF_VARIANTS_TABLE]
);

#[test]
#[serial]
fn test_ivf_variants_basic_functionality() {
    println!("=== 测试 IVF 变体索引基本功能 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct IVFRecord {
        id: i32,
        vector: [f32; 6],
        category: i32,
        value: f32,
    };

    // 测试不同的IVF变体
    let ivf_variants = vec!["IVF", "IVF_PQ"];

    for ivf_type in ivf_variants.iter() {
        println!("\n--- 测试 {} 索引 ---", ivf_type);

        // 重置全局数据库实例，确保测试之间的隔离
        remdb::reset_global_db();

        // 初始化包含向量表的数据库
        let config = &IVF_VARIANTS_DB;
        let db = init_global_db(config).unwrap();

        // 插入测试数据
        for i in 1..=50 {
            let record = IVFRecord {
                id: i,
                vector: [
                    i as f32 * 0.5,
                    (i as f32 * 0.3).sin() * 10.0,
                    (i as f32 * 0.3).cos() * 10.0,
                    i as f32 * 0.7,
                    (i as f32 * 0.4).sin() * 8.0,
                    (i as f32 * 0.4).cos() * 8.0,
                ],
                category: (i % 4) + 1,
                value: i as f32 * 0.25,
            };

            let table = db.get_table_mut(0).unwrap();
            table.insert(&record as *const _ as *const u8).unwrap();
        }

        println!("成功插入 50 条向量数据用于 {} 测试", ivf_type);

        // 初始化索引构建线程池
        crate::index::builder::init_index_build_thread_pool(2);
        println!("索引构建线程池初始化成功");

        // 测试1: 创建IVF变体索引
        println!("测试1: 创建 {} 索引", ivf_type);
        let create_query = format!(
            "CREATE INDEX ivf_{}_idx ON IVF_VARIANTS_TABLE (vector) USING {}",
            ivf_type.to_lowercase(),
            ivf_type
        );
        let result = db.sql_query(&create_query);
        assert!(result.is_ok(), "创建 {} 索引应该成功", ivf_type);
        println!("成功创建 {} 索引", ivf_type);

        // 测试2: 验证索引查询功能
        println!("测试2: 验证 {} 索引查询功能", ivf_type);
        let search_query = "SELECT id, vector <-> [5.0, 0.0, 10.0, 7.0, 0.0, 8.0] as distance FROM IVF_VARIANTS_TABLE ORDER BY distance LIMIT 5";
        let result = db.sql_query(search_query);
        assert!(result.is_ok(), "{} 索引查询应该成功", ivf_type);
        println!("{} 索引查询成功", ivf_type);

        // 测试3: 验证混合查询功能
        println!("测试3: 验证 {} 索引混合查询功能", ivf_type);
        let hybrid_query = "SELECT id, category, vector <-> [5.0, 0.0, 10.0, 7.0, 0.0, 8.0] as distance FROM IVF_VARIANTS_TABLE WHERE category = 2 ORDER BY distance LIMIT 3";
        let result = db.sql_query(hybrid_query);
        assert!(result.is_ok(), "{} 索引混合查询应该成功", ivf_type);
        println!("{} 索引混合查询成功", ivf_type);
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== IVF 变体索引基本功能测试完成 ===");
}

#[test]
#[serial]
fn test_ivf_variants_with_different_data_sizes() {
    println!("=== 测试 IVF 变体索引在不同数据规模下的性能 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct IVFRecord {
        id: i32,
        vector: [f32; 6],
        category: i32,
        value: f32,
    };

    // 测试不同数据规模
    let data_sizes = vec![20, 50, 100];

    for data_size in data_sizes.iter() {
        println!("\n--- 测试数据规模: {} 条记录 ---", data_size);

        // 测试IVF和IVF_PQ
        for ivf_type in ["IVF", "IVF_PQ"].iter() {
            println!("\n测试 {} 索引...", ivf_type);

            // 重置全局数据库实例，确保测试之间的隔离
            remdb::reset_global_db();

            // 初始化包含向量表的数据库
            let config = &IVF_VARIANTS_DB;
            let db = init_global_db(config).unwrap();

            // 插入测试数据
            for i in 1..=*data_size {
                let record = IVFRecord {
                    id: i,
                    vector: [
                        (i as f32 * 0.2).sin() * 20.0,
                        (i as f32 * 0.2).cos() * 20.0,
                        (i as f32 * 0.2 + 1.0).sin() * 15.0,
                        (i as f32 * 0.2 + 1.0).cos() * 15.0,
                        i as f32 * 0.1,
                        i as f32 * 0.2,
                    ],
                    category: (i % 5) + 1,
                    value: i as f32 * 0.5,
                };

                let table = db.get_table_mut(0).unwrap();
                table.insert(&record as *const _ as *const u8).unwrap();
            }

            println!("成功插入 {} 条向量数据", data_size);

            // 初始化索引构建线程池
            crate::index::builder::init_index_build_thread_pool(2);
            println!("索引构建线程池初始化成功");

            // 创建索引
            let create_query = format!(
                "CREATE INDEX ivf_{}_size_idx ON IVF_VARIANTS_TABLE (vector) USING {}",
                ivf_type.to_lowercase(),
                ivf_type
            );
            let result = db.sql_query(&create_query);
            assert!(result.is_ok(), "创建 {} 索引应该成功", ivf_type);
            println!("成功创建 {} 索引", ivf_type);

            // 执行查询
            let search_query = "SELECT id, vector <-> [10.0, 0.0, 7.5, 0.0, 5.0, 10.0] as distance FROM IVF_VARIANTS_TABLE ORDER BY distance LIMIT 5";
            let result = db.sql_query(search_query);
            assert!(result.is_ok(), "{} 索引查询应该成功", ivf_type);
            println!("{} 索引查询成功", ivf_type);

            // 执行范围查询
            let range_query = "SELECT id, vector <-> [10.0, 0.0, 7.5, 0.0, 5.0, 10.0] as distance FROM IVF_VARIANTS_TABLE WHERE distance < 20.0 ORDER BY distance";
            let result = db.sql_query(range_query);
            assert!(result.is_ok(), "{} 索引范围查询应该成功", ivf_type);
            println!("{} 索引范围查询成功", ivf_type);
        }
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== IVF 变体索引在不同数据规模下的性能测试完成 ===");
}

#[test]
#[serial]
fn test_ivf_variants_comparison() {
    println!("=== 测试 IVF 变体索引比较 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct IVFRecord {
        id: i32,
        vector: [f32; 6],
        category: i32,
        value: f32,
    };

    // 准备测试数据
    let test_vectors = vec![
        // 集群1
        ([1.0, 1.0, 1.0, 1.0, 1.0, 1.0], 1),
        ([1.1, 1.1, 1.1, 1.1, 1.1, 1.1], 1),
        ([0.9, 0.9, 0.9, 0.9, 0.9, 0.9], 1),
        ([1.2, 1.2, 1.2, 1.2, 1.2, 1.2], 1),
        // 集群2
        ([10.0, 10.0, 10.0, 10.0, 10.0, 10.0], 2),
        ([10.1, 10.1, 10.1, 10.1, 10.1, 10.1], 2),
        ([9.9, 9.9, 9.9, 9.9, 9.9, 9.9], 2),
        ([10.2, 10.2, 10.2, 10.2, 10.2, 10.2], 2),
        // 集群3
        ([20.0, 20.0, 20.0, 20.0, 20.0, 20.0], 3),
        ([20.1, 20.1, 20.1, 20.1, 20.1, 20.1], 3),
        ([19.9, 19.9, 19.9, 19.9, 19.9, 19.9], 3),
        ([20.2, 20.2, 20.2, 20.2, 20.2, 20.2], 3),
        // 离群点
        ([50.0, 50.0, 50.0, 50.0, 50.0, 50.0], 4),
        ([100.0, 100.0, 100.0, 100.0, 100.0, 100.0], 4),
    ];

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &IVF_VARIANTS_DB;
    let db = init_global_db(config).unwrap();

    // 插入测试数据
    for (i, (vector, category)) in test_vectors.iter().enumerate() {
        let record = IVFRecord {
            id: (i + 1) as i32,
            vector: *vector,
            category: *category,
            value: (i + 1) as f32 * 1.0,
        };

        let table = db.get_table_mut(0).unwrap();
        table.insert(&record as *const _ as *const u8).unwrap();
    }

    println!("成功插入集群测试数据");

    // 测试查询不同集群的中心点
    let query_centers = vec![
        ([1.0, 1.0, 1.0, 1.0, 1.0, 1.0], "集群1"),
        ([10.0, 10.0, 10.0, 10.0, 10.0, 10.0], "集群2"),
        ([20.0, 20.0, 20.0, 20.0, 20.0, 20.0], "集群3"),
    ];

    // 测试IVF和IVF_PQ
    for ivf_type in ["IVF", "IVF_PQ"].iter() {
        println!("\n=== 测试 {} 索引集群查询 ===", ivf_type);

        // 重置数据库
        remdb::reset_global_db();
        let db = init_global_db(config).unwrap();

        // 重新插入数据
        for (i, (vector, category)) in test_vectors.iter().enumerate() {
            let record = IVFRecord {
                id: (i + 1) as i32,
                vector: *vector,
                category: *category,
                value: (i + 1) as f32 * 1.0,
            };

            let table = db.get_table_mut(0).unwrap();
            table.insert(&record as *const _ as *const u8).unwrap();
        }

        // 初始化索引构建线程池
        crate::index::builder::init_index_build_thread_pool(2);
        println!("索引构建线程池初始化成功");

        // 创建索引
        let create_query = format!(
            "CREATE INDEX ivf_{}_cluster_idx ON IVF_VARIANTS_TABLE (vector) USING {}",
            ivf_type.to_lowercase(),
            ivf_type
        );
        db.sql_query(&create_query).unwrap();

        // 查询每个集群
        for (center, cluster_name) in query_centers.iter() {
            println!("\n查询 {} 中心: {:?}", cluster_name, center);

            let query = format!("SELECT id, vector <-> [{:.1}, {:.1}, {:.1}, {:.1}, {:.1}, {:.1}] as distance FROM IVF_VARIANTS_TABLE ORDER BY distance LIMIT 3", 
                               center[0], center[1], center[2], center[3], center[4], center[5]);

            let result = db.sql_query(&query);
            assert!(result.is_ok(), "{} 集群查询应该成功", cluster_name);
            println!("{} 集群查询成功", cluster_name);
        }
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== IVF 变体索引比较测试完成 ===");
}

#[test]
#[serial]
fn test_ivf_variants_distance_algorithms() {
    println!("=== 测试 IVF 变体索引与不同距离算法 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(1048576); // 1MB内存缓冲区
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct IVFRecord {
        id: i32,
        vector: [f32; 6],
        category: i32,
        value: f32,
    };

    // 准备测试数据
    let test_vectors = vec![
        ([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 1),
        ([2.0, 4.0, 6.0, 8.0, 10.0, 12.0], 1), // 同方向，长度2倍
        ([3.0, 6.0, 9.0, 12.0, 15.0, 18.0], 1), // 同方向，长度3倍
        ([-1.0, -2.0, -3.0, -4.0, -5.0, -6.0], 2), // 反方向
        ([1.1, 2.1, 3.1, 4.1, 5.1, 6.1], 2),   // 相似向量
        ([10.0, 20.0, 30.0, 40.0, 50.0, 60.0], 3), // 完全不同的向量
    ];

    // 测试不同距离算法
    let distance_algorithms = vec!["L2", "IP", "COSINE"];

    for distance_alg in distance_algorithms.iter() {
        println!("\n--- 测试距离算法: {} ---", distance_alg);

        // 测试IVF和IVF_PQ
        for ivf_type in ["IVF", "IVF_PQ"].iter() {
            println!("\n测试 {} 索引...", ivf_type);

            // 重置全局数据库实例，确保测试之间的隔离
            remdb::reset_global_db();

            // 初始化包含向量表的数据库
            let config = &IVF_VARIANTS_DB;
            let db = init_global_db(config).unwrap();

            // 插入测试数据
            for (i, (vector, category)) in test_vectors.iter().enumerate() {
                let record = IVFRecord {
                    id: (i + 1) as i32,
                    vector: *vector,
                    category: *category,
                    value: (i + 1) as f32 * 2.0,
                };

                let table = db.get_table_mut(0).unwrap();
                table.insert(&record as *const _ as *const u8).unwrap();
            }

            println!("成功插入测试数据");

            // 初始化索引构建线程池
            crate::index::builder::init_index_build_thread_pool(2);
            println!("索引构建线程池初始化成功");

            // 测试1: 创建带距离算法的IVF变体索引
            println!("测试1: 创建带 {} 距离的 {} 索引", distance_alg, ivf_type);
            let create_query = format!("CREATE INDEX ivf_{}_{}_idx ON IVF_VARIANTS_TABLE (vector) USING {} WITH (DISTANCE={})", 
                                     ivf_type.to_lowercase(), distance_alg.to_lowercase(), ivf_type, distance_alg);
            let result = db.sql_query(&create_query);
            assert!(
                result.is_ok(),
                "创建带 {} 距离的 {} 索引应该成功",
                distance_alg,
                ivf_type
            );
            println!("成功创建带 {} 距离的 {} 索引", distance_alg, ivf_type);

            // 测试2: 验证不同距离操作符
            println!("测试2: 验证不同距离操作符");
            let operator_map = vec![
                ("L2", "<->", "ORDER BY distance"),
                ("IP", "<#>", "ORDER BY distance DESC"),
                ("COSINE", "<=>", "ORDER BY similarity DESC"),
            ];

            // 找到对应距离算法的操作符
            let (_, op, order_by) = operator_map
                .iter()
                .find(|(alg, _, _)| *alg == *distance_alg)
                .unwrap();
            let col_name = if *distance_alg == "COSINE" {
                "similarity"
            } else {
                "distance"
            };

            let query = format!("SELECT id, vector {} [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] as {} FROM IVF_VARIANTS_TABLE {} LIMIT 3", 
                               op, col_name, order_by);

            let result = db.sql_query(&query);
            assert!(
                result.is_ok(),
                "{} 距离 {} 索引查询应该成功",
                distance_alg,
                ivf_type
            );
            println!("{} 距离 {} 索引查询成功", distance_alg, ivf_type);
        }
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== IVF 变体索引与不同距离算法测试完成 ===");
}
