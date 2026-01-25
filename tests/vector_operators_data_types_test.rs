//! 向量操作符与不同数据类型测试
//!
//! 该测试文件验证向量操作符如何与不同数据类型交互，包括标量值、数组和边界情况等。

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

// 定义包含向量字段和多种数据类型的测试表
remdb::table!(
    VECTOR_OPERATORS_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        vector_3d: vector(3), // 3维向量字段
        vector_5d: vector(5), // 5维向量字段
        scalar_i32: i32,      // 32位整数
        scalar_f32: f32,      // 32位浮点数
        scalar_bool: bool,    // 布尔值
        category: i32         // 分类ID
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_OPERATORS_DB,
    tables: [VECTOR_OPERATORS_TABLE]
);

#[test]
#[serial]
fn test_vector_operators_with_scalars() {
    println!("=== 测试向量操作符与标量值 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorOperatorsRecord {
        id: i32,
        vector_3d: [f32; 3], // 3维向量
        vector_5d: [f32; 5], // 5维向量
        scalar_i32: i32,     // 32位整数
        scalar_f32: f32,     // 32位浮点数
        scalar_bool: bool,   // 布尔值
        category: i32,       // 分类ID
    };

    // 插入测试数据
    let record = VectorOperatorsRecord {
        id: 1,
        vector_3d: [1.0, 2.0, 3.0],
        vector_5d: [1.0, 2.0, 3.0, 4.0, 5.0],
        scalar_i32: 10,
        scalar_f32: 3.14,
        scalar_bool: true,
        category: 1,
    };

    let table = db.get_table_mut(0).unwrap();
    table.insert(&record as *const _ as *const u8).unwrap();

    println!("成功插入测试数据");

    // 测试1: 创建向量索引
    println!("测试1: 创建向量索引");
    // 初始化索引构建线程池
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");
    let result =
        db.sql_query("CREATE INDEX vector_3d_idx ON VECTOR_OPERATORS_TABLE (vector_3d) USING HNSW");
    assert!(result.is_ok(), "创建3D向量索引应该成功");
    println!("成功创建向量索引");

    // 测试2: 向量操作符与标量值比较（L2距离）
    println!("测试2: 向量操作符与标量值比较（L2距离）");
    let test_cases = vec!(
        "SELECT id, vector_3d <-> [1.0, 2.0, 3.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance < 1.0",
        "SELECT id, vector_3d <-> [2.0, 3.0, 4.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance > 1.0",
        "SELECT id, vector_3d <-> [0.0, 0.0, 0.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance <= 5.0",
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance >= 3.0"
    );

    for query in test_cases.iter() {
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符与标量比较查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试3: 向量操作符与不同距离算法
    println!("测试3: 向量操作符与不同距离算法");
    let distance_operators = vec!(
        ("L2距离操作符 <->", "SELECT id, vector_3d <-> [1.0, 2.0, 3.0] as l2_distance FROM VECTOR_OPERATORS_TABLE"),
        ("IP距离操作符 <#>", "SELECT id, vector_3d <#> [1.0, 2.0, 3.0] as ip_distance FROM VECTOR_OPERATORS_TABLE"),
        ("Cosine距离操作符 <=>", "SELECT id, vector_3d <=> [1.0, 2.0, 3.0] as cosine_distance FROM VECTOR_OPERATORS_TABLE")
    );

    for (description, query) in distance_operators.iter() {
        println!("\n{}:", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "{} 查询应该成功", description);
        println!("✅ {} 查询成功", description);
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量操作符与标量值测试完成 ===");
}

#[test]
#[serial]
fn test_vector_operators_with_array_literals() {
    println!("=== 测试向量操作符与数组字面量 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorOperatorsRecord {
        id: i32,
        vector_3d: [f32; 3],
        vector_5d: [f32; 5],
        scalar_i32: i32,
        scalar_f32: f32,
        scalar_bool: bool,
        category: i32,
    };

    // 插入多条测试数据
    for i in 1..=5 {
        let record = VectorOperatorsRecord {
            id: i,
            vector_3d: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector_5d: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
                i as f32 * 5.0,
            ],
            scalar_i32: i * 10,
            scalar_f32: i as f32 * 0.5,
            scalar_bool: i % 2 == 1,
            category: i % 3 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        table.insert(&record as *const _ as *const u8).unwrap();
    }

    println!("成功插入 5 条测试数据");

    // 创建向量索引
    // 初始化索引构建线程池
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");
    db.sql_query(
        "CREATE INDEX vector_3d_array_idx ON VECTOR_OPERATORS_TABLE (vector_3d) USING HNSW",
    )
    .unwrap();

    // 测试1: 向量操作符与不同维度数组字面量
    println!("测试1: 向量操作符与不同维度数组字面量");
    let array_literal_tests = vec!(
        // 3D向量查询
        ("3D向量与3D数组字面量", "SELECT id, vector_3d <-> [2.0, 4.0, 6.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 3"),
        ("3D向量与不同值的3D数组", "SELECT id, vector_3d <-> [0.5, 1.0, 1.5] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 2"),
        ("3D向量与零向量", "SELECT id, vector_3d <-> [0.0, 0.0, 0.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance DESC LIMIT 2")
    );

    for (description, query) in array_literal_tests.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符与数组字面量查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试2: 向量操作符在ORDER BY子句中的使用
    println!("\n测试2: 向量操作符在ORDER BY子句中的使用");
    let order_by_tests = vec!(
        "SELECT id, vector_3d FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <-> [2.0, 4.0, 6.0] LIMIT 3",
        "SELECT id, vector_3d FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <#> [3.0, 6.0, 9.0] DESC LIMIT 3",
        "SELECT id, vector_3d FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <=> [1.0, 2.0, 3.0] DESC LIMIT 3"
    );

    for query in order_by_tests.iter() {
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符在ORDER BY中应该成功");
        println!("✅ 查询成功");
    }

    // 测试3: 向量操作符与标量过滤条件组合
    println!("\n测试3: 向量操作符与标量过滤条件组合");
    let combined_tests = vec!(
        "SELECT id, vector_3d, category FROM VECTOR_OPERATORS_TABLE WHERE category = 1 ORDER BY vector_3d <-> [2.0, 4.0, 6.0]",
        "SELECT id, vector_3d, scalar_f32 FROM VECTOR_OPERATORS_TABLE WHERE scalar_f32 > 1.0 ORDER BY vector_3d <-> [3.0, 6.0, 9.0] LIMIT 2",
        "SELECT id, vector_3d, scalar_bool FROM VECTOR_OPERATORS_TABLE WHERE scalar_bool = true ORDER BY vector_3d <-> [1.0, 2.0, 3.0] LIMIT 2"
    );

    for query in combined_tests.iter() {
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符与标量过滤组合查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试4: 别名字段在WHERE和ORDER BY中的使用
    println!("\n测试4: 别名字段在WHERE和ORDER BY中的使用");
    let alias_tests = vec!(
        // 使用别名进行排序
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance",
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance DESC",
        // 使用别名进行过滤
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance >= 3.0",
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance < 10.0",
        // 同时使用别名进行过滤和排序
        "SELECT id, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance BETWEEN 3.0 AND 8.0 ORDER BY distance DESC",
        // 结合其他条件
        "SELECT id, category, vector_3d <-> [3.0, 4.0, 5.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE category = 2 AND distance < 10.0 ORDER BY distance",
    );

    for query in alias_tests.iter() {
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "别名字段查询应该成功");
        println!("✅ 查询成功");
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量操作符与数组字面量测试完成 ===");
}

#[test]
#[serial]
fn test_vector_operators_boundary_cases() {
    println!("=== 测试向量操作符边界情况 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorOperatorsRecord {
        id: i32,
        vector_3d: [f32; 3],
        vector_5d: [f32; 5],
        scalar_i32: i32,
        scalar_f32: f32,
        scalar_bool: bool,
        category: i32,
    };

    // 插入边界情况测试数据
    let boundary_test_data = vec![
        // 正常情况
        (
            1,
            [1.0, 2.0, 3.0],
            [1.0, 2.0, 3.0, 4.0, 5.0],
            10,
            1.5,
            true,
            1,
        ),
        // 零向量
        (
            2,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0],
            0,
            0.0,
            false,
            2,
        ),
        // 大值向量
        (
            3,
            [1000.0, 2000.0, 3000.0],
            [1000.0, 2000.0, 3000.0, 4000.0, 5000.0],
            1000,
            1000.0,
            true,
            3,
        ),
        // 负值向量
        (
            4,
            [-1.0, -2.0, -3.0],
            [-1.0, -2.0, -3.0, -4.0, -5.0],
            -10,
            -1.5,
            false,
            1,
        ),
        // 混合正负值向量
        (
            5,
            [1.0, -2.0, 3.0],
            [1.0, -2.0, 3.0, -4.0, 5.0],
            -5,
            0.5,
            true,
            2,
        ),
    ];

    for (id, vec3d, vec5d, scalar_i32, scalar_f32, scalar_bool, category) in boundary_test_data {
        let record = VectorOperatorsRecord {
            id,
            vector_3d: vec3d,
            vector_5d: vec5d,
            scalar_i32,
            scalar_f32,
            scalar_bool,
            category,
        };

        let table = db.get_table_mut(0).unwrap();
        table.insert(&record as *const _ as *const u8).unwrap();
    }

    println!("成功插入边界情况测试数据");

    // 创建向量索引
    // 初始化索引构建线程池
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");
    db.sql_query(
        "CREATE INDEX vector_3d_boundary_idx ON VECTOR_OPERATORS_TABLE (vector_3d) USING HNSW",
    )
    .unwrap();

    // 测试1: 边界值向量查询
    println!("测试1: 边界值向量查询");
    let boundary_queries = vec!(
        ("查询零向量", "SELECT id, vector_3d <-> [0.0, 0.0, 0.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 2"),
        ("查询大值向量", "SELECT id, vector_3d <-> [1000.0, 2000.0, 3000.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 2"),
        ("查询负值向量", "SELECT id, vector_3d <-> [-1.0, -2.0, -3.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 2"),
        ("查询混合正负值向量", "SELECT id, vector_3d <-> [1.0, -2.0, 3.0] as distance FROM VECTOR_OPERATORS_TABLE ORDER BY distance LIMIT 2")
    );

    for (description, query) in boundary_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "边界值向量查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试2: 距离条件边界值
    println!("\n测试2: 距离条件边界值");
    let distance_boundary_queries = vec!(
        ("距离等于0", "SELECT id, vector_3d <-> [1.0, 2.0, 3.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance = 0.0"),
        ("距离接近0", "SELECT id, vector_3d <-> [1.0, 2.0, 3.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance < 0.1"),
        ("距离大于特定值", "SELECT id, vector_3d <-> [0.0, 0.0, 0.0] as distance FROM VECTOR_OPERATORS_TABLE WHERE distance > 1000.0")
    );

    for (description, query) in distance_boundary_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "距离条件边界值查询应该成功");
        println!("✅ 查询成功");
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量操作符边界情况测试完成 ===");
}

#[test]
#[serial]
fn test_vector_operators_in_different_contexts() {
    println!("=== 测试向量操作符在不同上下文中的使用 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorOperatorsRecord {
        id: i32,
        vector_3d: [f32; 3],
        vector_5d: [f32; 5],
        scalar_i32: i32,
        scalar_f32: f32,
        scalar_bool: bool,
        category: i32,
    };

    // 插入测试数据
    for i in 1..=10 {
        let record = VectorOperatorsRecord {
            id: i,
            vector_3d: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector_5d: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
                i as f32 * 5.0,
            ],
            scalar_i32: i * 5,
            scalar_f32: i as f32 * 0.25,
            scalar_bool: i % 2 == 1,
            category: i % 4 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        table.insert(&record as *const _ as *const u8).unwrap();
    }

    println!("成功插入 10 条测试数据");

    // 创建向量索引
    // 初始化索引构建线程池
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");
    db.sql_query(
        "CREATE INDEX vector_3d_context_idx ON VECTOR_OPERATORS_TABLE (vector_3d) USING HNSW",
    )
    .unwrap();

    // 测试1: 向量操作符在SELECT列表中
    println!("测试1: 向量操作符在SELECT列表中");
    let select_list_queries = vec!(
        ("仅距离字段", "SELECT vector_3d <-> [3.0, 6.0, 9.0] as distance FROM VECTOR_OPERATORS_TABLE"),
        ("ID + 距离字段", "SELECT id, vector_3d <-> [3.0, 6.0, 9.0] as distance FROM VECTOR_OPERATORS_TABLE"),
        ("多字段 + 距离字段", "SELECT id, category, scalar_f32, vector_3d <-> [3.0, 6.0, 9.0] as distance FROM VECTOR_OPERATORS_TABLE"),
        ("多个距离操作符", "SELECT id, vector_3d <-> [3.0, 6.0, 9.0] as l2_distance, vector_3d <#> [3.0, 6.0, 9.0] as ip_distance FROM VECTOR_OPERATORS_TABLE")
    );

    for (description, query) in select_list_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符在SELECT列表中查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试2: 向量操作符在WHERE子句中
    println!("\n测试2: 向量操作符在WHERE子句中");
    let where_clause_queries = vec!(
        ("距离小于阈值", "SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector_3d <-> [3.0, 6.0, 9.0] < 5.0"),
        ("距离大于阈值", "SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector_3d <-> [3.0, 6.0, 9.0] > 10.0"),
        ("距离在范围内", "SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector_3d <-> [3.0, 6.0, 9.0] BETWEEN 5.0 AND 15.0"),
        ("距离条件与其他条件组合", "SELECT id FROM VECTOR_OPERATORS_TABLE WHERE category = 2 AND vector_3d <-> [3.0, 6.0, 9.0] < 10.0")
    );

    for (description, query) in where_clause_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符在WHERE子句中查询应该成功");
        println!("✅ 查询成功");
    }

    // 测试3: 向量操作符在ORDER BY子句中
    println!("\n测试3: 向量操作符在ORDER BY子句中");
    let order_by_queries = vec!(
        ("按距离升序", "SELECT id FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <-> [3.0, 6.0, 9.0] LIMIT 4"),
        ("按距离降序", "SELECT id FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <-> [3.0, 6.0, 9.0] DESC LIMIT 4"),
        ("距离升序 + 其他字段降序", "SELECT id, category FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <-> [3.0, 6.0, 9.0], category DESC LIMIT 5")
    );

    for (description, query) in order_by_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "向量操作符在ORDER BY子句中查询应该成功");
        println!("✅ 查询成功");
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 向量操作符在不同上下文中的使用测试完成 ===");
}

