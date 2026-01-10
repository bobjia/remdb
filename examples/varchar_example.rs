// 示例：验证VARCHAR类型支持

extern crate alloc;
use remdb::*;
use remdb::config::{DbConfig, DefaultMemoryAllocator};

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
        static CONFIG: DbConfig = DbConfig {
            tables: &[],
            total_memory: 1024 * 1024 * 10, // 10MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &DefaultMemoryAllocator,
            log_path: "varchar_example.wal",
            log_mode: config::LogMode::Async,
            checkpoint_interval_ms: 60000, // 60秒
            log_file_size_limit: 16 * 1024 * 1024, // 16MB
            log_prealloc_size: 1 * 1024 * 1024, // 1MB
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            time_series_defaults: config::TimeSeriesConfig::DEFAULT,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: Some(config::HAConfig {
                ha_role: remdb::ha::HARole::Auto,
                replication_mode: remdb::ha::ReplicationMode::Async,
                heartbeat_interval_ms: 1000,
                failure_detection_ms: 3000,
                sync_timeout_ms: 2000,
                master_address: None,
                master_port: None,
                replication_port: 5556,
                heartbeat_port: 5557,
            }),
        };
        
        // 初始化数据库
        let mut db = init_global_db(&CONFIG).expect("Failed to initialize database");
        
        // 创建表，使用VARCHAR类型
        let create_table_sql = "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name VARCHAR(50),
            email VARCHAR(100),
            age INT
        )";
        
        println!("Executing: {}", create_table_sql);
        
        // 使用db.sql_query执行SQL
        let result = db.sql_query(create_table_sql).expect("Failed to create table");
        
        println!("Table created successfully, status: {}", result.to_string());
        
        // 插入数据
        let insert_sql = "INSERT INTO users (name, email, age) VALUES ('Alice', 'alice@example.com', 30)";
        
        println!("\nExecuting: {}", insert_sql);
        
        // 使用db.sql_query执行SQL
        let insert_result = db.sql_query(insert_sql).expect("Failed to insert data");
        
        println!("Data inserted successfully, affected rows: {}", insert_result.to_string());
        
        // 查询数据
        let select_sql = "SELECT * FROM users";
        
        println!("\nExecuting: {}", select_sql);
        
        // 使用db.sql_query执行SQL
        let select_result = db.sql_query(select_sql).expect("Failed to select data");
        
        println!("Query results:");
        println!("{}", select_result.to_string());
        
        // 测试新的专用方法
        println!("\n=== 测试新的专用方法 ===");
        
        // 使用insert_record插入记录
        println!("\n1. 使用insert_record插入记录:");
        let columns = &["id", "name", "email", "age"];
        let values = &["2", "Bob", "bob@example.com", "25"];
        let affected_rows = db.insert_record("users", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);
        
        // 使用execute_query查询记录
        println!("\n2. 使用execute_query查询记录:");
        let exec_result = db.execute_query("users", &["id", "name", "email", "age"], None, None).unwrap();
        println!("查询结果: {}", exec_result.to_string());
        
        // 使用update_record更新记录
        println!("\n3. 使用update_record更新记录:");
        let update_affected = db.update_record("users", "age = 26, email = 'bob.updated@example.com'", Some("id = 2")).unwrap();
        println!("更新记录成功，影响行数: {}", update_affected);
        
        // 查询验证更新
        let updated_result = db.execute_query("users", &["id", "name", "email", "age"], Some("id = 2"), None).unwrap();
        println!("更新后查询结果: {}", updated_result.to_string());
        
        // 使用delete_record删除记录
        println!("\n4. 使用delete_record删除记录:");
        let delete_affected = db.delete_record("users", Some("id = 1")).unwrap();
        println!("删除记录成功，影响行数: {}", delete_affected);
        
        // 查询剩余记录
        let remaining_result = db.execute_query("users", &["id", "name", "email", "age"], None, None).unwrap();
        println!("删除后剩余记录: {}", remaining_result.to_string());
        
        println!("\nVARCHAR type support verified successfully!");
    }
}