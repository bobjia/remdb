use remdb::{RemDb, Result};
use remdb::config::{DbConfig, WALConfig};
use remdb::memory::allocator;
use remdb::time_series::table::TimeSeriesConfig;
use core::ptr::NonNull;

// 简单的内存分配器实现
struct SimpleAllocator;

impl remdb::config::MemoryAllocator for SimpleAllocator {
    fn allocate(&self, _size: usize) -> Option<NonNull<u8>> {
        static mut BUFFER: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024];
        unsafe {
            Some(NonNull::new(BUFFER.as_mut_ptr()).unwrap())
        }
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
    // 初始化全局内存分配器 - 增加内存大小到16MB
    let mut mem_buffer = Box::new([0u8; 16 * 1024 * 1024]); // 16MB
    let ptr = mem_buffer.as_mut_ptr();
    allocator::init_global_allocator(ptr, mem_buffer.len())?;
    
    // 定义数据库配置
    let config = Box::leak(Box::new(DbConfig {
        tables: &[], // 空的数据库配置
        total_memory: 16 * 1024 * 1024, // 16MB，与全局缓冲区大小一致
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR, // 使用我们的静态内存分配器
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000, // 60秒
            log_file_size_limit: 16 * 1024 * 1024, // 16MB
            log_prealloc_size: 4 * 1024 * 1024, // 4MB
            log_segment_size: 16 * 1024 * 1024, // 16MB
            retained_checkpoints: 2,
        },
        time_series_defaults: TimeSeriesConfig {
            partition_duration_secs: 3600, // 1小时
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
    
    // 初始化数据库
    db.init()?;
    
    // 创建一个包含向量字段的表 - 完整版，使用WITH DISTANCE子句
    let sql = r#"CREATE TABLE products (
        id INT32 PRIMARY KEY,
        name VARCHAR(255),
        embedding VECTOR(128) WITH DISTANCE=L2
    )"#;
    
    // 使用sql_query方法执行DDL语句
    let _result = db.sql_query(sql)?;
    
    println!("表创建成功！");
    
    // 简化版本，直接打印结果
    println!("向量表示例运行完成！");
    
    Ok(())
}