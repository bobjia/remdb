use remdb::config::{
    DbConfig, DefaultMemoryAllocator, LogMode, TimeSeriesConfig, WALCompressionType, WALConfig,
};
use remdb::platform::{file_size, get_timestamp_us};
use remdb::transaction::{LogItem, LogManager, LogOperation, VariableSizeLogItem};

mod common;
use common::setup_test_db_with_posix;

#[cfg(windows)]
fn get_test_wal_path(name: &str) -> &'static str {
    let s = format!("C:\\temp\\{}", name);
    Box::leak(s.into_boxed_str())
}

#[cfg(not(windows))]
fn get_test_wal_path(name: &str) -> &'static str {
    let s = format!("/tmp/{}", name);
    Box::leak(s.into_boxed_str())
}

/// 测试不同压缩类型的 WAL 功能
fn test_wal_compression(compression_type: WALCompressionType, test_name: &str) {
    setup_test_db_with_posix();

    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

    // 创建数据库配置
    let config = DbConfig {
        tables: vec![],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        wal_config: WALConfig {
            log_path: &get_test_wal_path(test_name),
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type,
            compression_level: 3,
        },
        time_series_defaults: TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: None,

        model_worker_config: Default::default(),
    };

    unsafe {
        let mut log_manager = LogManager::new(&config).unwrap();

        // 测试1：创建小型可变大小日志项（小于512字节）
        let small_new_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut small_log_item = VariableSizeLogItem {
            header: LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 1,
                old_data_size: 0,
                new_data_size: small_new_data.len() as u16,
                tx_id: 1,
                timestamp: 1234567890,
                checksum: 0,
            },
            old_data: vec![],
            new_data: small_new_data,
        };

        let calculated_checksum =
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &small_log_item,
            );
        small_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&small_log_item);
        assert!(
            result.is_ok(),
            "Failed to write small variable size log item with {:?} compression",
            compression_type
        );

        // 测试2：创建大型可变大小日志项（大于512字节）
        let mut large_new_data = vec![0u8; 1024];
        for i in 0..1024 {
            large_new_data[i] = (i % 256) as u8;
        }
        let mut large_log_item = VariableSizeLogItem {
            header: LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 2,
                old_data_size: 0,
                new_data_size: large_new_data.len() as u16,
                tx_id: 2,
                timestamp: 1234567891,
                checksum: 0,
            },
            old_data: vec![],
            new_data: large_new_data,
        };

        let calculated_checksum =
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &large_log_item,
            );
        large_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&large_log_item);
        assert!(
            result.is_ok(),
            "Failed to write large variable size log item with {:?} compression",
            compression_type
        );

        // 测试3：读取并验证日志项
        let read_small = log_manager.read_variable_size_log_item(0);
        assert!(
            read_small.is_ok(),
            "Failed to read small variable size log item with {:?} compression",
            compression_type
        );
        let read_small = read_small.unwrap();
        assert_eq!(read_small.header.op_type, LogOperation::Insert);
        assert_eq!(read_small.header.table_id, 0);
        assert_eq!(read_small.header.record_id, 1);
        assert_eq!(read_small.header.new_data_size, 8);
        assert_eq!(read_small.new_data.len(), 8);
        assert_eq!(read_small.new_data, vec![1u8, 2, 3, 4, 5, 6, 7, 8]);

        let read_large = log_manager.read_variable_size_log_item(1);
        assert!(
            read_large.is_ok(),
            "Failed to read large variable size log item with {:?} compression",
            compression_type
        );
        let read_large = read_large.unwrap();
        assert_eq!(read_large.header.op_type, LogOperation::Insert);
        assert_eq!(read_large.header.table_id, 0);
        assert_eq!(read_large.header.record_id, 2);
        assert_eq!(read_large.header.new_data_size, 1024);
        assert_eq!(read_large.new_data.len(), 1024);
        for i in 0..1024 {
            assert_eq!(read_large.new_data[i], (i % 256) as u8);
        }

        // 测试4：创建检查点并验证
        let result = log_manager.create_checkpoint();
        assert!(
            result.is_ok(),
            "Failed to create checkpoint with {:?} compression",
            compression_type
        );

        println!("✅ {:?} compression test passed!", compression_type);
    }
}

#[test]
fn test_wal_compression_none() {
    test_wal_compression(WALCompressionType::None, "test_compression_none");
}

#[cfg(feature = "wal-compression-lz4")]
#[test]
fn test_wal_compression_lz4() {
    test_wal_compression(WALCompressionType::LZ4, "test_compression_lz4");
}

#[cfg(feature = "wal-compression-zstd")]
#[test]
fn test_wal_compression_zstd() {
    test_wal_compression(WALCompressionType::ZSTD, "test_compression_zstd");
}

