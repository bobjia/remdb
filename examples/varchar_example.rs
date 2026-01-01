// 示例：验证VARCHAR类型支持

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
            log_mode: config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
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
        
        // 解析SQL
        let query = sql::parse_sql_query(create_table_sql).expect("Failed to parse SQL");
        
        // 执行SQL
        let result = sql::execute_query(&mut db, &query).expect("Failed to execute SQL");
        
        println!("Table created successfully, status: {}", result.to_string());
        
        // 插入数据
        let insert_sql = "INSERT INTO users (name, email, age) VALUES ('Alice', 'alice@example.com', 30)";
        
        println!("\nExecuting: {}", insert_sql);
        
        // 解析SQL
        let insert_query = sql::parse_sql_query(insert_sql).expect("Failed to parse INSERT SQL");
        
        // 执行SQL
        let insert_result = sql::execute_query(&mut db, &insert_query).expect("Failed to execute INSERT SQL");
        
        println!("Data inserted successfully, affected rows: {}", insert_result.to_string());
        
        // 查询数据
        let select_sql = "SELECT * FROM users";
        
        println!("\nExecuting: {}", select_sql);
        
        // 解析SQL
        let select_query = sql::parse_sql_query(select_sql).expect("Failed to parse SELECT SQL");
        
        // 执行SQL
        let select_result = sql::execute_query(&mut db, &select_query).expect("Failed to execute SELECT SQL");
        
        println!("Query results:");
        println!("{}", select_result.to_string());
        
        println!("\nVARCHAR type support verified successfully!");
    }
}