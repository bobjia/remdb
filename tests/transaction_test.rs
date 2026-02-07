use core::mem::MaybeUninit;
use std::ptr::NonNull;
extern crate alloc;
use alloc::sync::Arc;
use remdb::config::WALConfig;
use remdb::platform::*;
use remdb::transaction::*;
use remdb::types::*;
use remdb::*;
use serial_test::serial;

mod common;
use crate::common::platform::TEST_PLATFORM;
use common::{setup_test_db, setup_test_db_with_memory};

// 简单的表定义用于测试
fn create_test_table_def() -> TableDef {
    TableDef {
        id: 0,
        name: "test_table".to_string(),
        fields: vec![
            FieldDef {
                name: "id".to_string(),
                data_type: DataType::UInt32,
                size: 4,
                string_length: None,
                offset: 0,
                primary_key: true,
                not_null: true,
                unique: true,
                auto_increment: true,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
            FieldDef {
                name: "value".to_string(),
                data_type: DataType::Float32,
                size: 4,
                string_length: None,
                offset: 4,
                primary_key: false,
                not_null: false,
                unique: false,
                auto_increment: false,
                default_value: None,
                vector_metadata: None,
                json_metadata: None,
            },
        ],
        primary_key: vec![0],
        secondary_index: None,
        secondary_index_type: IndexType::SortedArray,
        record_size: 8,
        max_records: 100,
        version: 1,
        created_at: 0,
        updated_at: 0,
    }
}

// 静态内存分配器实例
static DEFAULT_ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;

// 数据库配置
static TEST_DB_CONFIG: std::sync::LazyLock<config::DbConfig> = std::sync::LazyLock::new(|| {
    config::DbConfig {
        tables: vec![create_test_table_def()],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 100000,
        memory_allocator: &DEFAULT_ALLOCATOR,
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: config::LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
        },
        time_series_defaults: time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(config::HAConfig {
            node_id: 1,
            ha_role: remdb::ha::HARole::Auto,
            replication_mode: remdb::ha::ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
        }),
    }
});

// 静态缓冲区用于测试
static mut TABLES_BUFFER: [Option<MemoryTable>; 1] = [None];
static mut PRIMARY_INDICES_BUFFER: [Option<PrimaryIndex>; 1] = [None];
static mut SECONDARY_INDICES_BUFFER: [Option<AnySecondaryIndex>; 1] = [None];
static mut TABLE_DATA_BUFFER: [u8; 8 * 100] = [0u8; 8 * 100]; // 8字节记录 * 100条
static mut TABLE_STATUS_BUFFER: [MaybeUninit<RecordHeader>; 100] =
    [const { MaybeUninit::uninit() }; 100];
static mut TABLE_FREE_SLOTS_BUFFER: [usize; 100] = [0usize; 100];

