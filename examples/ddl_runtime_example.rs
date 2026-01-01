// 运行时DDL配置示例

use remdb::{RemDb, DdlExecutor, types::{DataType, IndexType}};
use remdb::config::{DbConfig, MemoryAllocator};
use remdb::memory::allocator::init_global_allocator;
use core::ptr::NonNull;

// 简单的内存分配器实现
struct SimpleAllocator;

impl SimpleAllocator {
    pub const fn new() -> Self {
        Self
    }
}

impl MemoryAllocator for SimpleAllocator {
    fn allocate(&self, _size: usize) -> Option<NonNull<u8>> {
        // 简化实现，总是返回None
        None
    }
    
    fn deallocate(&self, _ptr: NonNull<u8>, _size: usize) {
        // 简化实现，不实际释放内存
    }
}

fn main() {
    println!("=== RemDb Runtime DDL Configuration Example ===\n");
    
    // 初始化全局内存分配器
    static mut MEMORY_BUFFER: [u8; 1024 * 1024] = [0; 1024 * 1024];
    unsafe {
        init_global_allocator(MEMORY_BUFFER.as_mut_ptr(), MEMORY_BUFFER.len())
            .expect("Failed to initialize global allocator");
    }
    
    // 创建内存分配器
    static ALLOCATOR: SimpleAllocator = SimpleAllocator::new();
    
    // 创建数据库配置
    static CONFIG: DbConfig = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10, // 减少默认最大记录数，避免内存不足
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
    };
    
    // 创建数据库实例
    let mut db = RemDb::new(&CONFIG);
    
    // 初始化数据库和平台
    db.init().expect("Failed to initialize database");
    
    println!("1. Testing DDL API - DdlExecutor trait");
    println!("=========================================");
    
    // 使用DdlExecutor trait创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32),
            ("name", DataType::String),
            ("age", DataType::UInt8),
            ("active", DataType::Bool),
        ],
        Some(0) // 主键为id字段
    );
    
    match result {
        Ok(_) => println!("   ✓ Table 'users' created successfully!"),
        Err(e) => println!("   ✗ Failed to create table: {:?} ", e),
    }
    
    // 使用DdlExecutor trait创建索引
    let result = db.create_index(
        "users",
        "name",
        IndexType::BTree
    );
    
    match result {
        Ok(_) => println!("   ✓ Index on 'users.name' created successfully!"),
        Err(e) => println!("   ✗ Failed to create index: {:?} ", e),
    }
    
    println!("\n2. Testing SQL DDL Statements");
    println!("===========================");
    
    // 使用SQL语句创建表
    let result = db.sql_query(
        "CREATE TABLE products (id UINT32 PRIMARY KEY, name STRING, price FLOAT32, in_stock BOOL);"
    );
    
    match result {
        Ok(_) => println!("   ✓ Table 'products' created successfully via SQL!"),
        Err(e) => println!("   ✗ Failed to create table via SQL: {:?} ", e),
    }
    
    // 使用SQL语句创建索引
    let result = db.sql_query(
        "CREATE INDEX idx_product_name ON products (name) USING BTree;"
    );
    
    match result {
        Ok(_) => println!("   ✓ Index 'idx_product_name' created successfully via SQL!"),
        Err(e) => println!("   ✗ Failed to create index via SQL: {:?}", e),
    }
    
    println!("\n=== Example Completed ===");
}