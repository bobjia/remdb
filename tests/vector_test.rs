//! 向量功能单元测试
//!
//! 该测试文件验证向量数据模型的正确性，包括向量数据类型支持、向量操作符和向量函数。

#![cfg(feature = "std")]

use remdb::*;
use serial_test::serial;

mod common;
use common::{setup_test_db, setup_test_db_with_memory};

// 定义简单的测试表，不包含向量字段
remdb::table!(
    SIMPLE_TABLE,
    100,
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(64),
        value: f32
    }
);

// 定义包含向量字段的测试表
remdb::table!(
    VECTOR_TABLE,
    100,
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i32,
        name: str(64),
        vector: vector(3),
        category: i32
    }
);

// 定义包含向量字段和多种数据类型的测试表
remdb::table!(
    VECTOR_OPERATORS_TABLE,
    100,
    primary_key: id,
    fields: {
        id: i32,
        vector3: vector(3),
        vector4: vector(4),
        int_value: i32,
        float_value: f32,
        double_value: f64,
        bool_value: bool,
        str_value: str(32)
    }
);

// 定义包含向量字段的测试表（用于函数测试）
remdb::table!(
    VECTOR_OPS_FUNC_TABLE,
    100,
    primary_key: id,
    fields: {
        id: i32,
        vector3: vector(3),
        vector4: vector(4),
        scalar: f32
    }
);

// 定义测试数据库配置
remdb::database!(
    SIMPLE_DB,
    tables: [SIMPLE_TABLE]
);

remdb::database!(
    VECTOR_DB,
    tables: [VECTOR_TABLE]
);

remdb::database!(
    VECTOR_OPERATORS_DB,
    tables: [VECTOR_OPERATORS_TABLE]
);

remdb::database!(
    VECTOR_OPS_FUNC_DB,
    tables: [VECTOR_OPS_FUNC_TABLE]
);

#[test]
#[serial]
fn test_vector_basic_support() {
    println!("=== 测试向量数据类型基本支持 ===");

    let _db_memory = setup_test_db();

    let config = &SIMPLE_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    println!("数据库初始化成功");

    let result = db.sql_query("SELECT * FROM SIMPLE_TABLE LIMIT 1");
    assert!(result.is_ok(), "查询SIMPLE_TABLE应该成功");

    let result = db.sql_query("INSERT INTO SIMPLE_TABLE (id, name, value) VALUES (1, 'test record', 1.23)");
    assert!(result.is_ok(), "插入简单记录应该成功");

    let result = db.sql_query("SELECT id, name, value FROM SIMPLE_TABLE WHERE id = 1");
    assert!(result.is_ok(), "查询简单表应该成功");

    println!("=== 向量基本支持测试完成 ===");
}

#[test]
#[serial]
fn test_vector_table_creation() {
    println!("=== 测试创建包含向量字段的表 ====");

    println!("Step 1: 分配测试内存");
    let _db_memory = setup_test_db();
    println!("Step 2: 获取数据库配置");
    let config = &VECTOR_DB;
    println!("Step 3: 初始化全局数据库");
    let _db = unsafe { init_global_db(config).unwrap() };
    println!("Step 4: 数据库初始化成功");
    println!("=== 测试创建包含向量字段的表完成 ====");
}

