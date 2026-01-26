extern crate alloc;

use remdb::{DataType, RemDb, Result};
use std::sync::LazyLock;

// 静态内存分配器实例
static DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;

// 静态内存缓冲区，每个测试用例使用自己的缓冲区
static mut DB_MEMORY1: [u8; 8388608] = [0; 8388608]; // 8MB for test 1
static mut DB_MEMORY2: [u8; 8388608] = [0; 8388608]; // 8MB for test 2
static mut DB_MEMORY3: [u8; 8388608] = [0; 8388608]; // 8MB for test 3

// 创建一个静态测试配置
static TEST_CONFIG: LazyLock<remdb::config::DbConfig> = LazyLock::new(|| {
    remdb::config::DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024 * 10, // 10MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &DEFAULT_ALLOCATOR,
        wal_config: remdb::config::WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
        },
        time_series_defaults: remdb::config::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,
    }
});

#[test]
fn test_create_table_with_composite_pk() -> Result<()> {
    // 重置内存缓冲区
    unsafe {
        DB_MEMORY1.fill(0);
    }
    
    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY1.as_mut_ptr(), DB_MEMORY1.len())?;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有复合主键的表
    let fields = [
        ("id1", DataType::UInt32, 0, None, None),
        ("id2", DataType::UInt32, 0, None, None),
        ("name", DataType::String, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    // 定义主键为(id1, id2)
    let primary_key = Some(vec![0, 1]);
    
    db.create_table("test_composite", &fields, primary_key)?;
    
    Ok(())
}

#[test]
fn test_insert_and_query_with_composite_pk() -> Result<()> {
    // 重置内存缓冲区
    unsafe {
        DB_MEMORY2.fill(0);
    }
    
    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY2.as_mut_ptr(), DB_MEMORY2.len())?;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有复合主键的表
    let fields = [
        ("id1", DataType::UInt32, 0, None, None),
        ("id2", DataType::UInt32, 0, None, None),
        ("name", DataType::String, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    // 定义主键为(id1, id2)
    let primary_key = Some(vec![0, 1]);
    
    db.create_table("test_composite", &fields, primary_key)?;
    
    // 插入数据
    let table_id = 1; // 系统表占用0，所以新表ID为1
    let mut table = db.get_table_mut(table_id)?;
    
    // 准备记录数据
    let mut record = [0u8; 4 + 4 + 256 + 8]; // id1(4) + id2(4) + name(256) + value(8)
    
    // 插入第一条记录
    let id1: u32 = 1;
    let id2: u32 = 1;
    let name = "test1";
    let value: f64 = 100.5;
    
    // 设置id1
    record[0..4].copy_from_slice(&id1.to_le_bytes());
    // 设置id2
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    // 设置name
    let name_bytes = name.as_bytes();
    record[8..8+name_bytes.len()].copy_from_slice(name_bytes);
    // 设置value
    record[8+256..8+256+8].copy_from_slice(&value.to_le_bytes());
    
    // 插入记录
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 插入第二条记录，不同的id2
    let id2: u32 = 2;
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 插入第三条记录，不同的id1
    let id1: u32 = 2;
    let id2: u32 = 1;
    record[0..4].copy_from_slice(&id1.to_le_bytes());
    record[4..8].copy_from_slice(&id2.to_le_bytes());
    let record_id = table.insert(record.as_ptr() as *const u8)?;
    assert!(record_id >= 0);
    
    // 尝试插入重复主键记录，应该失败
    let result = table.insert(record.as_ptr() as *const u8);
    assert!(result.is_err());
    
    Ok(())
}

#[test]
fn test_composite_pk_with_three_fields() -> Result<()> {
    // 重置内存缓冲区
    unsafe {
        DB_MEMORY3.fill(0);
    }
    
    // 初始化全局内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY3.as_mut_ptr(), DB_MEMORY3.len())?;
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&*TEST_CONFIG);
    db.init()?;
    
    // 创建带有三字段复合主键的表
    let fields = [
        ("device_id", DataType::UInt32, 0, None, None),
        ("metric_id", DataType::UInt32, 0, None, None),
        ("timestamp", DataType::UInt64, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    // 定义复合主键：(device_id, metric_id, timestamp)
    let primary_key = Some(vec![0, 1, 2]);
    
    db.create_table("metrics", &fields, primary_key)?;
    
    Ok(())
}
