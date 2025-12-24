use remdb::table::*;
use remdb::types::*;
use remdb::platform::*;
use std::time::Instant;
use rand::random;

// 定义一个用于性能测试的大表，max_records设置为500,000
static LARGE_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "large_table",
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
        FieldDef {
            name: "name",
            data_type: DataType::String,
            size: 32,
            offset: 8,
        },
    ],
    primary_key: 0,
    secondary_index: None,
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
    
    // 计算所需的内存缓冲区大小
    let data_buffer_size = LARGE_TABLE_DEF.record_size * LARGE_TABLE_DEF.max_records;
    let status_buffer_size = LARGE_TABLE_DEF.max_records * core::mem::size_of::<RecordHeader>();
    let free_slots_size = LARGE_TABLE_DEF.max_records * core::mem::size_of::<usize>();
    
    // 分配内存缓冲区
    let mut data_buffer = vec![0u8; data_buffer_size];
    let mut status_buffer = vec![0u8; status_buffer_size];
    let mut free_slots_buffer = vec![0usize; LARGE_TABLE_DEF.max_records];
    
    // 将状态缓冲区转换为RecordHeader数组指针
    let status_ptr = status_buffer.as_mut_ptr() as *mut RecordHeader;
    let free_slots_ptr = free_slots_buffer.as_mut_ptr();
    
    unsafe {
        // 创建表
        let mut table = MemoryTable::new(
            &LARGE_TABLE_DEF,
            data_buffer.as_mut_ptr(),
            status_ptr,
            free_slots_ptr
        ).unwrap();
        
        // 1. 插入80,000条记录（达到80%容量）
        println!("\n1. 插入80,000条记录...");
        let start_time = Instant::now();
        
        let mut inserted_ids = Vec::with_capacity(80000);
        for i in 0..80000 {
            let mut record_data = [0u8; 40]; // 40字节记录
            
            // 设置ID字段（Int32）
            let id: i32 = (i + 1) as i32;
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
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
        
        let insert_duration = start_time.elapsed();
        println!("插入完成，耗时: {:?}", insert_duration);
        println!("插入速率: {:.2} 条/秒", 80000 as f64 / insert_duration.as_secs_f64());
        println!("当前记录数: {}", table.record_count());
        
        // 2. 测试查询性能（查询10,000条随机记录）
        println!("\n2. 查询性能测试（10,000条随机记录）...");
        let start_time = Instant::now();
        
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
        
        // 3. 测试删除性能（删除10,000条记录）
        println!("\n3. 删除性能测试（10,000条记录）...");
        let start_time = Instant::now();
        
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
        
        let delete_duration = start_time.elapsed();
        println!("删除完成，耗时: {:?}", delete_duration);
        println!("删除速率: {:.2} 条/秒", delete_success as f64 / delete_duration.as_secs_f64());
        println!("当前记录数: {}", table.record_count());
        
        // 4. 测试插入性能（插入10,000条新记录，填充被删除的空间）
        println!("\n4. 插入性能测试（10,000条新记录）...");
        let start_time = Instant::now();
        
        let mut insert_success = 0;
        for i in 80000..90000 {
            let mut record_data = [0u8; 40];
            
            // 设置ID字段
            let id: i32 = (i + 1) as i32;
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
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
        
        let second_insert_duration = start_time.elapsed();
        println!("插入完成，耗时: {:?}", second_insert_duration);
        println!("插入速率: {:.2} 条/秒", insert_success as f64 / second_insert_duration.as_secs_f64());
        println!("当前记录数: {}", table.record_count());
        
        // 5. 测试批量查询性能（顺序查询10,000条记录）
        println!("\n5. 批量查询性能测试（顺序查询10,000条记录）...");
        let start_time = Instant::now();
        
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
        
        println!("\n=== 大表性能测试结束 ===");
    }
}
