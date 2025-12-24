use remdb::*;
use remdb::types::*;
use remdb::transaction::*;

// 简单的表定义用于测试
static TEST_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "test_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::Int32,
            size: 4,
            offset: 0,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float32,
            size: 4,
            offset: 4,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    record_size: 8,
    max_records: 100,
};

// 数据库配置
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[&TEST_TABLE_DEF],
};

#[test]
fn test_transaction_begin_commit() {
    // 分配内存缓冲区
    let mut tables_buffer = [None::<MemoryTable>; 1];
    let mut primary_indices_buffer = [None::<PrimaryIndex>; 1];
    let mut secondary_indices_buffer = [None::<SecondaryIndex>; 1];
    let mut table_data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
    let mut table_status_buffer = [RecordHeader {
        status: RecordStatus::Free,
        version: 0,
        lock_type: LockType::None,
        lock_owner: 0,
        lock_count: 0
    }; 100];
    let mut table_free_slots_buffer = [0usize; 100];
    
    unsafe {
        // 创建数据库实例
        let mut db = RemDb::new(
            &TEST_DB_CONFIG,
            &mut tables_buffer,
            &mut primary_indices_buffer,
            &mut secondary_indices_buffer
        );
        
        // 创建表
        let table = MemoryTable::new(
            &TEST_TABLE_DEF,
            table_data_buffer.as_mut_ptr(),
            table_status_buffer.as_mut_ptr(),
            table_free_slots_buffer.as_mut_ptr()
        );
        tables_buffer[0] = Some(table);
        
        // 事务缓冲区
        let mut tx_buffer = Transaction {
            id: 0,
            tx_type: TransactionType::ReadWrite,
            status: TransactionStatus::Active,
            isolation_level: IsolationLevel::ReadCommitted,
            start_time: 0,
            log_items: core::ptr::NonNull::dangling(),
            max_log_items: 0,
            log_item_count: 0,
            depth: 1,
            lock: 0,
        };
        
        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];
        
        // 开始事务
        let tx = db.begin_transaction(
            TransactionType::ReadWrite,
            IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).unwrap();
        
        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        // 插入记录
        let mut table_mut = db.get_table_mut(0).unwrap();
        let record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        
        // 提交事务
        db.commit_transaction().unwrap();
        
        // 验证记录已插入
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 1);
    }
}

#[test]
fn test_transaction_rollback() {
    // 分配内存缓冲区
    let mut tables_buffer = [None::<MemoryTable>; 1];
    let mut primary_indices_buffer = [None::<PrimaryIndex>; 1];
    let mut secondary_indices_buffer = [None::<SecondaryIndex>; 1];
    let mut table_data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
    let mut table_status_buffer = [RecordHeader {
        status: RecordStatus::Free,
        version: 0,
        lock_type: LockType::None,
        lock_owner: 0,
        lock_count: 0
    }; 100];
    let mut table_free_slots_buffer = [0usize; 100];
    
    unsafe {
        // 创建数据库实例
        let mut db = RemDb::new(
            &TEST_DB_CONFIG,
            &mut tables_buffer,
            &mut primary_indices_buffer,
            &mut secondary_indices_buffer
        );
        
        // 创建表
        let table = MemoryTable::new(
            &TEST_TABLE_DEF,
            table_data_buffer.as_mut_ptr(),
            table_status_buffer.as_mut_ptr(),
            table_free_slots_buffer.as_mut_ptr()
        );
        tables_buffer[0] = Some(table);
        
        // 事务缓冲区
        let mut tx_buffer = Transaction {
            id: 0,
            tx_type: TransactionType::ReadWrite,
            status: TransactionStatus::Active,
            isolation_level: IsolationLevel::ReadCommitted,
            start_time: 0,
            log_items: core::ptr::NonNull::dangling(),
            max_log_items: 0,
            log_item_count: 0,
            depth: 1,
            lock: 0,
        };
        
        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];
        
        // 开始事务
        let tx = db.begin_transaction(
            TransactionType::ReadWrite,
            IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).unwrap();
        
        // 创建测试记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        // 插入记录
        let mut table_mut = db.get_table_mut(0).unwrap();
        let record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        
        // 回滚事务
        db.rollback_transaction().unwrap();
        
        // 验证记录已回滚
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 0);
    }
}

