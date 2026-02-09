mod common;
use common::platform::TEST_PLATFORM;
use common::{setup_test_db, setup_test_db_with_memory};
use serial_test::serial;

use remdb::types::DataType;
use remdb::RemDb;
use remdb::config::DbConfig;
use remdb::config::DefaultMemoryAllocator;
use remdb::config::WALConfig;
use remdb::config::LogMode;
use remdb::time_series::TimeSeriesConfig;
use remdb::time_series::CompressionType;

/// 创建默认的测试数据库配置
fn create_test_config() -> &'static DbConfig {
    Box::leak(Box::new(DbConfig {
        tables: vec![],
        total_memory: 50 * 1024 * 1024, // 50MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &DefaultMemoryAllocator,
        wal_config: WALConfig {
            log_path: "./test_logs",
            log_mode: LogMode::Async,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 0,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 2,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
        },
        time_series_defaults: TimeSeriesConfig {
            max_partitions: 100,
            partition_duration_secs: 3600,
            retention_period_secs: 86400 * 30,
            compression: CompressionType::None,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
    }))
}

/// 测试JSON_EXTRACT函数
#[test]
#[serial]
fn test_json_extract_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_EXTRACT函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok(), "创建表失败: {:?}", create_result);

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25, \"email\": \"alice@example.com\"}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"name\": \"Bob\", \"age\": 30, \"hobbies\": [\"reading\", \"hiking\"]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (3, '{\"name\": \"Charlie\", \"age\": 35, \"active\": true}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (4, '{\"name\": \"David\", \"age\": 40, \"address\": {\"city\": \"New York\", \"zip\": \"10001\"}}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok(), "插入数据应该成功: {}", query);
    }

    // 测试JSON_EXTRACT提取字符串
    let result = db.sql_query("SELECT JSON_EXTRACT(data, '$.name') AS name FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_EXTRACT提取字符串应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_EXTRACT提取数字
    let result = db.sql_query("SELECT JSON_EXTRACT(data, '$.age') AS age FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_EXTRACT提取数字应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_EXTRACT提取嵌套字段
    let result = db.sql_query("SELECT JSON_EXTRACT(data, '$.address.city') AS city FROM JSON_TEST_TABLE WHERE id = 4");
    assert!(result.is_ok(), "JSON_EXTRACT提取嵌套字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_EXTRACT提取数组元素
    let result = db.sql_query("SELECT JSON_EXTRACT(data, '$.hobbies[0]') AS hobby FROM JSON_TEST_TABLE WHERE id = 2");
    assert!(result.is_ok(), "JSON_EXTRACT提取数组元素应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_EXTRACT函数测试通过");
}

/// 测试JSON_VALUE函数
#[test]
#[serial]
fn test_json_value_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_VALUE函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_query = "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25}')";
    let result = db.sql_query(insert_query);
    assert!(result.is_ok());

    // 测试JSON_VALUE提取标量值
    let result = db.sql_query("SELECT JSON_VALUE(data, '$.name') AS name FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_VALUE提取标量值应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_VALUE提取数字
    let result = db.sql_query("SELECT JSON_VALUE(data, '$.age') AS age FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_VALUE提取数字应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_VALUE函数测试通过");
}

/// 测试JSON_QUERY函数
#[test]
#[serial]
fn test_json_query_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_QUERY函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"hobbies\": [\"reading\", \"hiking\"]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"name\": \"Bob\", \"address\": {\"city\": \"New York\", \"zip\": \"10001\"}}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_QUERY提取数组
    let result = db.sql_query("SELECT JSON_QUERY(data, '$.hobbies') AS hobbies FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_QUERY提取数组应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_QUERY提取对象
    let result = db.sql_query("SELECT JSON_QUERY(data, '$.address') AS address FROM JSON_TEST_TABLE WHERE id = 2");
    assert!(result.is_ok(), "JSON_QUERY提取对象应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_QUERY函数测试通过");
}

/// 测试JSON_HAS函数
#[test]
#[serial]
fn test_json_has_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_HAS函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"name\": \"Bob\", \"email\": \"bob@example.com\"}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_HAS检查存在的字段
    let result = db.sql_query("SELECT JSON_HAS(data, '$.age') AS has_age FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_HAS检查存在的字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_HAS检查不存在的字段
    let result = db.sql_query("SELECT JSON_HAS(data, '$.email') AS has_email FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_HAS检查不存在的字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_HAS函数测试通过");
}

/// 测试JSON_TYPE函数
#[test]
#[serial]
fn test_json_type_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_TYPE函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"active\": true, \"count\": 10}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (3, '{\"items\": [\"a\", \"b\", \"c\"]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (4, '{\"value\": null}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_TYPE检查字符串类型
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.name') AS type FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_TYPE检查字符串类型应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_TYPE检查数字类型
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.age') AS type FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_TYPE检查数字类型应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_TYPE检查布尔类型
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.active') AS type FROM JSON_TEST_TABLE WHERE id = 2");
    assert!(result.is_ok(), "JSON_TYPE检查布尔类型应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_TYPE检查数组类型
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.items') AS type FROM JSON_TEST_TABLE WHERE id = 3");
    assert!(result.is_ok(), "JSON_TYPE检查数组类型应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_TYPE检查null类型
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.value') AS type FROM JSON_TEST_TABLE WHERE id = 4");
    assert!(result.is_ok(), "JSON_TYPE检查null类型应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_TYPE函数测试通过");
}

/// 测试JSON_ARRAY_LENGTH函数
#[test]
#[serial]
fn test_json_array_length_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_ARRAY_LENGTH函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"items\": [\"a\", \"b\", \"c\"]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"empty\": []}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (3, '{\"name\": \"test\"}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_ARRAY_LENGTH计算数组长度
    let result = db.sql_query("SELECT JSON_ARRAY_LENGTH(data) AS length FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_ARRAY_LENGTH计算数组长度应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_ARRAY_LENGTH计算空数组长度
    let result = db.sql_query("SELECT JSON_ARRAY_LENGTH(data) AS length FROM JSON_TEST_TABLE WHERE id = 2");
    assert!(result.is_ok(), "JSON_ARRAY_LENGTH计算空数组长度应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_ARRAY_LENGTH计算非数组长度
    let result = db.sql_query("SELECT JSON_ARRAY_LENGTH(data) AS length FROM JSON_TEST_TABLE WHERE id = 3");
    assert!(result.is_ok(), "JSON_ARRAY_LENGTH计算非数组长度应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_ARRAY_LENGTH函数测试通过");
}

/// 测试JSON_ARRAY函数
#[test]
#[serial]
fn test_json_array_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_ARRAY函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 测试JSON_ARRAY创建数组
    let result = db.sql_query("SELECT JSON_ARRAY('a', 'b', 'c') AS arr");
    assert!(result.is_ok(), "JSON_ARRAY创建数组应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_ARRAY创建复杂数组
    let result = db.sql_query("SELECT JSON_ARRAY('test', 123, true, null) AS arr");
    assert!(result.is_ok(), "JSON_ARRAY创建复杂数组应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_ARRAY函数测试通过");
}

/// 测试JSON_OBJECT函数
#[test]
#[serial]
fn test_json_object_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_OBJECT函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 测试JSON_OBJECT创建对象
    let result = db.sql_query("SELECT JSON_OBJECT('name', 'Alice', 'age', 25) AS obj");
    assert!(result.is_ok(), "JSON_OBJECT创建对象应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_OBJECT创建复杂对象
    let result = db.sql_query("SELECT JSON_OBJECT('name', 'Bob', 'email', 'bob@example.com', 'active', true) AS obj");
    assert!(result.is_ok(), "JSON_OBJECT创建复杂对象应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_OBJECT函数测试通过");
}

/// 测试JSON_SET函数
#[test]
#[serial]
fn test_json_set_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_SET函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_query = "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25}')";
    let result = db.sql_query(insert_query);
    assert!(result.is_ok());

    // 测试JSON_SET修改字段
    let result = db.sql_query("SELECT JSON_SET(data, '$.age', 26) AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_SET修改字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_SET添加新字段
    let result = db.sql_query("SELECT JSON_SET(data, '$.email', 'alice@example.com') AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_SET添加新字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_SET函数测试通过");
}

/// 测试JSON_REMOVE函数
#[test]
#[serial]
fn test_json_remove_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_REMOVE函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_query = "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25, \"email\": \"alice@example.com\"}')";
    let result = db.sql_query(insert_query);
    assert!(result.is_ok());

    // 测试JSON_REMOVE删除字段
    let result = db.sql_query("SELECT JSON_REMOVE(data, '$.email') AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_REMOVE删除字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_REMOVE函数测试通过");
}

/// 测试JSON_MERGE_PATCH函数
#[test]
#[serial]
fn test_json_merge_patch_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_MERGE_PATCH函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_query = "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25}')";
    let result = db.sql_query(insert_query);
    assert!(result.is_ok());

    // 测试JSON_MERGE_PATCH合并对象
    let result = db.sql_query("SELECT JSON_MERGE_PATCH(data, '{\"age\": 26, \"email\": \"alice@example.com\"}') AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_MERGE_PATCH合并对象应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_MERGE_PATCH使用null删除字段
    let result = db.sql_query("SELECT JSON_MERGE_PATCH(data, '{\"age\": null}') AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_MERGE_PATCH使用null删除字段应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_MERGE_PATCH函数测试通过");
}

/// 测试JSON_ARRAY_APPEND函数
#[test]
#[serial]
fn test_json_array_append_function() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON_ARRAY_APPEND函数 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_query = "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"hobbies\": [\"reading\", \"hiking\"]}')";
    let result = db.sql_query(insert_query);
    assert!(result.is_ok());

    // 测试JSON_ARRAY_APPEND向数组追加元素
    let result = db.sql_query("SELECT JSON_ARRAY_APPEND(data, '$.hobbies', 'coding') AS new_data FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_ARRAY_APPEND向数组追加元素应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON_ARRAY_APPEND函数测试通过");
}

/// 测试JSON函数组合使用
#[test]
#[serial]
fn test_json_functions_combined() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON函数组合使用 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"name\": \"Alice\", \"age\": 25, \"hobbies\": [\"reading\", \"hiking\"]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"name\": \"Bob\", \"age\": 30, \"hobbies\": [\"coding\"]}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_EXTRACT与WHERE条件结合
    let result = db.sql_query("SELECT * FROM JSON_TEST_TABLE WHERE JSON_EXTRACT(data, '$.age') > 25");
    assert!(result.is_ok(), "JSON_EXTRACT与WHERE条件结合应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_HAS与WHERE条件结合
    let result = db.sql_query("SELECT * FROM JSON_TEST_TABLE WHERE JSON_HAS(data, '$.hobbies')");
    assert!(result.is_ok(), "JSON_HAS与WHERE条件结合应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 2);

    // 测试JSON_TYPE与ORDER BY结合
    let result = db.sql_query("SELECT JSON_TYPE(data, '$.name') AS type FROM JSON_TEST_TABLE ORDER BY id");
    assert!(result.is_ok(), "JSON_TYPE与ORDER BY结合应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 2);

    println!("JSON函数组合使用测试通过");
}

/// 测试JSON函数边界情况
#[test]
#[serial]
fn test_json_functions_edge_cases() {
    let _db_memory = setup_test_db_with_memory(50 * 1024 * 1024); // 50MB

    println!("=== 测试JSON函数边界情况 ====");

    // 创建数据库实例
    let mut db = RemDb::new(create_test_config());
    // 初始化数据库
    assert!(db.init().is_ok());

    // 创建表
    let create_result = db.create_table(
        "JSON_TEST_TABLE",
        &[
            ("id", DataType::Int32, 4, None, None),
            ("data", DataType::Json, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );
    assert!(create_result.is_ok());

    // 插入测试数据
    let insert_queries = [
        "INSERT INTO JSON_TEST_TABLE VALUES (1, '{\"nested\": {\"deep\": {\"value\": 42}}}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (2, '{\"array\": [1, 2, 3, 4, 5]}')",
        "INSERT INTO JSON_TEST_TABLE VALUES (3, '{\"empty\": {}}')",
    ];

    for query in insert_queries {
        let result = db.sql_query(query);
        assert!(result.is_ok());
    }

    // 测试JSON_EXTRACT深层嵌套
    let result = db.sql_query("SELECT JSON_EXTRACT(data, '$.nested.deep.value') AS value FROM JSON_TEST_TABLE WHERE id = 1");
    assert!(result.is_ok(), "JSON_EXTRACT深层嵌套应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_ARRAY_LENGTH计算大数组长度
    let result = db.sql_query("SELECT JSON_ARRAY_LENGTH(data) AS length FROM JSON_TEST_TABLE WHERE id = 2");
    assert!(result.is_ok(), "JSON_ARRAY_LENGTH计算大数组长度应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    // 测试JSON_HAS检查空对象
    let result = db.sql_query("SELECT JSON_HAS(data, '$.empty') AS has_empty FROM JSON_TEST_TABLE WHERE id = 3");
    assert!(result.is_ok(), "JSON_HAS检查空对象应该成功");
    assert_eq!(result.expect("Query should succeed").row_count(), 1);

    println!("JSON函数边界情况测试通过");
}
