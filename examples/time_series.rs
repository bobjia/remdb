extern crate alloc;

use core::ptr::NonNull;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 524288] = [0u8; 524288]; // Increased to 512KB to accommodate all allocations

// 定义时间序列表结构
remdb::table!(
    metrics,
    1000, // 最大记录数
    primary_key: id,
    secondary_index: timestamp,
    fields: {
        id: i32,
        metric_name: str(32), // 32字节定长字符串
        value: f64,
        timestamp: u64,
        tags: str(64) // 64字节定长字符串，用于存储标签
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [metrics]
);

fn main() {
    unsafe {
        // 使用生成的数据库配置静态变量
        let config = &DB_CONFIG;
        
        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        platform::init_platform(platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            // 在非posix平台上，使用一个简单的平台实现
            struct DummyPlatform;
            impl platform::Platform for DummyPlatform {
                fn get_timestamp(&self) -> u64 {
                    0
                }
                fn get_timestamp_us(&self) -> u64 {
                    0
                }
                fn spin_lock(&self, _lock: &mut u32) {
                }
                fn spin_unlock(&self, _lock: &mut u32) {
                }
                fn compiler_barrier(&self) {
                }
                fn full_memory_barrier(&self) {
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
                }
                fn delay_us(&self, _us: u32) {
                }
                fn file_open(&self, _path: &str, _mode: platform::FileMode) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(&self, _handle: platform::FileHandle, _buffer: *const u8, _size: usize) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(&self, _handle: platform::FileHandle, _buffer: *mut u8, _size: usize) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(&self, _handle: platform::FileHandle, _offset: i64, _whence: platform::SeekWhence) -> platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            platform::init_platform(&DUMMY_PLATFORM);
        }
        
        // 计算所需内存大小
        let table_size = MemoryTable::calculate_memory_size(&config.tables[0]);
        let primary_index_size = PrimaryIndex::calculate_memory_size(
            &config.tables[0],
            256, // 哈希表大小
            1000  // 最大索引项数量
        );
        let secondary_index_size = SecondaryIndex::calculate_memory_size(1000);
        
        println!("Table size: {} bytes", table_size);
        println!("Primary index size: {} bytes", primary_index_size);
        println!("Secondary index size: {} bytes", secondary_index_size);
        
        // 分配内存
        let table_ptr = memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8;
        let status_ptr = memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader;
        
        let free_slots_ptr = memory::allocator::alloc(
            core::mem::size_of::<usize>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut usize;
        
        let hash_table_ptr = memory::allocator::alloc(
            256 * core::mem::size_of::<Option<NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<NonNull<index::PrimaryIndexItem>>;
        
        let primary_index_items_ptr = memory::allocator::alloc(
            1000 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem;
        
        let secondary_index_items_ptr = memory::allocator::alloc(
            1000 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem;
        
        // 创建表和索引
        let table = MemoryTable::new(&config.tables[0], table_ptr, status_ptr, free_slots_ptr).unwrap();
        let primary_index = PrimaryIndex::new(
            &config.tables[0],
            hash_table_ptr,
            primary_index_items_ptr,
            256,
            1000
        );
        let secondary_index = SecondaryIndex::new(&config.tables[0], secondary_index_items_ptr, 1000);
        
        // 初始化表和索引数组
        static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
        static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
        static mut SECONDARY_INDICES: [Option<SecondaryIndex>; 1] = [None; 1];
        
        TABLES[0] = Some(table);
        PRIMARY_INDICES[0] = Some(primary_index);
        SECONDARY_INDICES[0] = Some(secondary_index);
        
        // 初始化全局数据库
        let db = init_global_db(
            config,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        ).unwrap();
        
        // 获取表引用
        let table_mut = db.get_table_mut(0).unwrap();
        
        // 批量插入测试数据
        let mut records_buffer = [0u8; 108 * 100]; // 100条记录
        let mut record_ids = [0usize; 100];
        
        println!("\n=== 批量插入测试数据 ===");
        for i in 0..100 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) * 0.5 + 50.0; // 50.0 to 99.5
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";
            
            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * 108);
            
            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr,
                4
            );
            
            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(4),
                name_bytes.len()
            );
            
            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(36),
                8
            );
            
            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(44),
                8
            );
            
            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(52),
                tags_bytes.len()
            );
        }
        
        // 使用时间序列批量插入优化
        let inserted_count = table_mut.time_series_batch_insert(
            records_buffer.as_ptr(),
            100,
            record_ids.as_mut_ptr()
        ).unwrap();
        
        println!("成功插入 {} 条时间序列记录", inserted_count);
        
        // 测试时间范围查询
        println!("\n=== 时间范围查询测试 ===");
        let start_time = 1609459200000;
        let end_time = 1609459200000 + 30 * 60000; // 30分钟
        
        let mut result_buffer = [0u8; 108 * 50];
        let found_count = table_mut.get_records_in_time_window(
            3, // timestamp字段索引
            start_time,
            end_time,
            result_buffer.as_mut_ptr(),
            50
        ).unwrap();
        
        println!("在时间范围内找到 {} 条记录", found_count);
        
        // 读取第一条记录验证
        let first_record = &result_buffer[0..108];
        let id = core::ptr::read(first_record.as_ptr() as *const i32);
        let value = core::ptr::read(first_record.as_ptr().add(36) as *const f64);
        let timestamp = core::ptr::read(first_record.as_ptr().add(44) as *const u64);
        
        println!("第一条记录: ID={}, Value={:.1}, Timestamp={}", id, value, timestamp);
        
        // 测试聚合功能
        println!("\n=== 时间序列聚合测试 ===");
        
        // 统计记录数
        let count = table_mut.aggregate_count(3, start_time, end_time).unwrap();
        println!("时间范围内记录数: {}", count);
        
        // 计算平均值
        let avg = table_mut.aggregate_avg(3, 2, start_time, end_time).unwrap();
        println!("时间范围内平均值: {:.2}", avg);
        
        // 计算总和
        let sum = table_mut.aggregate_sum(3, 2, start_time, end_time).unwrap();
        println!("时间范围内总和: {:.2}", sum);
        
        // 计算最小值
        let min = table_mut.aggregate_min(3, 2, start_time, end_time).unwrap();
        println!("时间范围内最小值: {:.2}", min);
        
        // 计算最大值
        let max = table_mut.aggregate_max(3, 2, start_time, end_time).unwrap();
        println!("时间范围内最大值: {:.2}", max);
        
        // 测试获取最新记录
        println!("\n=== 获取最新记录测试 ===");
        let mut latest_buffer = [0u8; 108 * 10];
        let latest_count = table_mut.get_latest_records(
            3, // timestamp字段索引
            10,
            latest_buffer.as_mut_ptr()
        ).unwrap();
        
        println!("获取到 {} 条最新记录", latest_count);
        
        // 读取第一条最新记录
        let latest_record = &latest_buffer[0..108];
        let latest_id = core::ptr::read(latest_record.as_ptr() as *const i32);
        let latest_value = core::ptr::read(latest_record.as_ptr().add(36) as *const f64);
        let latest_timestamp = core::ptr::read(latest_record.as_ptr().add(44) as *const u64);
        
        println!("最新记录: ID={}, Value={:.1}, Timestamp={}", latest_id, latest_value, latest_timestamp);
        
        // 测试时间窗口聚合
        println!("\n=== 时间窗口聚合测试 ===");
        let window_aggregates = table_mut.get_aggregate_in_time_window(
            3, // timestamp字段索引
            2, // value字段索引
            start_time,
            end_time,
            120000 // 2分钟窗口
        ).unwrap();
        
        println!("时间窗口聚合结果 (共 {} 个窗口):", window_aggregates.len());
        for (i, (window_start, sum, avg, min, max, count)) in window_aggregates.iter().enumerate() {
            println!("窗口 {}: 开始时间={}, 记录数={}, 平均值={:.2}, 最小值={:.2}, 最大值={:.2}", 
                     i+1, window_start, count, avg, min, max);
        }
        
        println!("\n=== 时间序列功能测试完成 ===");
        println!("所有测试通过!");
    }
}