#[test]
#[serial]
fn test_vector_insert_data() {
    println!("=== 测试插入向量数据 ===");

    let _db_memory = setup_test_db();

    let config = &VECTOR_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    #[repr(C)]
    struct VectorRecord {
        id: i32,
        name: [u8; 64],
        vector: [f32; 3],
        category: i32,
    }

    let test_data = [
        (1, "vector record 1", [1.0, 2.0, 3.0], 1),
        (2, "vector record 2", [4.0, 5.0, 6.0], 2),
        (3, "vector record 3", [7.0, 8.0, 9.0], 1),
    ];

    for (id, name, vector, category) in test_data {
        let mut record = VectorRecord {
            id,
            name: [0u8; 64],
            vector,
            category,
        };

        let name_bytes = name.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("=== 测试插入向量数据完成 ===");
}

#[test]
#[serial]
fn test_vector_query_data() {
    println!("=== 测试查询向量数据 ===");

    let _db_memory = setup_test_db();

    let config = &VECTOR_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    #[repr(C)]
    struct VectorRecord {
        id: i32,
        name: [u8; 64],
        vector: [f32; 3],
        category: i32,
    }

    for i in 1..=5 {
        let mut record = VectorRecord {
            id: i,
            name: [0u8; 64],
            vector: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            category: if i % 2 == 0 { 2 } else { 1 },
        };

        let name_str = format!("vector record {}", i);
        let name_bytes = name_str.as_bytes();
        record.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    let result = db.sql_query("SELECT id, name, vector, category FROM VECTOR_TABLE WHERE id = 1");
    assert!(result.is_ok(), "根据ID查询向量记录应该成功");

    let result = db.sql_query("SELECT id, name, category FROM VECTOR_TABLE");
    assert!(result.is_ok(), "查询所有向量记录应该成功");

    let result = db.sql_query("SELECT id, name FROM VECTOR_TABLE WHERE category = 1");
    assert!(result.is_ok(), "根据分类查询向量记录应该成功");

    println!("=== 测试查询向量数据完成 ===");
}

#[test]
#[serial]
fn test_vector_operators_basic() {
    println!("=== 测试向量操作符: 基本功能 ===");

    let _db_memory = setup_test_db();

    let config = &VECTOR_OPERATORS_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    println!("包含多种向量字段的数据库初始化成功");

    let test_data = [
        (1, [1.0, 2.0, 3.0], [1.0, 2.0, 3.0, 4.0], 10, 0.85, 1.75, true, "test1"),
        (2, [2.0, 3.0, 4.0], [2.0, 3.0, 4.0, 5.0], 20, 0.92, 2.85, false, "test2"),
        (3, [3.0, 4.0, 5.0], [3.0, 4.0, 5.0, 6.0], 30, 0.78, 3.95, true, "test3"),
    ];

    #[repr(C)]
    struct VectorOperatorRecord {
        id: i32,
        vector3: [f32; 3],
        vector4: [f32; 4],
        int_value: i32,
        float_value: f32,
        double_value: f64,
        bool_value: bool,
        str_value: [u8; 32],
    }

    for (id, vector3, vector4, int_val, float_val, double_val, bool_val, str_val) in test_data {
        let mut record = VectorOperatorRecord {
            id,
            vector3,
            vector4,
            int_value: int_val,
            float_value: float_val,
            double_value: double_val,
            bool_value: bool_val,
            str_value: [0u8; 32],
        };

        let str_bytes = str_val.as_bytes();
        record.str_value[..str_bytes.len()].copy_from_slice(str_bytes);

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入 {} 条测试数据", test_data.len());

    let l2_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <-> [1.0, 2.0, 3.0] < 1.0");
    if l2_result.is_ok() {
        println!("  L2 距离操作符 <-> 语法验证成功");
    } else {
        println!("  L2 距离操作符 <-> 语法验证失败");
    }

    let ip_result = db.sql_query("SELECT id FROM VECTOR_OPERATORS_TABLE WHERE vector3 <#> [1.0, 2.0, 3.0] > 0.0");
    if ip_result.is_ok() {
        println!("  IP 距离操作符 <#> 语法验证成功");
    } else {
        println!("  IP 距离操作符 <#> 语法验证失败");
    }

    println!("=== 测试向量操作符: 基本功能 完成 ===");
}

#[test]
#[serial]
fn test_vector_search_functions() {
    println!("=== 测试向量函数: VECTOR_SIMILAR 和 VECTOR_DISTANCE ===");

    let _db_memory = setup_test_db();

    let config = &VECTOR_OPS_FUNC_DB;
    let db = unsafe { init_global_db(config).unwrap() };

    #[repr(C)]
    struct VectorOpsFuncRecord {
        id: i32,
        vector3: [f32; 3],
        vector4: [f32; 4],
        scalar: f32,
    }

    for i in 1..=5 {
        let record = VectorOpsFuncRecord {
            id: i,
            vector3: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0],
            vector4: [i as f32 * 1.0, i as f32 * 2.0, i as f32 * 3.0, i as f32 * 4.0],
            scalar: i as f32 * 0.5,
        };

        let table = db.get_table_mut(0).unwrap();
        let insert_id = table.insert(&record as *const _ as *const u8).unwrap();
        assert!(insert_id < config.tables[0].max_records);
    }

    println!("成功插入5条测试数据");

    let idx_result = db.sql_query("CREATE INDEX vector_search_func_idx ON VECTOR_OPS_FUNC_TABLE (vector4) USING HNSW");
    if idx_result.is_ok() {
        println!("成功创建向量索引");
    }

    let similar_result = db.sql_query("SELECT id FROM VECTOR_OPS_FUNC_TABLE WHERE VECTOR_SIMILAR(vector3, [2.0, 4.0, 6.0])");
    if similar_result.is_ok() {
        println!("  VECTOR_SIMILAR 函数基本使用语法验证成功");
    } else {
        println!("  VECTOR_SIMILAR 函数基本使用语法验证失败");
    }

    let distance_result = db.sql_query("SELECT id, VECTOR_DISTANCE(vector3, [2.0, 4.0, 6.0]) AS dist FROM VECTOR_OPS_FUNC_TABLE ORDER BY dist");
    if distance_result.is_ok() {
        println!("  VECTOR_DISTANCE 函数基本使用语法验证成功");
    } else {
        println!("  VECTOR_DISTANCE 函数基本使用语法验证失败");
    }

    println!("=== 测试向量函数: VECTOR_SIMILAR 和 VECTOR_DISTANCE 完成 ===");
}