/// 测试压缩对存储空间的影响
#[test]
fn test_wal_compression_storage_impact() {
    setup_test_db_with_posix();

    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

    // 测试不同压缩类型的存储空间使用
    let compression_types = vec![
        (WALCompressionType::None, "test_storage_none"),
        #[cfg(feature = "wal-compression-lz4")]
        (WALCompressionType::LZ4, "test_storage_lz4"),
        #[cfg(feature = "wal-compression-zstd")]
        (WALCompressionType::ZSTD, "test_storage_zstd"),
    ];

    for (compression_type, test_name) in compression_types {
        // 创建数据库配置
        let config = DbConfig {
            tables: vec![],
            total_memory: 1024 * 1024, // 1MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &ALLOCATOR,
            wal_config: WALConfig {
                log_path: &get_test_wal_path(test_name),
                log_mode: LogMode::Sync,
                checkpoint_interval_ms: 60000,
                log_file_size_limit: 16 * 1024 * 1024,
                log_prealloc_size: 1 * 1024 * 1024,
                log_segment_size: 16 * 1024 * 1024,
                retained_checkpoints: 3,
                max_consecutive_invalid: 100,
                skip_threshold: 1000,
                skip_block_size: 1024 * 1024,
                max_skip_attempts: 3,
                compression_type,
                compression_level: 3,
            },
            time_series_defaults: TimeSeriesConfig::DEFAULT,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: None,

            model_worker_config: Default::default(),
        };

        unsafe {
            let mut log_manager = LogManager::new(&config).unwrap();

            // 写入大量重复数据以测试压缩效果
            let mut repeated_data = vec![0u8; 1024];
            for i in 0..1024 {
                repeated_data[i] = (i % 16) as u8; // 创建重复模式
            }

            // 写入100个日志项
            for i in 0..100 {
                let mut log_item = VariableSizeLogItem {
                    header: LogItem {
                        op_type: LogOperation::Insert,
                        table_id: 0,
                        record_id: i as u16,
                        old_data_size: 0,
                        new_data_size: repeated_data.len() as u16,
                        tx_id: i as u32,
                        timestamp: 1234567890 + i as u64,
                        checksum: 0,
                    },
                    old_data: vec![],
                    new_data: repeated_data.clone(),
                };

                let calculated_checksum =
                    remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                        &log_item,
                    );
                log_item.header.checksum = calculated_checksum;

                let result = log_manager.write_variable_size_log_item(&log_item);
                assert!(
                    result.is_ok(),
                    "Failed to write log item with {:?} compression",
                    compression_type
                );
            }

            // 获取日志文件大小
            let wal_file_path = format!("{}/remdb.wal", config.wal_config.log_path);
            let file_size = file_size(wal_file_path.as_str()).unwrap();
            println!(
                "📊 {:?} compression: WAL file size = {} bytes for 100 log items",
                compression_type, file_size
            );
        }
    }

    println!("✅ WAL compression storage impact test completed!");
}

/// 测试压缩对性能的影响
#[test]
fn test_wal_compression_performance() {
    setup_test_db_with_posix();

    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;

    // 测试不同压缩类型的性能
    let compression_types = vec![
        (WALCompressionType::None, "test_perf_none"),
        #[cfg(feature = "wal-compression-lz4")]
        (WALCompressionType::LZ4, "test_perf_lz4"),
        #[cfg(feature = "wal-compression-zstd")]
        (WALCompressionType::ZSTD, "test_perf_zstd"),
    ];

    for (compression_type, test_name) in compression_types {
        // 创建数据库配置
        let config = DbConfig {
            tables: vec![],
            total_memory: 1024 * 1024, // 1MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &ALLOCATOR,
            wal_config: WALConfig {
                log_path: &get_test_wal_path(test_name),
                log_mode: LogMode::Sync,
                checkpoint_interval_ms: 60000,
                log_file_size_limit: 16 * 1024 * 1024,
                log_prealloc_size: 1 * 1024 * 1024,
                log_segment_size: 16 * 1024 * 1024,
                retained_checkpoints: 3,
                max_consecutive_invalid: 100,
                skip_threshold: 1000,
                skip_block_size: 1024 * 1024,
                max_skip_attempts: 3,
                compression_type,
                compression_level: 3,
            },
            time_series_defaults: TimeSeriesConfig::DEFAULT,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: None,

            model_worker_config: Default::default(),
        };

        unsafe {
            let mut log_manager = LogManager::new(&config).unwrap();

            // 准备测试数据
            let mut test_data = vec![0u8; 512];
            for i in 0..512 {
                test_data[i] = (i % 256) as u8;
            }

            // 测试写入性能
            let start_time = get_timestamp_us();

            // 写入500个日志项
            for i in 0..500 {
                let mut log_item = VariableSizeLogItem {
                    header: LogItem {
                        op_type: LogOperation::Insert,
                        table_id: 0,
                        record_id: i as u16,
                        old_data_size: 0,
                        new_data_size: test_data.len() as u16,
                        tx_id: i as u32,
                        timestamp: 1234567890 + i as u64,
                        checksum: 0,
                    },
                    old_data: vec![],
                    new_data: test_data.clone(),
                };

                let calculated_checksum =
                    remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                        &log_item,
                    );
                log_item.header.checksum = calculated_checksum;

                let result = log_manager.write_variable_size_log_item(&log_item);
                assert!(
                    result.is_ok(),
                    "Failed to write log item with {:?} compression",
                    compression_type
                );
            }

            let end_time = get_timestamp_us();
            let write_time = end_time - start_time;
            println!(
                "📈 {:?} compression: Write time = {} us for 500 log items",
                compression_type, write_time
            );

            // 测试读取性能
            let start_time = get_timestamp_us();

            // 读取所有日志项
            for i in 0..500 {
                let result = log_manager.read_variable_size_log_item(i as u32);
                assert!(
                    result.is_ok(),
                    "Failed to read log item {} with {:?} compression",
                    i,
                    compression_type
                );
            }

            let end_time = get_timestamp_us();
            let read_time = end_time - start_time;
            println!(
                "📈 {:?} compression: Read time = {} us for 500 log items",
                compression_type, read_time
            );
        }
    }

    println!("✅ WAL compression performance test completed!");
}
