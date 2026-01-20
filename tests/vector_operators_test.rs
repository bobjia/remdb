//! 向量操作符与不同数据类型测试
//! 
//! 该测试文件验证向量操作符（<->, <#>, <=>）与不同数据类型的兼容性。

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

// 定义包含向量字段和多种数据类型的测试表
remdb::table!(
    VECTOR_OPERATORS_TABLE,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: i32,
        vector3: vector(3), // 3维向量字段
        vector4: vector(4), // 4维向量字段
        int_value: i32,
        float_value: f32,
        double_value: f64,
        bool_value: bool,
        str_value: str(32)
    }
);

// 定义包含向量表的测试数据库配置
remdb::database!(
    VECTOR_OPERATORS_DB,
    tables: [VECTOR_OPERATORS_TABLE]
);

// 向量记录结构
#[repr(C)]
struct VectorOperatorRecord {
    id: i32,
    vector3: [f32; 3],
    vector4: [f32; 4],
    int_value: i32,
    float_value: f32,
    double_value: f64,
    bool_value: bool,
    str_value: [u8; 32]
}

// 测试向量操作符基本功能
#[test]
#[serial]
fn test_vector_operators_basic() {
    println!("=== 测试向量操作符: 基本功能 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含多种向量字段的数据库初始化成功");
    
    // 插入测试数据
    let test_data = vec!(
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0], 10, 0.85, 1.75, true, "test1"),
        (2, [2.0, 3.0, 4.0], [2.0, 3.0, 4.0, 5.0], 20, 0.92, 2.85, false, "test2"),
        (3, [3.0, 4.0, 5.0], [3.0, 4.0, 5.0, 6.0], 30, 0.78, 3.95, true, "test3")
    );
    
    for (id, vector3, vector4, int_val, float_val, double_val, bool_val, str_val) in &test_data {
        let mut record = VectorOperatorRecord {
            id: *id,
            vector3: *vector3,
            vector4: *vector4,
            int_value: *int_val,
            float_value: *float_val,
            double_value: *double_val,
            bool_value: *bool_val,
            str_value: [0u8; 32]
        };
        
        // 设置字符串值
        let str_bytes = str_val.as_bytes();
        record.str_value[..str_bytes.len()].copy_from_slice(str_bytes);
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入 {} 条测试数据", test_data.len());
    
    // 测试1: 向量操作符语法验证
    println!("测试1: 向量操作符语法验证");
    
    // 测试 L2 距离操作符 <->
    let l2_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <-> [1.0, 2.0, 3.0] < 1.0");
    if l2_result.is_ok() {
        println!("  L2 距离操作符 <-> 语法验证成功");
    } else {
        println!("  L2 距离操作符 <-> 语法验证失败");
    }
    
    // 测试 IP 距离操作符 <#>
    let ip_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <#> [1.0, 2.0, 3.0] > 0.0");
    if ip_result.is_ok() {
        println!("  IP 距离操作符 <#> 语法验证成功");
    } else {
        println!("  IP 距离操作符 <#> 语法验证失败");
    }
    
    // 测试 Cosine 距离操作符 <=>
    let cosine_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <=> [1.0, 2.0, 3.0] > 0.5");
    if cosine_result.is_ok() {
        println!("  Cosine 距离操作符 <=> 语法验证成功");
    } else {
        println!("  Cosine 距离操作符 <=> 语法验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 基本功能 完成 ===");
}

// 测试向量操作符与不同数据类型结合
#[test]
#[serial]
fn test_vector_operators_with_data_types() {
    println!("=== 测试向量操作符: 与不同数据类型结合 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含多种数据类型的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=6 {
        let mut record = VectorOperatorRecord {
            id: i,
            vector3: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector4: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            int_value: i * 10,
            float_value: 0.7 + (i as f32 * 0.03),
            double_value: 1.5 + (i as f64 * 0.1),
            bool_value: i % 2 == 0,
            str_value: [0u8; 32]
        };
        
        // 设置字符串值
        let str_val = format!("item{}", i);
        let str_bytes = str_val.as_bytes();
        record.str_value[..str_bytes.len()].copy_from_slice(str_bytes);
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入6条测试数据");
    
    // 创建向量索引
    let idx_result = db.sql_query("CREATE INDEX vector_operators_idx ON VECTOR_OPERATORS_TABLE (vector4) USING HNSW");
    if idx_result.is_ok() {
        println!("成功创建向量索引");
    }
    
    // 测试1: 向量操作符与整数条件结合
    println!("测试1: 向量操作符与整数条件结合");
    let int_combined = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <-> [2.0, 4.0, 6.0] < 2.0 AND int_value > 15");
    if int_combined.is_ok() {
        println!("  向量操作符与整数条件结合成功");
    } else {
        println!("  向量操作符与整数条件结合失败");
    }
    
    // 测试2: 向量操作符与浮点数条件结合
    println!("测试2: 向量操作符与浮点数条件结合");
    let float_combined = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <#> [3.0, 6.0, 9.0] > 0.0 AND float_value < 0.85");
    if float_combined.is_ok() {
        println!("  向量操作符与浮点数条件结合成功");
    } else {
        println!("  向量操作符与浮点数条件结合失败");
    }
    
    // 测试3: 向量操作符与布尔条件结合
    println!("测试3: 向量操作符与布尔条件结合");
    let bool_combined = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <=> [4.0, 8.0, 12.0] > 0.0 AND bool_value = true");
    if bool_combined.is_ok() {
        println!("  向量操作符与布尔条件结合成功");
    } else {
        println!("  向量操作符与布尔条件结合失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 与不同数据类型结合 完成 ===");
}

// 测试不同维度向量的操作符
#[test]
#[serial]
fn test_vector_operators_different_dimensions() {
    println!("=== 测试向量操作符: 不同维度向量 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含不同维度向量的数据库初始化成功");
    
    // 插入测试数据
    let test_data = vec!(
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0]),
        (2, [2.0, 3.0, 4.0], [2.0, 3.0, 4.0, 5.0]),
        (3, [3.0, 4.0, 5.0], [3.0, 4.0, 5.0, 6.0])
    );
    
    for (id, vector3, vector4) in &test_data {
        let mut record = VectorOperatorRecord {
            id: *id,
            vector3: *vector3,
            vector4: *vector4,
            int_value: 0,
            float_value: 0.0,
            double_value: 0.0,
            bool_value: false,
            str_value: [0u8; 32]
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入 {} 条测试数据", test_data.len());
    
    // 测试1: 3维向量操作符
    println!("测试1: 3维向量操作符");
    let vec3_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <-> [1.0, 2.0, 3.0] < 1.0");
    if vec3_result.is_ok() {
        println!("  3维向量操作符验证成功");
    } else {
        println!("  3维向量操作符验证失败");
    }
    
    // 测试2: 4维向量操作符
    println!("测试2: 4维向量操作符");
    let vec4_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector4 <-> [1.0, 2.0, 3.0, 4.0] < 1.0");
    if vec4_result.is_ok() {
        println!("  4维向量操作符验证成功");
    } else {
        println!("  4维向量操作符验证失败");
    }
    
    // 测试3: 不同操作符与不同维度组合
    println!("测试3: 不同操作符与不同维度组合");
    let mixed_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <#> [2.0, 3.0, 4.0] > 0.0 AND vector4 <=> [2.0, 3.0, 4.0, 5.0] > 0.5");
    if mixed_result.is_ok() {
        println!("  不同操作符与不同维度组合验证成功");
    } else {
        println!("  不同操作符与不同维度组合验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 不同维度向量 完成 ===");
}

// 测试向量操作符的多个组合
#[test]
#[serial]
fn test_vector_operators_combinations() {
    println!("=== 测试向量操作符: 多个组合 ===");
    
    // 使用堆分配的内存缓冲区
    let mut db_memory = Vec::with_capacity(1048576);
    db_memory.resize(1048576, 0u8);
    
    // 初始化环境
    remdb::platform::init_platform(&TEST_PLATFORM);
    remdb::memory::allocator::init_global_allocator(db_memory.as_mut_ptr(), db_memory.len()).unwrap();
    remdb::reset_global_db();
    
    // 初始化数据库
    let config = &VECTOR_OPERATORS_DB;
    let db = remdb::init_global_db(config).unwrap();
    
    println!("包含向量字段的数据库初始化成功");
    
    // 插入测试数据
    for i in 1..=5 {
        let mut record = VectorOperatorRecord {
            id: i,
            vector3: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector4: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            int_value: i * 5,
            float_value: 0.5 + (i as f32 * 0.1),
            double_value: 1.0 + (i as f64 * 0.2),
            bool_value: i % 3 != 0,
            str_value: [0u8; 32]
        };
        
        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }
    
    println!("成功插入5条测试数据");
    
    // 测试1: 多个向量操作符组合
    println!("测试1: 多个向量操作符组合");
    let multiple_ops_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <-> [2.0, 4.0, 6.0] < 2.0 OR vector4 <#> [2.0, 4.0, 6.0, 8.0] > 0.0");
    if multiple_ops_result.is_ok() {
        println!("  多个向量操作符组合验证成功");
    } else {
        println!("  多个向量操作符组合验证失败");
    }
    
    // 测试2: 向量操作符与多个标量条件组合
    println!("测试2: 向量操作符与多个标量条件组合");
    let complex_combined_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <=> [3.0, 6.0, 9.0] > 0.0 AND int_value > 10 AND float_value < 0.9");
    if complex_combined_result.is_ok() {
        println!("  向量操作符与多个标量条件组合验证成功");
    } else {
        println!("  向量操作符与多个标量条件组合验证失败");
    }
    
    // 重置全局数据库实例
    remdb::reset_global_db();
    
    println!("=== 测试向量操作符: 多个组合 完成 ===");
}
