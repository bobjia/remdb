use remdb::config::{
    DbConfig, DefaultMemoryAllocator, LogMode, TimeSeriesConfig, WALCompressionType, WALConfig,
};
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

#[test]
fn test_variable_size_log_item_write_and_read() {
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
            log_path: get_test_wal_path("test_variable_size"),
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: WALCompressionType::None,
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

        let calculated_checksum = unsafe {
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &small_log_item,
            )
        };
        small_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&small_log_item);
        assert!(
            result.is_ok(),
            "Failed to write small variable size log item"
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

        let calculated_checksum = unsafe {
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &large_log_item,
            )
        };
        large_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&large_log_item);
        assert!(
            result.is_ok(),
            "Failed to write large variable size log item"
        );

        // 测试3：创建带有旧数据的可变大小日志项
        let old_data = vec![1u8, 2, 3, 4];
        let new_data = vec![5u8, 6, 7, 8];
        let mut update_log_item = VariableSizeLogItem {
            header: LogItem {
                op_type: LogOperation::Update,
                table_id: 0,
                record_id: 1,
                old_data_size: old_data.len() as u16,
                new_data_size: new_data.len() as u16,
                tx_id: 3,
                timestamp: 1234567892,
                checksum: 0,
            },
            old_data,
            new_data,
        };

        let calculated_checksum = unsafe {
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &update_log_item,
            )
        };
        update_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&update_log_item);
        assert!(
            result.is_ok(),
            "Failed to write update variable size log item"
        );

        // 测试4：读取并验证日志项
        let read_small = log_manager.read_variable_size_log_item(0);
        assert!(
            read_small.is_ok(),
            "Failed to read small variable size log item"
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
            "Failed to read large variable size log item"
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

        let read_update = log_manager.read_variable_size_log_item(2);
        assert!(
            read_update.is_ok(),
            "Failed to read update variable size log item"
        );
        let read_update = read_update.unwrap();
        assert_eq!(read_update.header.op_type, LogOperation::Update);
        assert_eq!(read_update.header.table_id, 0);
        assert_eq!(read_update.header.record_id, 1);
        assert_eq!(read_update.header.old_data_size, 4);
        assert_eq!(read_update.header.new_data_size, 4);
        assert_eq!(read_update.old_data, vec![1u8, 2, 3, 4]);
        assert_eq!(read_update.new_data, vec![5u8, 6, 7, 8]);

        println!("✅ 可变大小日志项写入和读取测试通过！");
    }
}

#[test]
fn test_variable_size_log_item_large_record() {
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
            log_path: get_test_wal_path("test_large_record"),
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
            compression_type: WALCompressionType::None,
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

        // 测试：创建超大型记录（超过512字节）
        let mut large_data = Vec::with_capacity(2048);
        for i in 0..2048 {
            large_data.push((i % 256) as u8);
        }

        let mut large_log_item = VariableSizeLogItem {
            header: LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 1,
                old_data_size: 0,
                new_data_size: large_data.len() as u16,
                tx_id: 1,
                timestamp: 1234567890,
                checksum: 0,
            },
            old_data: vec![],
            new_data: large_data,
        };

        let calculated_checksum = unsafe {
            remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(
                &large_log_item,
            )
        };
        large_log_item.header.checksum = calculated_checksum;

        let result = log_manager.write_variable_size_log_item(&large_log_item);
        assert!(result.is_ok(), "Failed to write large record log item");

        // 读取并验证
        let read_large = log_manager.read_variable_size_log_item(0);
        assert!(read_large.is_ok(), "Failed to read large record log item");
        let read_large = read_large.unwrap();
        assert_eq!(read_large.header.new_data_size, 2048);
        assert_eq!(read_large.new_data.len(), 2048);
        for i in 0..2048 {
            assert_eq!(read_large.new_data[i], (i % 256) as u8);
        }

        println!("✅ 超大型记录测试通过！");
    }
}

#[test]
fn test_variable_size_log_item_checksum() {
    // 测试校验和计算
    let new_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut log_item = VariableSizeLogItem {
        header: LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 1,
            old_data_size: 0,
            new_data_size: new_data.len() as u16,
            tx_id: 1,
            timestamp: 1234567890,
            checksum: 0,
        },
        old_data: vec![],
        new_data: new_data.clone(),
    };

    let calculated_checksum = unsafe {
        remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(&log_item)
    };
    log_item.header.checksum = calculated_checksum;

    // 验证校验和一致性
    let recalculated_checksum = unsafe {
        remdb::transaction::Transaction::calculate_variable_size_log_item_checksum(&log_item)
    };
    assert_eq!(log_item.header.checksum, recalculated_checksum);

    println!("✅ 校验和计算测试通过！");
}