#[test]
fn test_transaction_update_rollback() {
    // 分配内存缓冲区
    let mut tables_buffer = [None::<MemoryTable>; 1];
    let mut primary_indices_buffer = [None::<PrimaryIndex>; 1];
    let mut secondary_indices_buffer = [None::<SecondaryIndex>; 1];
    let mut table_data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
    let mut table_status_buffer = [RecordHeader {
        status: RecordStatus::Free,
        version: 0,
        lock_type: LockType::None,
        lock_owner: 0,
        lock_count: 0
    }; 100];
    let mut table_free_slots_buffer = [0usize; 100];
    
    unsafe {
        // 创建数据库实例
        let mut db = RemDb::new(
            &TEST_DB_CONFIG,
            &mut tables_buffer,
            &mut primary_indices_buffer,
            &mut secondary_indices_buffer
        );
        
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            table_data_buffer.as_mut_ptr(),
            table_status_buffer.as_mut_ptr(),
            table_free_slots_buffer.as_mut_ptr()
        );
        
        // 预插入一条记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        let record_id = table.insert(record_data.as_ptr()).unwrap();
        tables_buffer[0] = Some(table);
        
        // 事务缓冲区
        let mut tx_buffer = Transaction {
            id: 0,
            tx_type: TransactionType::ReadWrite,
            status: TransactionStatus::Active,
            isolation_level: IsolationLevel::ReadCommitted,
            start_time: 0,
            log_items: core::ptr::NonNull::dangling(),
            max_log_items: 0,
            log_item_count: 0,
            depth: 1,
            lock: 0,
        };
        
        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];
        
        // 开始事务
        let tx = db.begin_transaction(
            TransactionType::ReadWrite,
            IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).unwrap();
        
        // 更新记录
        let mut update_data = [0u8; 8];
        let new_id: i32 = 1;
        let new_value: f32 = 6.28;
        
        core::ptr::copy_nonoverlapping(
            &new_id as *const i32 as *const u8,
            update_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &new_value as *const f32 as *const u8,
            update_data.as_mut_ptr().add(4),
            4
        );
        
        let mut table_mut = db.get_table_mut(0).unwrap();
        table_mut.update(record_id, update_data.as_ptr()).unwrap();
        
        // 回滚事务
        db.rollback_transaction().unwrap();
        
        // 验证记录已回滚到原始值
        let table = db.get_table(0).unwrap();
        let mut result_data = [0u8; 8];
        table.get_by_id(record_id, result_data.as_mut_ptr()).unwrap();
        
        let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
        let result_value = core::ptr::read(result_data.as_ptr().add(4) as *const f32);
        
        assert_eq!(result_id, id);
        assert_eq!(result_value, value);
    }
}

#[test]
fn test_transaction_delete_rollback() {
    // 分配内存缓冲区
    let mut tables_buffer = [None::<MemoryTable>; 1];
    let mut primary_indices_buffer = [None::<PrimaryIndex>; 1];
    let mut secondary_indices_buffer = [None::<SecondaryIndex>; 1];
    let mut table_data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
    let mut table_status_buffer = [RecordHeader {
        status: RecordStatus::Free,
        version: 0,
        lock_type: LockType::None,
        lock_owner: 0,
        lock_count: 0
    }; 100];
    let mut table_free_slots_buffer = [0usize; 100];
    
    unsafe {
        // 创建数据库实例
        let mut db = RemDb::new(
            &TEST_DB_CONFIG,
            &mut tables_buffer,
            &mut primary_indices_buffer,
            &mut secondary_indices_buffer
        );
        
        // 创建表
        let mut table = MemoryTable::new(
            &TEST_TABLE_DEF,
            table_data_buffer.as_mut_ptr(),
            table_status_buffer.as_mut_ptr(),
            table_free_slots_buffer.as_mut_ptr()
        );
        
        // 预插入一条记录
        let mut record_data = [0u8; 8];
        let id: i32 = 1;
        let value: f32 = 3.14;
        
        core::ptr::copy_nonoverlapping(
            &id as *const i32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        core::ptr::copy_nonoverlapping(
            &value as *const f32 as *const u8,
            record_data.as_mut_ptr().add(4),
            4
        );
        
        let record_id = table.insert(record_data.as_ptr()).unwrap();
        tables_buffer[0] = Some(table);
        
        // 事务缓冲区
        let mut tx_buffer = Transaction {
            id: 0,
            tx_type: TransactionType::ReadWrite,
            status: TransactionStatus::Active,
            isolation_level: IsolationLevel::ReadCommitted,
            start_time: 0,
            log_items: core::ptr::NonNull::dangling(),
            max_log_items: 0,
            log_item_count: 0,
            depth: 1,
            lock: 0,
        };
        
        let mut log_buffer = [LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];
        
        // 开始事务
        let tx = db.begin_transaction(
            TransactionType::ReadWrite,
            IsolationLevel::ReadCommitted,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).unwrap();
        
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
    }
}
