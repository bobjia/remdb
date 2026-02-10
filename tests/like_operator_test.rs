use remdb::{config::{DbConfig, DefaultMemoryAllocator, WALConfig, LogMode}, RemDb, sql::execute_query, sql::parse_sql_query};

// 测试内存缓冲区
static mut DB_MEMORY: [u8; 4 * 1024 * 1024] = [0; 4 * 1024 * 1024]; // 4MB

// 测试内存分配器
static DEFAULT_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

// 测试配置
static TEST_CONFIG: DbConfig = DbConfig {
    total_memory: 1024 * 1024, // 1MB
    default_max_records: 100,
    low_power_mode_supported: false,
    low_power_max_records: Some(50),
    wal_config: WALConfig {
        log_path: "",
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 0,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 1,
        max_consecutive_invalid: 100,
        skip_threshold: 1000,
        skip_block_size: 1024 * 1024,
        max_skip_attempts: 3,
        compression_type: remdb::config::WALCompressionType::None,
        compression_level: 3,
    },
    tables: Vec::new(),
    memory_allocator: &DEFAULT_ALLOCATOR,
    time_series_defaults: remdb::time_series::TimeSeriesConfig {
        partition_duration_secs: 3600,
        retention_period_secs: 86400,
        max_partitions: 100,
        compression: remdb::time_series::CompressionType::None,
    },
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,
};

#[test]
fn test_like_operator() {
    // 初始化内存缓冲区
    unsafe {
        DB_MEMORY.fill(0);
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();
    }

    // 创建数据库
    let mut db = RemDb::new(&TEST_CONFIG);
    db.init().unwrap();

    // 创建测试表
    let create_table_sql = "CREATE TABLE test (id INTEGER PRIMARY KEY, name VARCHAR(255));";
    let create_query = parse_sql_query(create_table_sql).unwrap();
    execute_query(&mut db, &create_query).unwrap();
    
    // 插入测试数据
    let insert_sql_1 = "INSERT INTO test (id, name) VALUES (1, 'test');";
    let insert_query_1 = parse_sql_query(insert_sql_1).unwrap();
    execute_query(&mut db, &insert_query_1).unwrap();

    let insert_sql_2 = "INSERT INTO test (id, name) VALUES (2, 'testing');";
    let insert_query_2 = parse_sql_query(insert_sql_2).unwrap();
    execute_query(&mut db, &insert_query_2).unwrap();

    let insert_sql_3 = "INSERT INTO test (id, name) VALUES (3, 'test123');";
    let insert_query_3 = parse_sql_query(insert_sql_3).unwrap();
    execute_query(&mut db, &insert_query_3).unwrap();

    let insert_sql_4 = "INSERT INTO test (id, name) VALUES (4, '123test');";
    let insert_query_4 = parse_sql_query(insert_sql_4).unwrap();
    execute_query(&mut db, &insert_query_4).unwrap();

    let insert_sql_5 = "INSERT INTO test (id, name) VALUES (5, '100%');";
    let insert_query_5 = parse_sql_query(insert_sql_5).unwrap();
    execute_query(&mut db, &insert_query_5).unwrap();

    // 测试 LIKE 'test%' - 应该匹配以'test'开头的字符串
    let sql1 = "SELECT * FROM test WHERE name LIKE 'test%';";
    let query1 = parse_sql_query(sql1).unwrap();
    let result1 = execute_query(&mut db, &query1).unwrap();
    assert_eq!(result1.rows.len(), 3); // 应该匹配 'test', 'testing', 'test123'

    // 测试 LIKE '%test' - 应该匹配以'test'结尾的字符串
    let sql2 = "SELECT * FROM test WHERE name LIKE '%test';";
    let query2 = parse_sql_query(sql2).unwrap();
    let result2 = execute_query(&mut db, &query2).unwrap();
    assert_eq!(result2.rows.len(), 2); // 应该匹配 'test', '123test'

    // 测试 LIKE '%test%' - 应该匹配包含'test'的字符串
    let sql3 = "SELECT * FROM test WHERE name LIKE '%test%';";
    let query3 = parse_sql_query(sql3).unwrap();
    let result3 = execute_query(&mut db, &query3).unwrap();
    assert_eq!(result3.rows.len(), 4); // 应该匹配 'test', 'testing', 'test123', '123test'

    // 测试 LIKE '_test' - 应该匹配长度为5且以'test'结尾的字符串
    let sql4 = "SELECT * FROM test WHERE name LIKE '_test';";
    let query4 = parse_sql_query(sql4).unwrap();
    let result4 = execute_query(&mut db, &query4).unwrap();
    assert_eq!(result4.rows.len(), 0); // 没有匹配

    // 测试 LIKE '100\%' - 应该匹配包含字面量'%'的字符串
    let sql5 = "SELECT * FROM test WHERE name LIKE '100\\%';";
    let query5 = parse_sql_query(sql5).unwrap();
    let result5 = execute_query(&mut db, &query5).unwrap();
    assert_eq!(result5.rows.len(), 1); // 应该匹配 '100%'
}

#[test]
fn test_like_pattern_match_various_cases() {
    // 初始化内存缓冲区
    unsafe {
        DB_MEMORY.fill(0);
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();
    }

    // 创建数据库
    let mut db = RemDb::new(&TEST_CONFIG);
    db.init().unwrap();

    // 创建测试表
    let create_table_sql = "CREATE TABLE test (id INTEGER PRIMARY KEY, name VARCHAR(255));";
    let create_query = parse_sql_query(create_table_sql).unwrap();
    execute_query(&mut db, &create_query).unwrap();

    // 插入测试数据
    let insert_sql = "INSERT INTO test (id, name) VALUES (1, 'test');";
    let insert_query = parse_sql_query(insert_sql).unwrap();
    execute_query(&mut db, &insert_query).unwrap();

    // 测试各种模式
    let test_cases = vec!(
        ("test%", true),    // 以'test'开头
        ("%test", true),    // 以'test'结尾
        ("%test%", true),   // 包含'test'
        ("test", true),     // 完全匹配
        ("testing", false), // 不匹配
        ("%", true),        // 匹配任意字符串
        ("_est", true),     // 匹配长度为4，后3个字符为'est' - 应该匹配
        ("t_st", true),     // 匹配长度为4，第1个为't'，第3个为's'，第4个为't' - 应该匹配
        ("\\%", false),     // 匹配字面量'%' - 应该不匹配
    );

    for (pattern, expected) in test_cases {
        let sql = format!("SELECT * FROM test WHERE name LIKE '{}';", pattern);
        let query = parse_sql_query(&sql).unwrap();
        let result = execute_query(&mut db, &query).unwrap();
        let actual = !result.rows.is_empty();
        assert_eq!(actual, expected, "Pattern '{}' failed: expected {}, got {}", pattern, expected, actual);
    }
}
