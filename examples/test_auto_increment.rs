// 测试AUTO_INCREMENT支持

use remdb::*;

// 定义数据库内存区域
static mut DB_MEMORY: [u8; 1024 * 1024] = [0; 1024 * 1024];

fn main() {
    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        ).expect("Failed to initialize allocator");
        
        // 创建数据库配置
        static ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;
        static CONFIG: config::DbConfig = config::DbConfig {
            tables: &[],
            total_memory: 1024 * 1024,
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &ALLOCATOR,
            log_path: "auto_increment_test.wal",
            log_mode: config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_role: config::HARole::Auto,
            #[cfg(feature = "ha")]
            replication_mode: config::ReplicationMode::Async,
            #[cfg(feature = "ha")]
            heartbeat_interval_ms: 1000,
            #[cfg(feature = "ha")]
            failure_detection_ms: 3000,
            #[cfg(feature = "ha")]
            sync_timeout_ms: 2000,
            #[cfg(feature = "ha")]
            master_address: None,
            #[cfg(feature = "ha")]
            master_port: None,
            #[cfg(feature = "ha")]
            replication_port: 5556,
            #[cfg(feature = "ha")]
            heartbeat_port: 5557,
            time_series_defaults: config::TimeSeriesConfig::DEFAULT,
        };
        
        // 初始化数据库
        let mut db = init_global_db(&CONFIG).expect("Failed to initialize database");
        
        // 测试1：测试AUTOINCREMENT（无下划线）
        println!("=== 测试1：AUTOINCREMENT（无下划线）===");
        let create_table_sql1 = "CREATE TABLE test1 (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    name VARCHAR(50)\n)";
        
        match sql::parse_sql_query(create_table_sql1) {
            Ok(query) => {
                println!("✅ SQL解析成功: AUTOINCREMENT");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: AUTOINCREMENT");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试2：测试AUTO_INCREMENT（带下划线）
        println!("\n=== 测试2：AUTO_INCREMENT（带下划线）===");
        let create_table_sql2 = "CREATE TABLE test2 (\n    id INTEGER PRIMARY KEY AUTO_INCREMENT,\n    name VARCHAR(50)\n)";
        
        match sql::parse_sql_query(create_table_sql2) {
            Ok(query) => {
                println!("✅ SQL解析成功: AUTO_INCREMENT");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: AUTO_INCREMENT");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试3：测试数据插入（显式指定主键）
        println!("\n=== 测试3：数据插入（显式指定主键）===");
        let insert_explicit_sql = "INSERT INTO test1 (id, name) VALUES (1, 'Test with explicit ID')";
        
        match sql::parse_sql_query(insert_explicit_sql) {
            Ok(query) => {
                println!("✅ SQL解析成功: 显式插入");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: 显式插入");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试4：测试数据插入（不指定主键，使用自增）
        println!("\n=== 测试4：数据插入（不指定主键，使用自增）===");
        let insert_auto_sql = "INSERT INTO test1 (name) VALUES ('Test with auto ID')";
        
        match sql::parse_sql_query(insert_auto_sql) {
            Ok(query) => {
                println!("✅ SQL解析成功: 自增插入");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: 自增插入");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试5：测试数据插入（显式指定主键）到test2表
        println!("\n=== 测试5：test2表数据插入（显式指定主键）===");
        let insert_explicit_sql2 = "INSERT INTO test2 (id, name) VALUES (1, 'Test2 with explicit ID')";
        
        match sql::parse_sql_query(insert_explicit_sql2) {
            Ok(query) => {
                println!("✅ SQL解析成功: test2显式插入");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: test2显式插入");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试6：测试数据插入（不指定主键，使用自增）到test2表
        println!("\n=== 测试6：test2表数据插入（不指定主键，使用自增）===");
        let insert_auto_sql2 = "INSERT INTO test2 (name) VALUES ('Test2 with auto ID')";
        
        match sql::parse_sql_query(insert_auto_sql2) {
            Ok(query) => {
                println!("✅ SQL解析成功: test2自增插入");
                if let Err(e) = sql::execute_query(&mut db, &query) {
                    println!("❌ 执行失败: {}", e);
                } else {
                    println!("✅ 执行成功: test2自增插入");
                }
            },
            Err(e) => {
                println!("❌ 解析失败: {}", e);
            }
        }
        
        // 测试7：测试查询数据，验证test1表插入结果
        println!("\n=== 测试7：查询test1表，验证插入结果===");
        let select_sql1 = "SELECT * FROM test1";
        
        match db.sql_query(select_sql1) {
            Ok(result) => {
                println!("✅ 查询成功，test1结果如下：");
                println!("{}", result.to_string());
            },
            Err(e) => {
                println!("❌ 查询失败: {}", e);
            }
        }
        
        // 测试8：测试查询数据，验证test2表插入结果
        println!("\n=== 测试8：查询test2表，验证插入结果===");
        let select_sql2 = "SELECT * FROM test2";
        
        match db.sql_query(select_sql2) {
            Ok(result) => {
                println!("✅ 查询成功，test2结果如下：");
                println!("{}", result.to_string());
            },
            Err(e) => {
                println!("❌ 查询失败: {}", e);
            }
        }
        
        // 测试9：使用新的专用方法查询数据
        println!("\n=== 测试9：使用新的专用方法查询数据===");
        match db.execute_query("test1", &["id", "name"], None, None) {
            Ok(result) => {
                println!("✅ 使用execute_query查询test1成功：");
                println!("{}", result.to_string());
            },
            Err(e) => {
                println!("❌ 使用execute_query查询失败: {}", e);
            }
        }
        
        // 测试10：使用新的专用方法插入数据
        println!("\n=== 测试10：使用新的专用方法插入数据===");
        let columns = &["name"];
        let values = &["New User"];
        match db.insert_record("test1", columns, values) {
            Ok(affected_rows) => {
                println!("✅ 使用insert_record插入成功，影响行数: {}", affected_rows);
                
                // 查询验证
                if let Ok(result) = db.execute_query("test1", &["id", "name"], None, None) {
                    println!("✅ 插入后查询结果：");
                    println!("{}", result.to_string());
                }
            },
            Err(e) => {
                println!("❌ 使用insert_record插入失败: {}", e);
            }
        }
    }
}