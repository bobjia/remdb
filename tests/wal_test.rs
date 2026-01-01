use remdb::config::{DbConfig, DefaultMemoryAllocator, LogMode, HARole, ReplicationMode};
use remdb::transaction::{LogManager, LogItem, LogOperation};
use remdb::platform::{Platform, FileMode, FileResult, FileHandle, SeekWhence, init_platform};

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
        // 返回一个非空指针作为有效的FileHandle
        Ok(1 as *const u8)
    }
    
    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Ok(())
    }
    
    fn file_write(&self, _handle: FileHandle, _buffer: *const u8, size: usize) -> FileResult<usize> {
        // 模拟写入成功，返回写入的字节数
        Ok(size)
    }
    
    fn file_read(&self, _handle: FileHandle, _buffer: *mut u8, _size: usize) -> FileResult<usize> {
        // 模拟读取成功，返回0表示文件为空
        Ok(0)
    }
    
    fn file_seek(&self, _handle: FileHandle, _offset: i64, _whence: SeekWhence) -> FileResult<u64> {
        // 模拟seek成功，返回当前位置0
        Ok(0)
    }
    
    fn file_remove(&self, _path: &str) -> FileResult<()> {
        Ok(())
    }
    
    fn file_size(&self, _path: &str) -> FileResult<usize> {
        Ok(0)
    }
    
    fn crc32(&self, data: *const u8, size: usize) -> u32 {
        // 简单的CRC32实现，仅用于测试
        let data = unsafe { std::slice::from_raw_parts(data, size) };
        let mut crc = 0xFFFFFFFF;
        
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB88320
                } else {
                    crc >> 1
                };
            }
        }
        
        crc ^ 0xFFFFFFFF
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

// 测试 WAL 功能的测试用例
#[test]
fn test_wal_log_manager_creation() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    // 创建数据库配置
    let config = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: HARole::Auto,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
    };
    
    // 测试创建 LogManager
    unsafe {
        let log_path = "/tmp/test_wal.log";
        let log_manager = LogManager::new(log_path, &config);
        assert!(log_manager.is_ok());
    }
}

#[test]
fn test_wal_log_write_sync_mode() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    // 创建同步模式的数据库配置
    let config = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: HARole::Auto,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
    };
    
    unsafe {
        let log_path = "/tmp/test_wal_sync.log";
        let mut log_manager = LogManager::new(log_path, &config).unwrap();
        
        // 创建测试日志项
        let mut new_data = [0u8; 512];
        new_data[0..8].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);
        
        let log_item = LogItem {
            op_type: LogOperation::Insert,
            table_id: 0,
            record_id: 1,
            data_size: 8,
            old_data: [0u8; 512],
            new_data,
            tx_id: 1,
            timestamp: 1234567890,
            checksum: 0, // 会在写入时计算
        };
        
        // 写入日志项（同步模式）
        let result = log_manager.write_log_item(&log_item);
        assert!(result.is_ok());
    }
}

#[test]
fn test_wal_log_write_async_mode() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    // 创建异步模式的数据库配置
    let config = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: HARole::Auto,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
    };
    
    unsafe {
        let log_path = "/tmp/test_wal_async.log";
        let mut log_manager = LogManager::new(log_path, &config).unwrap();
        
        // 创建测试日志项
        let mut old_data = [0u8; 512];
        old_data[0..8].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7]);
        
        let mut new_data = [0u8; 512];
        new_data[0..8].copy_from_slice(&[7, 6, 5, 4, 3, 2, 1, 0]);
        
        let log_item = LogItem {
            op_type: LogOperation::Update,
            table_id: 0,
            record_id: 1,
            data_size: 8,
            old_data,
            new_data,
            tx_id: 1,
            timestamp: 1234567890,
            checksum: 0, // 会在写入时计算
        };
        
        // 写入日志项（异步模式，应该进入缓冲区）
        let result = log_manager.write_log_item(&log_item);
        assert!(result.is_ok());
        
        // 手动刷新缓冲区
        let result = log_manager.flush_buffer();
        assert!(result.is_ok());
    }
}

#[test]
fn test_wal_checkpoint_mechanism() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    // 创建数据库配置，使用短检查点间隔
    let config = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 100, // 100毫秒检查点间隔
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: HARole::Auto,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
    };
    
    unsafe {
        let log_path = "/tmp/test_wal_checkpoint.log";
        let mut log_manager = LogManager::new(log_path, &config).unwrap();
        
        // 模拟检查点触发
        let result = log_manager.check_flush_and_checkpoint();
        assert!(result.is_ok());
        
        // 写入一些日志项
        for i in 0..5 {
            let mut new_data = [0u8; 512];
            new_data[0] = i as u8;
            
            let log_item = LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: i as u16,
                data_size: 8,
                old_data: [0u8; 512],
                new_data,
                tx_id: 1,
                timestamp: 1234567890 + i,
                checksum: 0,
            };
            
            log_manager.write_log_item(&log_item).unwrap();
        }
        
        // 再次检查检查点
        let result = log_manager.check_flush_and_checkpoint();
        assert!(result.is_ok());
    }
}

#[test]
fn test_wal_log_preallocation() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建内存分配器
    static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
    
    // 创建带有大预分配大小的配置
    let config = DbConfig {
        tables: &[],
        total_memory: 1024 * 1024, // 1MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 1000,
        memory_allocator: &ALLOCATOR,
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 512 * 1024, // 512KB 预分配
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: HARole::Auto,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
    };
    
    unsafe {
        let log_path = "/tmp/test_wal_prealloc.log";
        let log_manager = LogManager::new(log_path, &config);
        assert!(log_manager.is_ok());
        
        // 这里可以添加文件大小检查，但需要平台特定的API
        // 暂时只测试创建成功
    }
}

#[test]
fn test_wal_different_log_modes() {
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 测试不同日志模式的行为差异
    // 使用静态字符串作为日志路径
    static LOG_PATH_SYNC: &str = "/tmp/test_wal_mode_sync.log";
    static LOG_PATH_ASYNC: &str = "/tmp/test_wal_mode_async.log";
    
    let modes = [(LogMode::Sync, LOG_PATH_SYNC), (LogMode::Async, LOG_PATH_ASYNC)];
    
    for (mode, log_path) in modes {
        // 创建内存分配器
        static ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
        
        let config = DbConfig {
            tables: &[],
            total_memory: 1024 * 1024, // 1MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 1000,
            memory_allocator: &ALLOCATOR,
            log_mode: mode,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            ha_role: HARole::Auto,
            replication_mode: ReplicationMode::Async,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
        };
        
        unsafe {
            let mut log_manager = LogManager::new(log_path, &config).unwrap();
            
            // 写入测试日志项
            let mut new_data = [0u8; 512];
            new_data[0..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
            
            let log_item = LogItem {
                op_type: LogOperation::Insert,
                table_id: 0,
                record_id: 1,
                data_size: 8,
                old_data: [0u8; 512],
                new_data,
                tx_id: 1,
                timestamp: 1234567890,
                checksum: 0,
            };
            
            let result = log_manager.write_log_item(&log_item);
            assert!(result.is_ok());
            
            // 对于异步模式，手动刷新
            if mode == LogMode::Async {
                let result = log_manager.flush_buffer();
                assert!(result.is_ok());
            }
        }
    }
}
