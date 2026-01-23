use core::ptr::NonNull;
use remdb::config::{DbConfig, WALConfig};
use remdb::memory::allocator;
use remdb::time_series::table::TimeSeriesConfig;
use remdb::{RemDb, Result};

// 简单的内存分配器实现
struct SimpleAllocator;

impl remdb::config::MemoryAllocator for SimpleAllocator {
    fn allocate(&self, _size: usize) -> Option<NonNull<u8>> {
        static mut BUFFER: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];
        unsafe { Some(NonNull::new(BUFFER.as_mut_ptr()).unwrap()) }
    }

    fn deallocate(&self, _ptr: NonNull<u8>, _size: usize) {
        // 简化实现，不实际释放内存
    }
}

// 显式实现Sync trait
unsafe impl Sync for SimpleAllocator {}

// 静态内存分配器实例
static ALLOCATOR: SimpleAllocator = SimpleAllocator;

fn main() -> Result<()> {
    // 初始化全局内存分配器
    let mut mem_buffer = Box::new([0u8; 16 * 1024 * 1024]); // 16MB
    let ptr = mem_buffer.as_mut_ptr();
    allocator::init_global_allocator(ptr, mem_buffer.len())?;

    // 定义数据库配置
    let config = Box::leak(Box::new(DbConfig {
        tables: &[],                    // 空的数据库配置
        total_memory: 16 * 1024 * 1024, // 16MB，与全局缓冲区大小一致
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR, // 使用我们的静态内存分配器
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000,         // 60秒
            log_file_size_limit: 16 * 1024 * 1024, // 16MB
            log_prealloc_size: 4 * 1024 * 1024,    // 4MB
            log_segment_size: 16 * 1024 * 1024,    // 16MB
            retained_checkpoints: 2,
        },
        time_series_defaults: TimeSeriesConfig {
            partition_duration_secs: 3600,        // 1小时
            retention_period_secs: 7 * 24 * 3600, // 7天
            compression: remdb::time_series::compression::CompressionType::None,
            max_partitions: 100,
        },
        #[cfg(feature = "ha")]
        ha_config: None,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
    }));

    // 初始化数据库
    let mut db = RemDb::new(config);
    db.init()?;

    // 测试不同距离类型的CREATE TABLE语句
    let test_cases = [
        ("L2距离", r#"CREATE TABLE products_l2 (
            id INT32 PRIMARY KEY,
            name TEXT,
            embedding VECTOR(64) WITH DISTANCE=L2
        )"#),
        ("COSINE距离", r#"CREATE TABLE products_cosine (
            id INT32 PRIMARY KEY,
            name TEXT,
            embedding VECTOR(64) WITH DISTANCE=COSINE
        )"#),
        ("IP距离", r#"CREATE TABLE products_ip (
            id INT32 PRIMARY KEY,
            name TEXT,
            embedding VECTOR(64) WITH DISTANCE=IP
        )"#),
        ("INNER_PRODUCT完整写法", r#"CREATE TABLE products_inner_product (
            id INT32 PRIMARY KEY,
            name TEXT,
            embedding VECTOR(64) WITH DISTANCE=INNER_PRODUCT
        )"#),
    ];

    for (test_name, sql) in test_cases.iter() {
        println!("\n测试 {}:", test_name);
        println!("SQL: {}", sql);
        match db.sql_query(sql) {
            Ok(_) => println!("✓ 成功"),
            Err(e) => println!("✗ 失败: {}", e),
        }
    }

    // 验证表创建成功
    println!("\n验证表创建:");
    let tables = ["products_l2", "products_cosine", "products_ip", "products_inner_product"];
    for table in tables.iter() {
        let select_sql = format!("SELECT * FROM {}", table);
        match db.sql_query(&select_sql) {
            Ok(result) => println!("✓ 表 {} 存在，返回 {} 行数据", table, result.rows.len()),
            Err(e) => println!("✗ 表 {} 验证失败: {}", table, e),
        }
    }

    println!("\n所有距离类型测试完成！");

    Ok(())
}
