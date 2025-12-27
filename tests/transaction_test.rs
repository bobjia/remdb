use core::mem::MaybeUninit; use std::ptr::NonNull; extern crate alloc; use alloc::sync::Arc;
use remdb::*;
use remdb::types::*;
use remdb::transaction::*;
use remdb::platform::*;
use serial_test::serial;

// 测试用Platform实现
struct TestPlatform;

impl Platform for TestPlatform {
    fn get_timestamp(&self) -> u64 {
        0
    }
    
    fn get_timestamp_us(&self) -> u64 {
        0
    }
    
    fn spin_lock(&self, lock: &mut u32) {
        // 简单的自旋锁实现
        unsafe {
            while core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .compare_exchange(0, 1, 
                                 core::sync::atomic::Ordering::Acquire,
                                 core::sync::atomic::Ordering::Relaxed)
                .is_err() {
                core::hint::spin_loop();
            }
        }
    }
    
    fn spin_unlock(&self, lock: &mut u32) {
        unsafe {
            core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .store(0, core::sync::atomic::Ordering::Release);
        }
    }
    
    fn compiler_barrier(&self) {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
    
    fn full_memory_barrier(&self) {
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }
    
    fn memcpy(&self, dest: *mut u8, src: *const u8, size: usize) {
        unsafe {
            core::ptr::copy_nonoverlapping(src, dest, size);
        }
    }
    
    fn memset(&self, dest: *mut u8, value: u8, size: usize) {
        unsafe {
            core::ptr::write_bytes(dest, value, size);
        }
    }
    
    fn delay_ms(&self, _ms: u32) {
        // 空实现
    }
    
    fn delay_us(&self, _us: u32) {
        // 空实现
    }
    
    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> {
        Ok(core::ptr::null())
    }
    
    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Ok(())
    }
    
    fn file_write(&self, _handle: FileHandle, _buffer: *const u8, _size: usize) -> FileResult<usize> {
        Ok(0)
    }
    
    fn file_read(&self, _handle: FileHandle, _buffer: *mut u8, _size: usize) -> FileResult<usize> {
        Ok(0)
    }
    
    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
        Ok(0)
    }
    
    fn file_remove(&self, _path: &str) -> FileResult<()> {
        Ok(())
    }
    
    fn file_size(&self, _path: &str) -> FileResult<usize> {
        Ok(0)
    }
    
    fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
        0
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 简单的表定义用于测试
static TEST_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "test_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::UInt32,
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
    secondary_index_type: IndexType::SortedArray,
    record_size: 8,
    max_records: 100,
};

// 数据库配置
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[TEST_TABLE_DEF],
    total_memory: 1024 * 1024, // 1MB
    low_power_mode_supported: false,
    low_power_max_records: None,
    memory_allocator: unsafe {
        static mut DEFAULT_ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;
        &mut DEFAULT_ALLOCATOR
    },
};

// 静态缓冲区用于测试
static mut TABLES_BUFFER: [Option<MemoryTable>; 1] = [None];
static mut PRIMARY_INDICES_BUFFER: [Option<PrimaryIndex>; 1] = [None];
static mut SECONDARY_INDICES_BUFFER: [Option<AnySecondaryIndex>; 1] = [None];
static mut TABLE_DATA_BUFFER: [u8; 8 * 100] = [0u8; 8 * 100]; // 8字节记录 * 100条
static mut TABLE_STATUS_BUFFER: [MaybeUninit<RecordHeader>; 100] = [const { MaybeUninit::uninit() }; 100];
static mut TABLE_FREE_SLOTS_BUFFER: [usize; 100] = [0usize; 100];

#[test]
#[serial]
fn test_transaction_begin_commit() {
    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);
        
        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(1024 * 1024); // 1MB
        memory_buffer.set_len(1024 * 1024);
        remdb::memory::allocator::init_global_allocator(
            memory_buffer.as_mut_ptr(), 
            1024 * 1024
        ).unwrap();
        
        // 重置全局数据库实例和事务管理器
        remdb::reset_global_db();
        TX_MANAGER.reset();
        
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
                lock_count: 0
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
#[serial]
fn test_transaction_rollback() {
    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);
        
        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(1024 * 1024); // 1MB
        memory_buffer.set_len(1024 * 1024);
        remdb::memory::allocator::init_global_allocator(
            memory_buffer.as_mut_ptr(), 
            1024 * 1024
        ).unwrap();
        
        // 重置事务管理器
        TX_MANAGER.reset();
        
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
                lock_count: 0
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
        let _record_id = table_mut.insert(record_data.as_ptr()).unwrap();
        
        // 回滚事务
        db.rollback_transaction().unwrap();
        
        // 验证记录已回滚
        let table = db.get_table(0).unwrap();
        assert_eq!(table.record_count(), 0);
    }
}

#[test]
#[serial]
fn test_transaction_update_rollback() {
    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);
        
        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(1024 * 1024); // 1MB
        memory_buffer.set_len(1024 * 1024);
        remdb::memory::allocator::init_global_allocator(
            memory_buffer.as_mut_ptr(), 
            1024 * 1024
        ).unwrap();
        
        // 重置事务管理器
        TX_MANAGER.reset();
        
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
                lock_count: 0
            });
        }
        
        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();
        
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
        
        let record_id = db.get_table_mut(0).unwrap().insert(record_data.as_ptr()).unwrap();
        
        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        
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
#[serial]
fn test_transaction_delete_rollback() {
    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);
        
        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(1024 * 1024); // 1MB
        memory_buffer.set_len(1024 * 1024);
        remdb::memory::allocator::init_global_allocator(
            memory_buffer.as_mut_ptr(), 
            1024 * 1024
        ).unwrap();
        
        // 重置事务管理器
        TX_MANAGER.reset();
        
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
                lock_count: 0
            });
        }
        
        // 创建数据库实例
        let db = init_global_db(&TEST_DB_CONFIG).unwrap();
        
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
        
        let record_id = db.get_table_mut(0).unwrap().insert(record_data.as_ptr()).unwrap();
        
        // 事务缓冲区
        #[allow(invalid_value)]
        let mut tx_buffer = core::mem::MaybeUninit::<Transaction>::uninit().assume_init();
        
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
