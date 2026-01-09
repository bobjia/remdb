use remdb::*;
use remdb::time_series::*;
use std::time::{Duration, SystemTime};

fn main() {
    // 1. 初始化内存分配器
    println!("1. 初始化内存分配器...");
    let memory_size = 64 * 1024 * 1024; // 64MB
    let mut memory = vec![0u8; memory_size];
    memory::allocator::init_global_allocator(memory.as_mut_ptr(), memory_size)
        .expect("Failed to initialize memory allocator");
    
    // 2. 创建数据库配置
    println!("\n2. 创建数据库配置...");
    static DB_CONFIG: config::DbConfig = config::DbConfig {
        tables: &[],
        total_memory: 64 * 1024 * 1024, // 64MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100000,
        memory_allocator: &config::DefaultMemoryAllocator,
        log_path: "time_series_complete.wal",
        log_mode: config::LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        time_series_defaults: config::TimeSeriesConfig::DEFAULT,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: config::HARole::Auto,
        replication_mode: config::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
        replication_port: 5556,
        heartbeat_port: 5557,
    };
    
    // 3. 初始化数据库
    println!("\n3. 初始化数据库...");
    let db = init_global_db(&DB_CONFIG)
        .expect("Failed to initialize database");

    // 3. 创建时序表
    println!("\n3. 创建时序表...");
    let table_name = "sensor_data";
    let timestamp_field = "timestamp";
    let value_field = "value";
    let tags = &["sensor_id", "location"];
    
    // 创建时序表配置
    let ts_config = TimeSeriesConfig::DEFAULT;
    
    // 创建时序表
    unsafe {
        db.create_time_series_table(
            table_name,
            timestamp_field,
            value_field,
            tags,
            Some(ts_config)
        ).expect("Failed to create time series table");
    };
    println!("时序表 '{}' 创建成功", table_name);

    // 4. 测试时序记录创建
    println!("\n4. 测试时序记录创建...");
    
    // 获取当前时间戳（秒）
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // 创建一个时序记录
    let sensor_id = 1;
    let location = "room1";
    let value = 25.5; // 模拟温度数据
    
    let record = TimeSeriesRecord {
        timestamp: now * 1000000000, // 转换为纳秒
        value,
        tag_count: 2, // 2个标签
        tags: [sensor_id as u64, location.as_ptr() as u64, 0, 0, 0, 0, 0, 0]
    };
    
    println!("创建时序记录成功:");
    println!("  时间戳: {}", record.timestamp);
    println!("  值: {:.2}°C", record.value);
    println!("  标签数量: {}", record.tag_count);

    // 5. 测试生命周期管理
    println!("\n5. 测试生命周期管理...");
    
    // 创建生命周期管理器（保留30分钟数据）
    let lifecycle_manager = LifecycleManager::new(Duration::from_secs(30 * 60)); // 30分钟
    
    // 测试数据是否过期
    let expired_time = now - 60 * 60; // 1小时前
    let recent_time = now - 10 * 60; // 10分钟前
    
    println!("1小时前的数据是否过期: {}", lifecycle_manager.is_expired(expired_time));
    println!("10分钟前的数据是否过期: {}", lifecycle_manager.is_expired(recent_time));

    // 6. 测试时间分区管理
    println!("\n6. 测试时间分区管理...");
    
    // 查询最近5分钟的数据
    let query_start = now - 5 * 60;
    let query_end = now;
    
    // 创建分区管理器（1小时一个分区）
    let mut partition_manager = PartitionManager::new(Duration::from_secs(3600), 100); // 1小时
    
    // 获取时间范围的分区
    let partitions = partition_manager.get_partitions_in_range(query_start, query_end);
    println!("查询时间范围内的分区数量: {}", partitions.len());
    
    // 获取分区总数
    let partition_count = partition_manager.get_partition_count();
    println!("当前分区总数: {}", partition_count);

    // 7. 测试压缩算法
    println!("\n7. 测试压缩算法...");
    
    // 测试Delta编码
    let values = [100, 101, 102, 103, 104, 105, 106, 107, 108, 109];
    println!("原始数据: {:?}", values);
    
    let compressed = compress_delta(&values);
    println!("Delta压缩后大小: {} 字节", compressed.len());
    
    let decompressed = decompress_delta(&compressed, values.len());
    println!("解压缩后数据: {:?}", decompressed);
    println!("压缩和解压缩成功: {}", decompressed == values);

    println!("\n时序数据库完整示例运行结束！");
}
