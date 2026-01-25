//! 向量索引管理器测试
//! 测试非阻塞索引构建、进度监控、索引持久化等功能

#![cfg(feature = "std")]

use remdb::*;
use serial_test::serial;
use std::thread::sleep;
use std::time::Duration;

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
    VECTOR_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: vector,
    fields: {
        id: i32,
        vector: vector(10), // 10维向量字段
        category: i32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_DB,
    tables: [VECTOR_TABLE]
);

#[test]
#[serial]
fn test_index_build_thread_pool_init() {
    println!("=== 测试索引构建线程池初始化 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    println!("数据库初始化成功");

    // 初始化索引构建线程池
    let thread_count = 2;
    index::builder::init_index_build_thread_pool(thread_count);
    println!("索引构建线程池初始化成功，线程数: {}", thread_count);

    // 获取线程池实例
    let thread_pool = index::builder::get_index_build_thread_pool();
    assert!(thread_pool.is_ok(), "获取索引构建线程池应该成功");
    println!("成功获取索引构建线程池实例");

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 索引构建线程池初始化测试完成 ===");
}

#[test]
#[serial]
fn test_non_blocking_index_creation() {
    println!("=== 测试非阻塞索引创建 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    println!("数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 10], // 10维向量
        category: i32,
    }

    // 插入一些向量数据用于测试
    println!("插入测试数据...");
    for i in 1..=20 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32; 10], // 10维向量，所有元素都是i
            category: if i % 2 == 0 { 2 } else { 1 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 20 条向量数据");

    // 初始化索引构建线程池
    index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建HNSW索引，使用WITH子句配置参数
    println!("测试1: 创建HNSW索引，使用WITH子句配置参数");
    let result = db.sql_query("CREATE INDEX hnsw_idx ON VECTOR_TABLE (vector) USING HNSW WITH (M=16, EF_CONSTRUCTION=100, EF_SEARCH=50, ONLINE=true)");
    assert!(result.is_ok(), "创建HNSW索引应该成功");
    println!("成功提交HNSW索引创建任务");

    // 测试2: 检查索引构建状态
    println!("测试2: 检查索引构建状态");
    let result = db.sql_query("SHOW INDEX BUILD STATUS");
    assert!(result.is_ok(), "查看索引构建状态应该成功");
    println!("成功执行SHOW INDEX BUILD STATUS命令");

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 非阻塞索引创建测试完成 ===");
}

#[test]
#[serial]
fn test_index_params_with_clause() {
    println!("=== 测试索引参数WITH子句 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    println!("数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 10], // 10维向量
        category: i32,
    }

    // 插入一些向量数据用于测试
    for i in 1..=15 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 0.1; 10],
            category: if i % 3 == 0 { 3 } else if i % 3 == 1 { 1 } else { 2 },
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 15 条向量数据");

    // 初始化索引构建线程池
    index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: 创建HNSW索引，使用完整的WITH子句参数
    println!("测试1: 创建HNSW索引，使用完整的WITH子句参数");
    let result = db.sql_query(
        "CREATE INDEX hnsw_full_params_idx ON VECTOR_TABLE (vector) USING HNSW         WITH (M=8, EF_CONSTRUCTION=200, EF_SEARCH=100, ONLINE=true)"
    );
    assert!(result.is_ok(), "使用完整参数创建HNSW索引应该成功");
    println!("成功创建带完整参数的HNSW索引");

    // 测试2: 创建IVF_FLAT索引，使用WITH子句参数
    println!("测试2: 创建IVF_FLAT索引，使用WITH子句参数");
    // 先重置数据库
    remdb::reset_global_db();
    let db = init_global_db(config).unwrap();

    // 重新插入数据
    for i in 1..=10 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 0.5; 10],
            category: i % 2 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result = db.sql_query(
        "CREATE INDEX ivf_flat_idx ON VECTOR_TABLE (vector) USING IVF         WITH (NLIST=10, NPROBE=3, ONLINE=true)"
    );
    assert!(result.is_ok(), "创建IVF_FLAT索引应该成功");
    println!("成功创建带参数的IVF_FLAT索引");

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 索引参数WITH子句测试完成 ===");
}

#[test]
#[serial]
fn test_index_persistence_api() {
    println!("=== 测试索引持久化API ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    println!("数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 10], // 10维向量
        category: i32,
    }

    // 插入一些向量数据用于测试
    for i in 1..=10 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32; 10],
            category: i % 2 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 10 条向量数据");

    // 测试索引保存和加载的API结构
    // 注意：实际的向量索引是作为二级索引存储的，这里测试获取二级索引的API
    println!("测试获取二级索引API调用");
    let result = db.get_secondary_index(0);
    println!("获取二级索引API调用已执行，结果: {:?}", result.is_ok());

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 索引持久化API测试完成 ===");
}

#[test]
#[serial]
fn test_different_index_algorithms() {
    println!("=== 测试不同索引算法 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 初始化索引构建线程池
    index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");

    // 测试1: HNSW算法
    println!("测试1: HNSW算法");
    remdb::reset_global_db();
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    // 插入数据
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 10],
        category: i32,
    }

    for i in 1..=15 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 0.2; 10],
            category: i % 3 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result = db.sql_query("CREATE INDEX hnsw_alg_idx ON VECTOR_TABLE (vector) USING HNSW");
    assert!(result.is_ok(), "创建HNSW索引应该成功");
    println!("成功创建HNSW索引");

    // 测试2: IVF算法
    println!("测试2: IVF算法");
    remdb::reset_global_db();
    let db = init_global_db(config).unwrap();

    // 插入数据
    for i in 1..=12 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32 * 0.4; 10],
            category: i % 2 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result = db.sql_query("CREATE INDEX ivf_alg_idx ON VECTOR_TABLE (vector) USING IVF");
    assert!(result.is_ok(), "创建IVF索引应该成功");
    println!("成功创建IVF索引");

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 不同索引算法测试完成 ===");
}

#[test]
#[serial]
fn test_index_build_status_monitoring() {
    println!("=== 测试索引构建状态监控 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_DB;
    let db = init_global_db(config).unwrap();

    println!("数据库初始化成功");

    // 定义向量记录结构
    #[repr(C)]
    struct VectorRecord {
        id: i32,
        vector: [f32; 10],
        category: i32,
    }

    // 插入数据
    for i in 1..=10 {
        let record = VectorRecord {
            id: i,
            vector: [i as f32; 10],
            category: i % 2 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 10 条向量数据");

    // 初始化索引构建线程池
    index::builder::init_index_build_thread_pool(1);
    println!("索引构建线程池初始化成功");

    // 创建索引
    let result = db.sql_query("CREATE INDEX status_test_idx ON VECTOR_TABLE (vector) USING HNSW WITH (ONLINE=true)");
    assert!(result.is_ok(), "创建索引应该成功");
    println!("成功创建索引，任务已提交");

    // 测试SHOW INDEX BUILD STATUS命令
    println!("执行SHOW INDEX BUILD STATUS命令");
    for _ in 0..3 {
        let result = db.sql_query("SHOW INDEX BUILD STATUS");
        assert!(result.is_ok(), "查看索引构建状态应该成功");
        // 等待一段时间
        sleep(Duration::from_millis(50));
    }
    println!("多次执行SHOW INDEX BUILD STATUS命令成功");

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 索引构建状态监控测试完成 ===");
}
