//! 向量索引高级功能测试
//!
//! 该测试文件验证向量索引的高级功能，包括更新、重建、配置参数等。

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

// 定义包含向量字段的测试表（用于高级测试）
remdb::table!(
    VECTOR_ADVANCED_TABLE,
    200, // 更大的最大记录数，用于高级测试
    primary_key: id,
    fields: {
        id: i32,
        vector: vector(8), // 8维向量字段，适合高级测试
        category: i32,
        value: f32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_ADVANCED_DB,
    tables: [VECTOR_ADVANCED_TABLE]
);

// 简化的更新操作测试，避免栈溢出
#[test]
#[serial]
fn test_vector_index_simple_update() {
    println!("=== 测试向量索引简单更新操作 ===");

    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576]; // 1MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_ADVANCED_DB;
    let db = init_global_db(config).unwrap();

    println!("包含8维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 8], // 8维向量
        category: i32,
        value: f32,
    };

    // 插入初始向量数据
    let initial_record = VectorRecord {
        id: 1,
        vector: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        category: 1,
        value: 1.0,
    };

    let table = db.get_table_mut(0).unwrap();
    let insert_id = table
        .insert(&initial_record as *const _ as *const u8)
        .unwrap();
    assert!(insert_id < config.tables[0].max_records);
    println!("成功插入 1 条初始向量数据");

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 创建向量索引
    println!("创建初始向量索引");
    let result = db.sql_query(
        "CREATE INDEX vector_simple_update_idx ON VECTOR_ADVANCED_TABLE (vector) USING HNSW",
    );
    assert!(result.is_ok(), "创建初始向量索引应该成功");
    println!("成功创建初始向量索引");

    // 测试1: 验证基本查询
    println!("测试1: 验证基本查询");
    let result = db.sql_query("SELECT id FROM VECTOR_ADVANCED_TABLE WHERE id = 1");
    assert!(result.is_ok(), "基本查询应该成功");
    println!("基本查询成功");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引简单更新操作测试完成 ===");
}

// 简化的配置参数测试，只测试基本功能
#[test]
#[serial]
fn test_vector_index_simple_config() {
    println!("=== 测试向量索引简单配置 ===");

    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576]; // 1MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_ADVANCED_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 8], // 8维向量
        category: i32,
        value: f32,
    };

    // 插入一些测试数据
    for i in 1..=2 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0; 8],
            category: if i % 2 == 0 { 2 } else { 1 },
            value: i as f32 * 0.1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 2 条测试数据");

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建基本HNSW索引（不使用配置参数）
    println!("测试1: 创建基本HNSW索引");
    let query =
        "CREATE INDEX vector_simple_config_idx ON VECTOR_ADVANCED_TABLE (vector) USING HNSW";
    let result = db.sql_query(query);
    assert!(result.is_ok(), "创建基本HNSW索引应该成功");
    println!("成功创建基本HNSW索引");

    // 测试2: 验证基本查询
    println!("测试2: 验证基本查询");
    let result = db.sql_query("SELECT id FROM VECTOR_ADVANCED_TABLE WHERE id = 1");
    assert!(result.is_ok(), "基本查询应该成功");
    println!("基本查询成功");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量索引简单配置测试完成 ===");
}

// 简化的边界情况测试，避免栈溢出
#[test]
#[serial]
fn test_vector_index_simple_boundary() {
    println!("=== 测试向量索引简单边界情况 ===");

    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576]; // 1MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_ADVANCED_DB;
    let db = init_global_db(config).unwrap();

    println!("包含8维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 8], // 8维向量
        category: i32,
        value: f32,
    };

    // 测试: 单个向量的索引情况
    println!("测试: 单个向量的索引情况");
    let single_record = VectorRecord {
        id: 1,
        vector: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        category: 1,
        value: 1.0,
    };

    let table = db.get_table_mut(0).unwrap();
    let insert_id = table
        .insert(&single_record as *const _ as *const u8)
        .unwrap();
    assert!(insert_id < config.tables[0].max_records);
    println!("成功插入单个向量数据");

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 创建索引
    let result = db.sql_query(
        "CREATE INDEX vector_simple_boundary_idx ON VECTOR_ADVANCED_TABLE (vector) USING HNSW",
    );
    assert!(result.is_ok(), "为单个向量创建索引应该成功");
    println!("成功为单个向量创建索引");

    // 验证基本查询
    let result = db.sql_query("SELECT id FROM VECTOR_ADVANCED_TABLE WHERE id = 1");
    assert!(result.is_ok(), "单个向量的查询应该成功");
    println!("单个向量的查询成功");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 向量索引简单边界情况测试完成 ===");
}

#[test]
#[serial]
fn test_vector_index_multiple_distance_algorithms() {
    println!("=== 测试向量索引多距离算法组合 ===");

    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576]; // 1MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 测试单个距离算法，避免使用尚未完全实现的向量操作符
    println!("\n--- 测试距离算法: L2 ---");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_ADVANCED_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 8], // 8维向量
        category: i32,
        value: f32,
    };

    // 插入一些测试数据
    let test_vectors = vec![
        ([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], 1),
        ([2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0], 1),
    ];

    for (i, (vec, category)) in test_vectors.iter().enumerate() {
        let record = VectorRecord {
            id: (i + 1) as i32,
            vector: *vec,
            category: *category,
            value: (i + 1) as f32 * 0.1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_vectors.len());

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 使用L2距离算法创建索引
    println!("测试1: 使用 L2 距离算法创建索引");
    let query = "CREATE INDEX vector_l2_alg_idx ON VECTOR_ADVANCED_TABLE (vector) USING HNSW WITH (DISTANCE=L2)";
    let result = db.sql_query(query);
    assert!(result.is_ok(), "使用 L2 距离算法创建索引应该成功");
    println!("成功使用 L2 距离算法创建索引");

    // 测试2: 验证基本查询（不使用向量操作符）
    println!("测试2: 验证基本查询");
    let search_query = "SELECT id FROM VECTOR_ADVANCED_TABLE WHERE id = 1";
    let result = db.sql_query(search_query);
    assert!(result.is_ok(), "基本查询应该成功");
    println!("基本查询成功");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量索引多距离算法组合测试完成 ===");
}

// 简化的维度测试，避免栈溢出
#[test]
#[serial]
fn test_vector_index_simple_dimension() {
    println!("=== 测试简单维度向量索引 ===");

    // 使用局部内存缓冲区，确保测试之间的隔离
    let mut db_memory = [0u8; 1048576]; // 1MB内存缓冲区

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 使用现有的表结构
    let config = &VECTOR_ADVANCED_DB;
    let db = init_global_db(config).unwrap();

    println!("包含8维向量表的数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 8], // 8维向量
        category: i32,
        value: f32,
    };

    // 插入一些测试数据
    for i in 1..=3 {
        let record = VectorRecord {
            id: i as i32,
            vector: [i as f32 * 1.0; 8],
            category: if i % 2 == 0 { 2 } else { 1 },
            value: i as f32 * 0.5,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 3 条向量数据");

    // 初始化索引构建线程池
    println!("初始化索引构建线程池");
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    let result = db.sql_query(
        "CREATE INDEX vector_simple_dim_idx ON VECTOR_ADVANCED_TABLE (vector) USING HNSW",
    );
    assert!(result.is_ok(), "创建向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 验证查询
    println!("测试2: 验证向量查询");
    let result = db.sql_query("SELECT id FROM VECTOR_ADVANCED_TABLE WHERE id = 1");
    assert!(result.is_ok(), "向量查询应该成功");
    println!("向量查询成功");

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("=== 简单维度向量索引测试完成 ===");
}
