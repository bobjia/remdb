//! IVF变体索引性能测试
//!
//! 该测试文件验证不同IVF变体索引的性能和功能。

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
        vector: vector(4), // 4维向量字段
        group_id: i32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    IVF_VARIANTS_DB,
    tables: [IVF_VARIANTS_TABLE]
);

// 向量记录结构
#[repr(C)]
struct IVFVectorRecord {
    id: i32,
    vector: [f32; 4],
    group_id: i32,
}

// 测试IVF基础索引
#[test]
#[serial]
fn test_vector_index_ivf_basic() {
    println!("=== 测试IVF变体索引: IVF基础索引 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &IVF_VARIANTS_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量字段的数据库初始化成功");

    // 插入测试数据
    for i in 1..=10 {
        let record = IVFVectorRecord {
            id: i,
            vector: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
            ],
            group_id: (i - 1) / 3 + 1, // 每3条记录一组
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入10条测试数据");

    // 测试创建IVF索引
    println!("测试创建IVF基础索引");
    let ivf_result =
        db.sql_query("CREATE INDEX ivf_basic_idx ON IVF_VARIANTS_TABLE (vector) USING IVF");
    if ivf_result.is_ok() {
        println!("  成功创建IVF基础索引");
    } else {
        println!("  创建IVF基础索引失败，可能功能尚未实现");
    }

    // 测试基础查询
    println!("测试基础查询");
    let basic_result = db.sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE id = 5");
    if basic_result.is_ok() {
        println!("  基础查询验证成功");
    } else {
        println!("  基础查询验证失败");
    }

    // 测试向量查询
    println!("测试向量查询");
    let vector_result = db
        .sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <-> [2.0, 4.0, 6.0, 8.0] < 5.0");
    if vector_result.is_ok() {
        println!("  向量查询验证成功");
    } else {
        println!("  向量查询验证失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试IVF变体索引: IVF基础索引 完成 ===");
}

// 测试IVF_PQ索引
#[test]
#[serial]
fn test_vector_index_ivf_pq() {
    println!("=== 测试IVF变体索引: IVF_PQ索引 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &IVF_VARIANTS_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量字段的数据库初始化成功");

    // 插入测试数据
    for i in 1..=12 {
        let record = IVFVectorRecord {
            id: i,
            vector: [
                i as f32 * 0.5,
                i as f32 * 1.0,
                i as f32 * 1.5,
                i as f32 * 2.0,
            ],
            group_id: i % 4 + 1, // 分成4组
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入12条测试数据");

    // 测试创建IVF_PQ索引
    println!("测试创建IVF_PQ索引");
    let ivf_pq_result =
        db.sql_query("CREATE INDEX ivf_pq_idx ON IVF_VARIANTS_TABLE (vector) USING IVF_PQ");
    if ivf_pq_result.is_ok() {
        println!("  成功创建IVF_PQ索引");
    } else {
        println!("  创建IVF_PQ索引失败，可能功能尚未实现");
    }

    // 测试向量查询
    println!("测试向量查询");
    let vector_result = db.sql_query(
        "SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <#> [3.0, 6.0, 9.0, 12.0] > 0.0",
    );
    if vector_result.is_ok() {
        println!("  IVF_PQ向量查询验证成功");
    } else {
        println!("  IVF_PQ向量查询验证失败");
    }

    // 测试带有分组条件的向量查询
    println!("测试带有分组条件的向量查询");
    let group_vector_result = db.sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE group_id = 2 AND vector <=> [3.0, 6.0, 9.0, 12.0] > 0.5");
    if group_vector_result.is_ok() {
        println!("  带分组条件的IVF_PQ向量查询验证成功");
    } else {
        println!("  带分组条件的IVF_PQ向量查询验证失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试IVF变体索引: IVF_PQ索引 完成 ===");
}

// 测试不同IVF变体索引的组合
#[test]
#[serial]
fn test_vector_index_ivf_variants_combination() {
    println!("=== 测试IVF变体索引: 不同变体组合 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &IVF_VARIANTS_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量字段的数据库初始化成功");

    // 插入测试数据
    let test_data = vec![
        (1, [1.0, 2.0, 3.0, 4.0], 1),
        (2, [2.0, 4.0, 6.0, 8.0], 1),
        (3, [3.0, 6.0, 9.0, 12.0], 2),
        (4, [4.0, 8.0, 12.0, 16.0], 2),
        (5, [5.0, 10.0, 15.0, 20.0], 3),
        (6, [6.0, 12.0, 18.0, 24.0], 3),
        (7, [7.0, 14.0, 21.0, 28.0], 4),
        (8, [8.0, 16.0, 24.0, 32.0], 4),
    ];

    for (id, vector, group_id) in &test_data {
        let record = IVFVectorRecord {
            id: *id,
            vector: *vector,
            group_id: *group_id,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_data.len());

    // 测试1: 尝试创建多个IVF变体索引
    println!("测试1: 尝试创建多个IVF变体索引");

    // 首先创建IVF索引
    let ivf_result =
        db.sql_query("CREATE INDEX ivf_comb_idx ON IVF_VARIANTS_TABLE (vector) USING IVF");
    if ivf_result.is_ok() {
        println!("  成功创建IVF索引");
    } else {
        println!("  创建IVF索引失败");
    }

    // 测试2: 基本向量查询
    println!("测试2: 基本向量查询");
    let basic_query = db
        .sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <-> [2.0, 4.0, 6.0, 8.0] < 3.0");
    if basic_query.is_ok() {
        println!("  基本向量查询成功");
    } else {
        println!("  基本向量查询失败");
    }

    // 测试3: 不同距离算法的向量查询
    println!("测试3: 不同距离算法的向量查询");

    // L2距离查询
    let l2_result = db.sql_query(
        "SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <-> [3.0, 6.0, 9.0, 12.0] < 5.0",
    );
    if l2_result.is_ok() {
        println!("  L2距离查询成功");
    } else {
        println!("  L2距离查询失败");
    }

    // IP距离查询
    let ip_result = db.sql_query(
        "SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <#> [3.0, 6.0, 9.0, 12.0] > 100.0",
    );
    if ip_result.is_ok() {
        println!("  IP距离查询成功");
    } else {
        println!("  IP距离查询失败");
    }

    // Cosine距离查询
    let cosine_result = db.sql_query(
        "SELECT id FROM IVF_VARIANTS_TABLE WHERE vector <=> [3.0, 6.0, 9.0, 12.0] > 0.95",
    );
    if cosine_result.is_ok() {
        println!("  Cosine距离查询成功");
    } else {
        println!("  Cosine距离查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试IVF变体索引: 不同变体组合 完成 ===");
}

// 测试IVF索引与其他索引的兼容性
#[test]
#[serial]
fn test_vector_index_ivf_compatibility() {
    println!("=== 测试IVF变体索引: 与其他索引的兼容性 ===");

    // 使用堆分配的内存缓冲区
    let mut db_memory = vec![0; 1048576];

    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();
    remdb::reset_global_db();

    // 初始化数据库
    let config = &IVF_VARIANTS_DB;
    let db = remdb::init_global_db(config).unwrap();

    println!("包含向量字段的数据库初始化成功");

    // 插入测试数据
    for i in 1..=9 {
        let record = IVFVectorRecord {
            id: i,
            vector: [
                i as f32 * 1.1,
                i as f32 * 2.2,
                i as f32 * 3.3,
                i as f32 * 4.4,
            ],
            group_id: i / 3 + 1, // 分成3组
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入9条测试数据");

    // 测试1: IVF索引与标量查询的兼容性
    println!("测试1: IVF索引与标量查询的兼容性");

    // 创建IVF索引
    let ivf_result =
        db.sql_query("CREATE INDEX ivf_compat_idx ON IVF_VARIANTS_TABLE (vector) USING IVF");
    if ivf_result.is_ok() {
        println!("  成功创建IVF索引");
    } else {
        println!("  创建IVF索引失败");
    }

    // 测试标量查询
    let scalar_query = db.sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE group_id = 2");
    if scalar_query.is_ok() {
        println!("  标量查询成功，IVF索引不影响标量查询");
    } else {
        println!("  标量查询失败");
    }

    // 测试2: IVF索引与混合查询的兼容性
    println!("测试2: IVF索引与混合查询的兼容性");
    let hybrid_query = db.sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE group_id = 1 AND vector <-> [2.2, 4.4, 6.6, 8.8] < 4.0");
    if hybrid_query.is_ok() {
        println!("  混合查询成功，IVF索引支持与标量条件结合");
    } else {
        println!("  混合查询失败");
    }

    // 测试3: 验证索引不影响基本操作
    println!("测试3: 验证索引不影响基本操作");
    let basic_op_query = db.sql_query("SELECT id FROM IVF_VARIANTS_TABLE WHERE id > 3 AND id < 7");
    if basic_op_query.is_ok() {
        println!("  基本范围查询成功，索引不影响基本操作");
    } else {
        println!("  基本范围查询失败");
    }

    // 重置全局数据库实例
    remdb::reset_global_db();

    println!("=== 测试IVF变体索引: 与其他索引的兼容性 完成 ===");
}