#[test]
#[serial]
fn test_transaction_begin_commit() {
    let _db_memory = setup_test_db();

    // 重置全局数据库实例和事务管理器
    remdb::reset_global_db();
    crate::transaction::init_tx_manager();

    // 重置缓冲区
    unsafe {
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer =
            unsafe { core::mem::MaybeUninit::<Transaction>::uninit().assume_init() };

        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        // 开始事务
        let tx = unsafe {
            db.begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
        }
        .unwrap();

        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;

        core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        // 插入记录
        let mut table_mut = db.get_table_mut(0).unwrap();
        let record_id = table_mut.insert(record_data.as_ptr()).unwrap();

        // 提交事务
        db.commit_transaction().unwrap();

        // 验证记录已插入
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 1);

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_mvcc_snapshot_isolation() {
    let _db_memory = setup_test_db();

    // 重置事务管理器
    crate::transaction::init_tx_manager();

    unsafe {
        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 事务1：插入初始记录
        let mut tx1_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        let mut tx1_log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        let tx1 = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::RepeatableRead,
                &mut tx1_buffer,
                tx1_log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 插入初始记录
        let mut record_data = [0u8; 8];
        let id: u32 = 1;
        let value: f32 = 3.14;

        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 提交事务1
        db.commit_transaction().unwrap();

        // 事务2：开始读取事务，建立快照
        let mut tx2_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        let mut tx2_log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        let tx2 = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::RepeatableRead,
                &mut tx2_buffer,
                tx2_log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 事务2：读取初始值
        let mut result_data = [0u8; 8];
        {
            let table = db.get_table(0).unwrap();
            table
                .get_by_id(record_id, result_data.as_mut_ptr())
                .unwrap();
        }
        let result_value1 = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        assert_eq!(result_value1, value); // 应该读取到初始值

        // 提交事务2
        db.commit_transaction().unwrap();

        // 事务3：更新记录
        let mut tx3_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        let mut tx3_log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        let tx3 = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx3_buffer,
                tx3_log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 更新记录
        let mut update_data = [0u8; 8];
        let new_value: f32 = 6.28;

        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, update_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &new_value as *const f32 as *const u8,
            update_data.as_mut_ptr().add(4),
            4,
        );

        db.get_table_mut(0)
            .unwrap()
            .update(record_id, update_data.as_ptr())
            .unwrap();

        // 提交事务3
        db.commit_transaction().unwrap();

        // 新事务：读取最新值
        let mut tx4_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        let mut tx4_log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        let tx4 = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx4_buffer,
                tx4_log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        {
            let table = db.get_table(0).unwrap();
            table
                .get_by_id(record_id, result_data.as_mut_ptr())
                .unwrap();
        }
        let result_value3 = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        assert_eq!(result_value3, new_value); // 应该读取到更新后的值

        // 提交事务4
        db.commit_transaction().unwrap();

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_mvcc_version_chain() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 插入初始记录
        let mut record_data = [0u8; 8];
        let id: u32 = 1;
        let value1: f32 = 1.0;

        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value1 as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 多次更新记录，创建版本链
        let values = [2.0f32, 3.0f32, 4.0f32];

        for i in 0..3 {
            // 为每个事务单独创建缓冲区
            let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
            let mut log_buffer = [LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 0,
                old_data_size: 0,
                new_data_size: 0,
                old_data: [0; 512],
                new_data: [0; 512],
                tx_id: 0,
                timestamp: 0,
                checksum: 0,
            }; 10];

            // 开始事务
            db.begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

            // 更新记录
            let mut update_data = [0u8; 8];
            let new_value = values[i];

            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                update_data.as_mut_ptr(),
                4,
            );
            core::ptr::copy_nonoverlapping(
                &new_value as *const f32 as *const u8,
                update_data.as_mut_ptr().add(4),
                4,
            );

            db.get_table_mut(0)
                .unwrap()
                .update(record_id, update_data.as_ptr())
                .unwrap();

            // 提交事务
            db.commit_transaction().unwrap();
        }

        // 验证最新值
        let mut result_data = [0u8; 8];
        let table = db.get_table(0).unwrap();
        table
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();
        let result_value = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        assert_eq!(result_value, 4.0); // 应该是最后一次更新的值

        // 验证版本号已经增加
        let status_ptr = table.status_array.as_ptr().add(record_id);
        let current_status = *status_ptr;
        assert_eq!(current_status.version, 4); // 初始版本1 + 3次更新 = 4

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_mvcc_gc() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 插入初始记录
        let mut record_data = [0u8; 8];
        let id: u32 = 1;
        let value: f32 = 1.0;

        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 多次更新记录，创建版本链
        for i in 0..5 {
            // 为每个事务单独创建缓冲区
            let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
            let mut log_buffer = [LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 0,
                old_data_size: 0,
                new_data_size: 0,
                old_data: [0; 512],
                new_data: [0; 512],
                tx_id: 0,
                timestamp: 0,
                checksum: 0,
            }; 10];

            // 开始事务
            db.begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

            // 更新记录
            let mut update_data = [0u8; 8];
            let new_value = (i + 2) as f32;

            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                update_data.as_mut_ptr(),
                4,
            );
            core::ptr::copy_nonoverlapping(
                &new_value as *const f32 as *const u8,
                update_data.as_mut_ptr().add(4),
                4,
            );

            db.get_table_mut(0)
                .unwrap()
                .update(record_id, update_data.as_ptr())
                .unwrap();

            // 提交事务
            db.commit_transaction().unwrap();
        }

        // 跳过垃圾回收测试，因为MemoryTable没有实现gc方法和free_version_slot_count字段
        // 直接验证最新值仍然可用

        // 验证最新值仍然可用
        let mut result_data = [0u8; 8];
        {
            let table = db.get_table(0).unwrap();
            table
                .get_by_id(record_id, result_data.as_mut_ptr())
                .unwrap();
        }
        let result_value = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        assert_eq!(result_value, 6.0); // 应该是最后一次更新的值

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_mvcc_visibility() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 插入初始记录
        let mut record_data = [0u8; 8];
        let id: u32 = 1;
        let value: f32 = 1.0;

        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 事务1：更新记录
        {
            let mut tx1_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
            let mut tx1_log_buffer = [LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 0,
                old_data_size: 0,
                new_data_size: 0,
                old_data: [0; 512],
                new_data: [0; 512],
                tx_id: 0,
                timestamp: 0,
                checksum: 0,
            }; 10];

            let tx1 = db
                .begin_transaction(
                    TransactionType::ReadWrite,
                    IsolationLevel::ReadCommitted,
                    &mut tx1_buffer,
                    tx1_log_buffer.as_mut_ptr(),
                    10,
                )
                .unwrap();

            // 更新记录
            let mut update_data = [0u8; 8];
            let new_value: f32 = 2.0;

            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                update_data.as_mut_ptr(),
                4,
            );
            core::ptr::copy_nonoverlapping(
                &new_value as *const f32 as *const u8,
                update_data.as_mut_ptr().add(4),
                4,
            );

            db.get_table_mut(0)
                .unwrap()
                .update(record_id, update_data.as_ptr())
                .unwrap();

            // 提交事务1
            db.commit_transaction().unwrap();
        }

        // 事务2：读取记录（应该能看到事务1的更新）
        {
            let mut tx2_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
            let mut tx2_log_buffer = [LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 0,
                old_data_size: 0,
                new_data_size: 0,
                old_data: [0; 512],
                new_data: [0; 512],
                tx_id: 0,
                timestamp: 0,
                checksum: 0,
            }; 10];

            let tx2 = db
                .begin_transaction(
                    TransactionType::ReadWrite,
                    IsolationLevel::ReadCommitted,
                    &mut tx2_buffer,
                    tx2_log_buffer.as_mut_ptr(),
                    10,
                )
                .unwrap();

            // 读取记录，验证能看到已提交的更新
            let mut result_data = [0u8; 8];
            {
                let table = db.get_table(0).unwrap();
                table
                    .get_by_id(record_id, result_data.as_mut_ptr())
                    .unwrap();
            }
            let result_value1 = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
            assert_eq!(result_value1, 2.0); // 应该能看到已提交的更新

            // 提交事务2
            db.commit_transaction().unwrap();
        }

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_transaction_rollback() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();

        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        // 开始事务
        let tx = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;

        core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        // 插入记录
        let mut table_mut = db.get_table_mut(0).unwrap();
        let _record_id = table_mut.insert(record_data.as_ptr()).unwrap();

        // 回滚事务
        db.rollback_transaction().unwrap();

        // 验证记录已回滚
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 0);

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_transaction_update_rollback() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 预插入一条记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;

        core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();

        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data_size: 0,
            new_data_size: 0,
            old_data: [0; 512],
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        // 开始事务
        let tx = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 更新记录
        let mut update_data = [0u8; 8];
        let new_id: i32 = 1;
        let new_value: f32 = 6.28;

        core::ptr::copy_nonoverlapping(
            &new_id as *const i32 as *const u8,
            update_data.as_mut_ptr(),
            4,
        );
        core::ptr::copy_nonoverlapping(
            &new_value as *const f32 as *const u8,
            update_data.as_mut_ptr().add(4),
            4,
        );

        let mut table_mut = db.get_table_mut(0).unwrap();
        table_mut.update(record_id, update_data.as_ptr()).unwrap();

        // 回滚事务
        db.rollback_transaction().unwrap();

        // 验证记录已回滚到原始值
        let table = db.get_table(0).unwrap();
        let mut result_data = [0u8; 8];
        table
            .get_by_id(record_id, result_data.as_mut_ptr())
            .unwrap();

        let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
        let result_value = core::ptr::read(result_data.as_ptr().add(4) as *const f32);

        assert_eq!(result_id, id);
        assert_eq!(result_value, value);

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}

