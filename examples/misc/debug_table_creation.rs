// 测试表创建和检索

use remdb::config::WALConfig;
use remdb::*;

// 定义数据库内存区域
static mut DB_MEMORY: [u8; 2 * 1024 * 1024] = [0; 2 * 1024 * 1024];

fn main() {
    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())
            .expect("Failed to initialize allocator");

        // 创建数据库配置
        static ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;
        static CONFIG: config::DbConfig = config::DbConfig {
            tables: vec![],
            total_memory: 2 * 1024 * 1024,
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &ALLOCATOR,
            wal_config: WALConfig {
                log_path: "./wal",
                log_mode: config::LogMode::Sync,
                checkpoint_interval_ms: 60000,
                log_file_size_limit: 16 * 1024 * 1024,
                log_prealloc_size: 1 * 1024 * 1024,
                log_segment_size: 16 * 1024 * 1024,
                retained_checkpoints: 3,
                max_consecutive_invalid: 100,
                skip_threshold: 1000,
                skip_block_size: 1024 * 1024,
                max_skip_attempts: 3,
                compression_type: config::WALCompressionType::None,
                compression_level: 3,
            },
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: Some(config::HAConfig {
                node_id: 1, // 默认节点ID为1
                ha_role: remdb::ha::HARole::Auto,
                replication_mode: remdb::ha::ReplicationMode::Async,
                heartbeat_interval_ms: 1000,
                failure_detection_ms: 3000,
                sync_timeout_ms: 2000,
                master_address: None,
                master_port: None,
                replication_port: 5556,
            }),
            time_series_defaults: config::TimeSeriesConfig::DEFAULT,

            model_worker_config: remdb::config::ModelWorkerConfig::DEFAULT,
        };

        // 初始化数据库
        let mut db = init_global_db(&CONFIG).expect("Failed to initialize database");

        // 测试1：使用SQL创建表
        println!("=== 测试1：使用SQL创建表===");
        let create_table_sql = "CREATE TABLE test_table (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    name VARCHAR(50)\n)";

        println!("执行SQL: {}", create_table_sql);
        match db.sql_query(create_table_sql) {
            Ok(result) => {
                println!("✅ SQL执行成功");
                println!("结果: 成功创建表");
            }
            Err(e) => {
                println!("❌ SQL执行失败: {}", e);
            }
        }

        // 测试2：尝试插入数据
        println!("\n=== 测试2：尝试插入数据===");
        let insert_sql = "INSERT INTO test_table (name) VALUES ('Test User')";
        println!("执行SQL: {}", insert_sql);
        match db.sql_query(insert_sql) {
            Ok(result) => {
                println!("✅ SQL执行成功");
                println!("结果: 成功插入数据");
            }
            Err(e) => {
                println!("❌ SQL执行失败: {}", e);
            }
        }

        // 测试3：尝试查询数据
        println!("\n=== 测试3：尝试查询数据===");
        let select_sql = "SELECT * FROM test_table";
        println!("执行SQL: {}", select_sql);
        match db.sql_query(select_sql) {
            Ok(result) => {
                println!("✅ SQL执行成功");
                println!("结果: {}", result.to_string());
            }
            Err(e) => {
                println!("❌ SQL执行失败: {}", e);
            }
        }
    }
}
