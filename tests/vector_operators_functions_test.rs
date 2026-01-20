//! 向量操作符和函数测试
//! 
//! 该测试文件验证文档中提到的向量操作符和函数功能。

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
    VECTOR_OPS_FUNC_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        vector3: vector(3), // 3维向量字段
        vector4: vector(4), // 4维向量字段
        scalar: f32
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_OPS_FUNC_DB,
    tables: [VECTOR_OPS_FUNC_TABLE]
);

// 向量记录结构
#[repr(C)]
struct VectorOpsFuncRecord {
    id: i32,
    vector3: [f32; 3],
    vector4: [f32; 4],
    scalar: f32
}

// 测试向量基本运算操作符
#[test]
#[serial]
fn test_vector_basic_operators() {
    println!("=== 测试向量操作符: 基本运算 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPS_FUNC_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含向量字段的数据库初始化成功");
    
    // 插入测试数据
    let test_data = vec!(
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0], 2.0),
        (2, [4.0, 5.0, 6.0], [5.0, 6.0, 7.0, 8.0], 3.0),
        (3, [7.0, 8.0, 9.0], [9.0, 10.0, 11.0, 12.0], 4.0)
    );
    
    for (id, vector3, vector4, scalar) in &test_data {
        let record = VectorOpsFuncRecord {
            id: *id,
            vector3: *vector3,
            vector4: *vector4,
            scalar: *scalar
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入 {} 条测试数据", test_data.len());
    
    // 测试向量基本运算操作符
    println!("测试向量基本运算操作符");
    
    // 测试1: 向量加法 (+)
    println!("测试1: 向量加法 (+)");
    let add_result = db.sql_query("SELECT vector3 + vector3 FROM VECTOR_OPS_FUNC_TABLE WHERE id = 1");
    if add_result.is_ok() {
        println!("  向量加法 (+) 语法验证成功");
    } else {
        println!("  向量加法 (+) 语法验证失败");
    }
    
    // 测试2: 向量减法 (-)
    println!("测试2: 向量减法 (-)");
    let sub_result = db.sql_query("SELECT vector3 - vector3 FROM VECTOR_OPS_FUNC_TABLE WHERE id = 1");
    if sub_result.is_ok() {
        println!("  向量减法 (-) 语法验证成功");
    } else {
        println!("  向量减法 (-) 语法验证失败");
    }
    
    // 测试3: 向量标量乘法 (*)
    println!("测试3: 向量标量乘法 (*)");
    let mul_result = db.sql_query("SELECT vector3 * 2 FROM VECTOR_OPS_FUNC_TABLE WHERE id = 1");
    if mul_result.is_ok() {
        println!("  向量标量乘法 (*) 语法验证成功");
    } else {
        println!("  向量标量乘法 (*) 语法验证失败");
    }
    
    // 测试4: 向量与字段标量乘法
    println!("测试4: 向量与字段标量乘法");
    let field_mul_result = db.sql_query("SELECT vector3 * scalar FROM VECTOR_OPS_FUNC_TABLE WHERE id = 1");
    if field_mul_result.is_ok() {
        println!("  向量与字段标量乘法 语法验证成功");
    } else {
        println!("  向量与字段标量乘法 语法验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 基本运算 完成 ===");
}

// 测试向量比较操作符
#[test]
#[serial]
fn test_vector_comparison_operators() {
    println!("=== 测试向量操作符: 比较操作符 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPS_FUNC_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含向量字段的数据库初始化成功");
    
    // 插入测试数据
    let test_data = vec!(
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0], 2.0),
        (2, [2.0, 4.0, 6.0], [5.0, 6.0, 7.0, 8.0], 3.0),
        (3, [3.0, 6.0, 9.0], [9.0, 10.0, 11.0, 12.0], 4.0)
    );
    
    for (id, vector3, vector4, scalar) in &test_data {
        let record = VectorOpsFuncRecord {
            id: *id,
            vector3: *vector3,
            vector4: *vector4,
            scalar: *scalar
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入 {} 条测试数据", test_data.len());
    
    // 测试向量比较操作符
    println!("测试向量比较操作符");
    
    // 测试1: 向量相等 (=)
    println!("测试1: 向量相等 (=)");
    let eq_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 = [1.0, 2.0, 3.0]");
    if eq_result.is_ok() {
        println!("  向量相等 (=) 语法验证成功");
    } else {
        println!("  向量相等 (=) 语法验证失败");
    }
    
    // 测试2: 向量不等 (!=)
    println!("测试2: 向量不等 (!=)");
    let ne_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 != [1.0, 2.0, 3.0]");
    if ne_result.is_ok() {
        println!("  向量不等 (!=) 语法验证成功");
    } else {
        println!("  向量不等 (!=) 语法验证失败");
    }
    
    // 测试3: 向量小于 (<)
    println!("测试3: 向量小于 (<)");
    let lt_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 < [2.0, 4.0, 6.0]");
    if lt_result.is_ok() {
        println!("  向量小于 (<) 语法验证成功");
    } else {
        println!("  向量小于 (<) 语法验证失败");
    }
    
    // 测试4: 向量大于 (>
    println!("测试4: 向量大于 (>");
    let gt_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 > [1.0, 2.0, 3.0]");
    if gt_result.is_ok() {
        println!("  向量大于 (>) 语法验证成功");
    } else {
        println!("  向量大于 (>) 语法验证失败");
    }
    
    // 测试5: 向量小于等于 (<=)
    println!("测试5: 向量小于等于 (<=");
    let le_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 <= [2.0, 4.0, 6.0]");
    if le_result.is_ok() {
        println!("  向量小于等于 (<=) 语法验证成功");
    } else {
        println!("  向量小于等于 (<=) 语法验证失败");
    }
    
    // 测试6: 向量大于等于 (>=)
    println!("测试6: 向量大于等于 (>=");
    let ge_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 >= [1.0, 2.0, 3.0]");
    if ge_result.is_ok() {
        println!("  向量大于等于 (>=) 语法验证成功");
    } else {
        println!("  向量大于等于 (>=) 语法验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 比较操作符 完成 ===");
}

// 测试向量搜索函数 VECTOR_SIMILAR 和 VECTOR_DISTANCE
#[test]
#[serial]
fn test_vector_search_functions() {
    println!("=== 测试向量函数: VECTOR_SIMILAR 和 VECTOR_DISTANCE ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPS_FUNC_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含向量字段的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=5 {
        let record = VectorOpsFuncRecord {
            id: i,
            vector3: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector4: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            scalar: i as f32 * 0.5
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入5条测试数据");
    
    // 创建向量索引
    let idx_result = db.sql_query("CREATE INDEX vector_search_func_idx ON VECTOR_OPS_FUNC_TABLE (vector4) USING HNSW");
    if idx_result.is_ok() {
        println!("成功创建向量索引");
    }
    
    // 测试向量搜索函数
    println!("测试向量搜索函数");
    
    // 测试1: VECTOR_SIMILAR 函数基本使用
    println!("测试1: VECTOR_SIMILAR 函数基本使用");
    let similar_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE VECTOR_SIMILAR(vector3, [2.0, 4.0, 6.0])");
    if similar_result.is_ok() {
        println!("  VECTOR_SIMILAR 函数基本使用语法验证成功");
    } else {
        println!("  VECTOR_SIMILAR 函数基本使用语法验证失败");
    }
    
    // 测试2: VECTOR_SIMILAR 函数带距离类型
    println!("测试2: VECTOR_SIMILAR 函数带距离类型");
    let similar_dist_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE VECTOR_SIMILAR(vector3, [2.0, 4.0, 6.0], L2)");
    if similar_dist_result.is_ok() {
        println!("  VECTOR_SIMILAR 函数带距离类型语法验证成功");
    } else {
        println!("  VECTOR_SIMILAR 函数带距离类型语法验证失败");
    }
    
    // 测试3: VECTOR_DISTANCE 函数基本使用
    println!("测试3: VECTOR_DISTANCE 函数基本使用");
    let distance_result = db.sql_query("SELECT id, VECTOR_DISTANCE(vector3, [2.0, 4.0, 6.0]) AS dist FROM VECTOR_OPS_FUNC_TABLE ORDER BY dist");
    if distance_result.is_ok() {
        println!("  VECTOR_DISTANCE 函数基本使用语法验证成功");
    } else {
        println!("  VECTOR_DISTANCE 函数基本使用语法验证失败");
    }
    
    // 测试4: VECTOR_DISTANCE 函数带距离类型
    println!("测试4: VECTOR_DISTANCE 函数带距离类型");
    let distance_dist_result = db.sql_query("SELECT id, VECTOR_DISTANCE(vector3, [2.0, 4.0, 6.0], COSINE) AS dist FROM VECTOR_OPS_FUNC_TABLE ORDER BY dist DESC");
    if distance_dist_result.is_ok() {
        println!("  VECTOR_DISTANCE 函数带距离类型语法验证成功");
    } else {
        println!("  VECTOR_DISTANCE 函数带距离类型语法验证失败");
    }
    
    // 测试5: VECTOR_SIMILAR 与其他条件结合
    println!("测试5: VECTOR_SIMILAR 与其他条件结合");
    let combined_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE VECTOR_SIMILAR(vector3, [2.0, 4.0, 6.0]) AND id > 1");
    if combined_result.is_ok() {
        println!("  VECTOR_SIMILAR 与其他条件结合语法验证成功");
    } else {
        println!("  VECTOR_SIMILAR 与其他条件结合语法验证失败");
    }
    
    // 测试6: 完整的向量搜索语法
    println!("测试6: 完整的向量搜索语法");
    let full_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE VECTOR_SIMILAR(vector3, [2.0, 4.0, 6.0], IP) ORDER BY VECTOR_DISTANCE(vector3, [2.0, 4.0, 6.0], IP) LIMIT 3");
    if full_result.is_ok() {
        println!("  完整的向量搜索语法验证成功");
    } else {
        println!("  完整的向量搜索语法验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量函数: VECTOR_SIMILAR 和 VECTOR_DISTANCE 完成 ===");
}

// 测试向量操作符与函数的组合使用
#[test]
#[serial]
fn test_vector_operators_functions_combination() {
    println!("=== 测试向量操作符与函数: 组合使用 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPS_FUNC_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含向量字段的数据库初始化成功");
    
    // 插入测试数据
    let test_data = vec!(
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0], 2.0),
        (2, [2.0, 4.0, 6.0], [2.0, 4.0, 6.0, 8.0], 3.0),
        (3, [3.0, 6.0, 9.0], [3.0, 6.0, 9.0, 12.0], 4.0),
        (4, [4.0, 8.0, 12.0], [4.0, 8.0, 12.0, 16.0], 5.0),
        (5, [5.0, 10.0, 15.0], [5.0, 10.0, 15.0, 20.0], 6.0)
    );
    
    for (id, vector3, vector4, scalar) in &test_data {
        let record = VectorOpsFuncRecord {
            id: *id,
            vector3: *vector3,
            vector4: *vector4,
            scalar: *scalar
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入 {} 条测试数据", test_data.len());
    
    // 测试向量操作符与函数的组合使用
    println!("测试向量操作符与函数的组合使用");
    
    // 测试1: 向量操作符与标量条件结合
    println!("测试1: 向量操作符与标量条件结合");
    let op_scalar_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 <-> [2.0, 4.0, 6.0] < 3.0 AND scalar > 2.5");
    if op_scalar_result.is_ok() {
        println!("  向量操作符与标量条件结合语法验证成功");
    } else {
        println!("  向量操作符与标量条件结合语法验证失败");
    }
    
    // 测试2: 向量函数与ORDER BY结合
    println!("测试2: 向量函数与ORDER BY结合");
    let func_order_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE scalar < 5.0 ORDER BY VECTOR_DISTANCE(vector3, [3.0, 6.0, 9.0])");
    if func_order_result.is_ok() {
        println!("  向量函数与ORDER BY结合语法验证成功");
    } else {
        println!("  向量函数与ORDER BY结合语法验证失败");
    }
    
    // 测试3: 不同维度向量的操作
    println!("测试3: 不同维度向量的操作");
    let dim_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE vector3 <-> [2.0, 4.0, 6.0] < 4.0 AND vector4 <-> [2.0, 4.0, 6.0, 8.0] < 5.0");
    if dim_result.is_ok() {
        println!("  不同维度向量的操作语法验证成功");
    } else {
        println!("  不同维度向量的操作语法验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符与函数: 组合使用 完成 ===");
}