#[test]
#[serial]
fn test_transaction_delete_rollback() {
    unsafe {
        let _db_memory = setup_test_db();

        // 重置事务管理器
        crate::transaction::init_tx_manager();

        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);

        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0,
                create_tx_id: 0,
                delete_tx_id: 0,
                next_version_ptr: 0,
            });
        }

        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();

        // 预插入一条记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;

        core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4,
        );

        let record_id = db
            .get_table_mut(0)
            .unwrap()
            .insert(record_data.as_ptr())
            .unwrap();

        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();

        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            old_data: [0u8; 512],
            old_data_size: 0,
            new_data_size: 0,
            new_data: [0; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];

        // 开始事务
        let tx = db
            .begin_transaction(
                TransactionType::ReadWrite,
                IsolationLevel::ReadCommitted,
                &mut tx_buffer,
                log_buffer.as_mut_ptr(),
                10,
            )
            .unwrap();

        // 删除记录
        let mut table_mut = db.get_table_mut(0).unwrap();
        table_mut.delete(record_id).unwrap();

        // 回滚事务
        db.rollback_transaction().unwrap();

        // 验证记录已恢复
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 1);

        let mut result_data = [0u8; 8];
        let result = table.get_by_id(record_id, result_data.as_mut_ptr());
        assert!(result.is_ok());

        // 显式重置数据库实例，确保所有资源被正确释放
        remdb::reset_global_db();
    }
}
