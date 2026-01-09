use remdb::table::*;
use remdb::types::*;
use remdb::platform::*;
use remdb::{init_global_db, PrimaryIndex, AnySecondaryIndex}; use remdb::config::DefaultMemoryAllocator;
use remdb::config::DbConfig;
use std::time::Instant;
use rand::random;

// 定义一个用于性能测试的大表，max_records设置为500,000
static LARGE_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "large_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float32,
            size: 4,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
        FieldDef {
            name: "name",
            data_type: DataType::String,
            size: 32,
            offset: 8,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 4 + 4 + 32, // 40字节记录
    max_records: 500000,
};

// 简单的测试平台实现
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
        while unsafe {
            core::sync::atomic::AtomicU32::from_ptr(lock as *mut u32)
                .compare_exchange(0, 1, 
                                 core::sync::atomic::Ordering::Acquire,
                                 core::sync::atomic::Ordering::Relaxed)
                .is_err()
        } {
            core::hint::spin_loop();
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
    
    fn delay_ms(&self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    
    fn delay_us(&self, us: u32) {
        std::thread::sleep(std::time::Duration::from_micros(us as u64));
    }
    
    fn file_open(&self, _path: &str, _mode: FileMode) -> FileResult<FileHandle> {
        Ok(core::ptr::null())
    }
    
    fn file_close(&self, _handle: FileHandle) -> FileResult<()> {
        Ok(())
    }
    
    fn file_write(&self, _handle: FileHandle, _buffer: *const u8, size: usize) -> FileResult<usize> {
        Ok(size)
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

#[test]
fn test_large_table_performance() {
    println!("=== 大表性能测试开始 ===");
    println!("表定义: {}", LARGE_TABLE_DEF.name);
    println!("记录大小: {} 字节", LARGE_TABLE_DEF.record_size);
    println!("最大记录数: {}", LARGE_TABLE_DEF.max_records);
    println!("目标测试记录数: {} (80%容量)", 80000);
    
    // 初始化平台
    unsafe {
        init_platform(&TEST_PLATFORM);
    }
    
    // 创建数据库配置
    static DB_CONFIG: DbConfig = DbConfig {
        tables: &[LARGE_TABLE_DEF],
        total_memory: 500_000_000, // 500MB
        low_power_mode_supported: false,
        low_power_max_records: Some(10000),
        default_max_records: 100000,
        memory_allocator: unsafe {
            static mut DEFAULT_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
            &mut DEFAULT_ALLOCATOR
        },
        log_path: "large_table_test.wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
        ha_role: remdb::config::HARole::Auto,
        replication_mode: remdb::config::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
        replication_port: 5556,
        heartbeat_port: 5557,
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
    };
    
    unsafe {
        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(DB_CONFIG.total_memory);
        memory_buffer.set_len(DB_CONFIG.total_memory);
        remdb::memory::allocator::init_global_allocator(
            memory_buffer.as_mut_ptr(), 
            DB_CONFIG.total_memory
        ).unwrap();
        
        // 初始化数据库实例
        let db = init_global_db(&DB_CONFIG).unwrap();
        
        // 输出初始监控指标
        println!("\n初始监控指标:");
        println!("{}", db.dump_metrics());
        
        // 1. 插入80,000条记录（达到80%容量）
        println!("\n1. 插入80,000条记录...");
        let start_time = Instant::now();
        
        // 获取表引用
        let table = db.get_table_mut(0).unwrap();
        let mut inserted_ids = Vec::with_capacity(80000);
        for i in 0..80000 {
            let mut record_data = [0u8; 40]; // 40字节记录
            
            // 设置ID字段（UInt32）
            let id: u32 = (i + 1) as u32;
            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                record_data.as_mut_ptr(),
                4
            );
            
            // 设置value字段（Float32）
            let value: f32 = (i as f32) * 1.5;
            core::ptr::copy_nonoverlapping(
                &value as *const f32 as *const u8,
                record_data.as_mut_ptr().add(4),
                4
            );
            
            // 设置name字段（String，32字节）
            let name_str = format!("record_{:08}", i);
            let name_bytes = name_str.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_data.as_mut_ptr().add(8),
                name_bytes.len()
            );
            
            // 插入记录
            let result = table.insert(record_data.as_ptr());
            assert!(result.is_ok(), "插入记录 {} 失败: {:?}", i, result);
            inserted_ids.push(result.unwrap());
        }
        
        // 先获取当前记录数，然后释放表引用
        let record_count = table.record_count();
        drop(table); // 释放表的可变引用
        
        let insert_duration = start_time.elapsed();
        println!("插入完成，耗时: {:?}", insert_duration);
        println!("插入速率: {:.2} 条/秒", 80000 as f64 / insert_duration.as_secs_f64());
        println!("当前记录数: {}", record_count);
        
        // 输出监控指标
        println!("\n插入80,000条记录后的监控指标:");
        println!("{}", db.dump_metrics());
        
        // 2. 测试查询性能（查询10,000条随机记录）
        println!("\n2. 查询性能测试（10,000条随机记录）...");
        let start_time = Instant::now();
        
        // 获取表引用
        let table = db.get_table_mut(0).unwrap();
        
        let mut query_success = 0;
        for _ in 0..10000 {
            // 生成随机记录ID（0-79999之间）
            let random_index = (random::<u32>() % 80000) as usize;
            let record_id = inserted_ids[random_index];
            
            // 读取记录数据
            let mut result_data = [0u8; 40];
            let get_result = table.get_by_id(record_id, result_data.as_mut_ptr());
            if get_result.is_ok() {
                query_success += 1;
            }
        }
        
        let query_duration = start_time.elapsed();
        println!("查询完成，耗时: {:?}", query_duration);
        println!("查询速率: {:.2} 条/秒", query_success as f64 / query_duration.as_secs_f64());
        println!("查询成功率: {:.2}%", (query_success as f64 / 10000.0) * 100.0);
        
        // 释放表引用
        drop(table);
        
        // 输出监控指标
        println!("\n查询10,000条记录后的监控指标:");
        println!("{}", db.dump_metrics());
        
        // 3. 测试删除性能（删除10,000条记录）
        println!("\n3. 删除性能测试（10,000条记录）...");
        let start_time = Instant::now();
        
        // 获取表引用
        let table = db.get_table_mut(0).unwrap();
        let mut delete_success = 0;
        let mut deleted_ids = Vec::with_capacity(10000);
        for i in 0..10000 {
            // 删除前10,000条记录
            let record_id = inserted_ids[i];
            let delete_result = table.delete(record_id);
            if delete_result.is_ok() {
                delete_success += 1;
                deleted_ids.push(record_id);
            }
        }
        
        let record_count_after_delete = table.record_count();
        let delete_duration = start_time.elapsed();
        println!("删除完成，耗时: {:?}", delete_duration);
        println!("删除速率: {:.2} 条/秒", delete_success as f64 / delete_duration.as_secs_f64());
        println!("当前记录数: {}", record_count_after_delete);
        
        // 释放表引用
        drop(table);
        
        // 输出监控指标
        println!("\n删除10,000条记录后的监控指标:");
        println!("{}", db.dump_metrics());
        
        // 4. 测试插入性能（插入10,000条新记录，填充被删除的空间）
        println!("\n4. 插入性能测试（10,000条新记录）...");
        let start_time = Instant::now();
        
        // 获取表引用
        let table = db.get_table_mut(0).unwrap();
        let mut insert_success = 0;
        for i in 80000..90000 {
            let mut record_data = [0u8; 40];
            
            // 设置ID字段
            let id: u32 = (i + 1) as u32;
            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                record_data.as_mut_ptr(),
                4
            );
            
            // 设置value字段
            let value: f32 = (i as f32) * 1.5;
            core::ptr::copy_nonoverlapping(
                &value as *const f32 as *const u8,
                record_data.as_mut_ptr().add(4),
                4
            );
            
            // 设置name字段
            let name_str = format!("record_{:08}", i);
            let name_bytes = name_str.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_data.as_mut_ptr().add(8),
                name_bytes.len()
            );
            
            // 插入记录
            let result = table.insert(record_data.as_ptr());
            if result.is_ok() {
                insert_success += 1;
            }
        }
        
        let record_count_after_insert = table.record_count();
        let second_insert_duration = start_time.elapsed();
        println!("插入完成，耗时: {:?}", second_insert_duration);
        println!("插入速率: {:.2} 条/秒", insert_success as f64 / second_insert_duration.as_secs_f64());
        println!("当前记录数: {}", record_count_after_insert);
        
        // 释放表引用
        drop(table);
        
        // 输出监控指标
        println!("\n第二次插入10,000条记录后的监控指标:");
        println!("{}", db.dump_metrics());
        
        // 5. 测试批量查询性能（顺序查询10,000条记录）
        println!("\n5. 批量查询性能测试（顺序查询10,000条记录）...");
        let start_time = Instant::now();
        
        // 获取表引用
        let table = db.get_table_mut(0).unwrap();
        let mut batch_query_success = 0;
        for i in 10000..20000 {
            // 使用预先保存的记录ID查询
            let record_id = inserted_ids[i];
            
            // 读取记录数据
            let mut result_data = [0u8; 40];
            let get_result = table.get_by_id(record_id, result_data.as_mut_ptr());
            if get_result.is_ok() {
                batch_query_success += 1;
            }
        }
        
        let batch_query_duration = start_time.elapsed();
        println!("批量查询完成，耗时: {:?}", batch_query_duration);
        println!("批量查询速率: {:.2} 条/秒", batch_query_success as f64 / batch_query_duration.as_secs_f64());
        
        // 释放表引用
        drop(table);
        
        // 输出监控指标
        println!("\n批量查询10,000条记录后的监控指标:");
        println!("{}", db.dump_metrics());
        
        // 执行健康检查（暂时注释，health_check方法不存在）
        // println!("\n健康检查结果:");
        // let health_result = db.health_check();
        // println!("{}", health_result.to_text());
        
        // 输出最终指标快照
        println!("\n测试完成，最终指标快照:");
        let snapshot = db.metrics_snapshot();
        println!("{}", snapshot.to_text());
        
        // 重置指标
        db.reset_metrics();
        println!("\n指标已重置，准备下次测试");
        
        println!("\n=== 大表性能测试结束 ===");
    }
}