#[test]
#[serial]
fn test_vector_operators_multiple_vectors() {
    println!("=== 测试多个向量字段的操作符使用 ===");

    // 使用堆分配的内存缓冲区，确保测试之间的隔离
    let mut db_memory = Vec::with_capacity(2097152); // 2MB内存缓冲区
    db_memory.resize(2097152, 0u8);

    // 初始化平台抽象层
    remdb::platform::init_platform(&TEST_PLATFORM);

    // 初始化内存分配器
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len())
        .unwrap();

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    // 初始化包含向量表的数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = init_global_db(config).unwrap();

    // 定义向量记录结构
    #[repr(C)]
    struct VectorOperatorsRecord {
        id: i32,
        vector_3d: [f32; 3],
        vector_5d: [f32; 5],
        scalar_i32: i32,
        scalar_f32: f32,
        scalar_bool: bool,
        category: i32,
    };

    // 插入测试数据
    for i in 1..=5 {
        let record = VectorOperatorsRecord {
            id: i,
            vector_3d: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector_5d: [
                i as f32 * 1.0,
                i as f32 * 2.0,
                i as f32 * 3.0,
                i as f32 * 4.0,
                i as f32 * 5.0,
            ],
            scalar_i32: i * 2,
            scalar_f32: i as f32 * 1.0,
            scalar_bool: i % 2 == 1,
            category: i % 2 + 1,
        };

        let table = db.get_table_mut(0).unwrap();
        table.insert(&record as *const _ as *const u8).unwrap();
    }

    println!("成功插入 5 条测试数据");

    // 创建向量索引
    // 初始化索引构建线程池
    crate::index::builder::init_index_build_thread_pool(2);
    println!("索引构建线程池初始化成功");
    db.sql_query(
        "CREATE INDEX vector_3d_multi_idx ON VECTOR_OPERATORS_TABLE (vector_3d) USING HNSW",
    )
    .unwrap();

    // 测试1: 多个向量字段的操作符使用
    println!("测试1: 多个向量字段的操作符使用");
    let multiple_vectors_queries = vec!(
        ("同时查询3D和5D向量", "SELECT id, vector_3d <-> [2.0, 4.0, 6.0] as dist_3d, vector_5d <-> [2.0, 4.0, 6.0, 8.0, 10.0] as dist_5d FROM VECTOR_OPERATORS_TABLE"),
        ("基于3D向量排序，返回5D向量距离", "SELECT id, vector_5d <-> [2.0, 4.0, 6.0, 8.0, 10.0] as dist_5d FROM VECTOR_OPERATORS_TABLE ORDER BY vector_3d <-> [2.0, 4.0, 6.0] LIMIT 3"),
        ("3D向量过滤，5D向量距离排序", "SELECT id, vector_5d <-> [2.0, 4.0, 6.0, 8.0, 10.0] as dist_5d FROM VECTOR_OPERATORS_TABLE WHERE vector_3d <-> [2.0, 4.0, 6.0] < 5.0 ORDER BY dist_5d LIMIT 2")
    );

    for (description, query) in multiple_vectors_queries.iter() {
        println!("\n{}", description);
        println!("查询: {}", query);
        let result = db.sql_query(query);
        assert!(result.is_ok(), "多个向量字段操作符查询应该成功");
        println!("✅ 查询成功");
    }

    // 重置全局数据库实例，确保测试之间的隔离
    remdb::reset_global_db();

    println!("\n=== 多个向量字段的操作符使用测试完成 ===");
}
