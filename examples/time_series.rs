#![allow(unsafe_code)]
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
        // 使用一个简单的平台实现
        struct DummyPlatform;
        impl platform::Platform for DummyPlatform {
            fn get_timestamp(&self) -> u64 {
                0
            }
            fn get_timestamp_us(&self) -> u64 {
                0
            }
            fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
                let len = dest.len().min(src.len());
                dest[..len].copy_from_slice(&src[..len]);
            }
            fn memset(&self, dest: &mut [u8], value: u8) {
                dest.fill(value);
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
            fn file_write(&self, _handle: platform::FileHandle, _buf: &[u8]) -> platform::FileResult<usize> {
                Err(())
            }
            fn file_read(&self, _handle: platform::FileHandle, _buf: &mut [u8]) -> platform::FileResult<usize> {
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
            fn crc32(&self, _data: &[u8]) -> u32 {
                0
            }
        }
        static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
        platform::init_platform(&DUMMY_PLATFORM);
        
        // 初始化全局数据库
        let db = init_global_db(config).unwrap();
        
        // 获取表引用
        let table_mut = db.get_table_mut(0).unwrap();
        
        // 批量插入测试数据
        let record_size = table_mut.record_size;
        let mut records_buffer = [0u8; 120 * 100]; // 100条记录，使用最大可能的记录大小
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
            let record_ptr = records_buffer.as_mut_ptr().add(i * record_size);
            
            // 填充id（偏移0）
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr,
                4
            );
            
            // 填充metric_name（偏移4）
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(4),
                name_bytes.len()
            );
            
            // 填充value（偏移40）
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(40),
                8
            );
            
            // 填充timestamp（偏移48）
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(48),
                8
            );
            
            // 填充tags（偏移56）
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(56),
                tags_bytes.len()
            );
        }
        
        // 使用时间序列批量插入优化
        let inserted_count = table_mut.time_series_batch_insert(
            &records_buffer,
            100,
            &mut record_ids
        ).unwrap();
        
        println!("成功插入 {} 条时间序列记录", inserted_count);
        
        // 测试时间范围查询
        println!("\n=== 时间范围查询测试 ===");
        let start_time = 1609459200000;
        let end_time = 1609459200000 + 30 * 60000; // 30分钟
        
        let mut result_buffer = [0u8; 120 * 50]; // 使用最大可能的记录大小
        let found_count = table_mut.get_records_in_time_window(
            3, // timestamp字段索引
            start_time,
            end_time,
            &mut result_buffer,
            50
        ).unwrap();
        
        println!("在时间范围内找到 {} 条记录", found_count);
        
        // 读取第一条记录验证
        let first_record = &result_buffer[0..record_size];
        let id = core::ptr::read(first_record.as_ptr() as *const i32);
        let value = core::ptr::read(first_record.as_ptr().add(40) as *const f64);
        let timestamp = core::ptr::read(first_record.as_ptr().add(48) as *const u64);
        
        println!("第一条记录: ID={}, Value={:.1}, Timestamp={}", id, value, timestamp);
        
        // 测试聚合功能
        println!("\n=== 时间序列聚合测试 ===");
        
        // 统计记录数
        match table_mut.aggregate_count(3, start_time, end_time) {
            Ok(count) => {
                println!("时间范围内记录数: {}", count);
                
                if count > 0 {
                    // 计算平均值
                    if let Ok(avg) = table_mut.aggregate_avg(3, 2, start_time, end_time) {
                        println!("时间范围内平均值: {:.2}", avg);
                    }
                    
                    // 计算总和
                    if let Ok(sum) = table_mut.aggregate_sum(3, 2, start_time, end_time) {
                        println!("时间范围内总和: {:.2}", sum);
                    }
                    
                    // 计算最小值
                    if let Ok(min) = table_mut.aggregate_min(3, 2, start_time, end_time) {
                        println!("时间范围内最小值: {:.2}", min);
                    }
                    
                    // 计算最大值
                    if let Ok(max) = table_mut.aggregate_max(3, 2, start_time, end_time) {
                        println!("时间范围内最大值: {:.2}", max);
                    }
                } else {
                    println!("时间范围内没有记录，跳过聚合计算");
                }
            },
            Err(e) => println!("统计记录数失败: {:?}", e)
        }
        
        // 测试获取最新记录
        println!("\n=== 获取最新记录测试 ===");
        let mut latest_buffer = [0u8; 120 * 10]; // 使用最大可能的记录大小
        let latest_count = table_mut.get_latest_records(
            3, // timestamp字段索引
            10,
            &mut latest_buffer
        ).unwrap();
        
        println!("获取到 {} 条最新记录", latest_count);
        
        // 读取第一条最新记录
        let latest_record = &latest_buffer[0..record_size];
        let latest_id = core::ptr::read(latest_record.as_ptr() as *const i32);
        let latest_value = core::ptr::read(latest_record.as_ptr().add(40) as *const f64);
        let latest_timestamp = core::ptr::read(latest_record.as_ptr().add(48) as *const u64);
        
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
        
        // 测试新的专用方法
        println!("\n=== 测试新的专用方法 ===");
        
        // 使用insert_record插入单条记录
        println!("\n1. 使用insert_record插入单条记录:");
        let columns = &["id", "metric_name", "value", "timestamp", "tags"];
        let values = &["200", "memory_usage", "75.5", "1609459200000", "host=server01,region=us-west"];
        let affected_rows = db.insert_record("metrics", columns, values).unwrap();
        println!("插入记录成功，影响行数: {}", affected_rows);
        
        // 使用execute_query查询记录
        println!("\n2. 使用execute_query查询记录:");
        let result = db.execute_query("metrics", &["id", "metric_name", "value", "timestamp"], Some("id = 200"), None).unwrap();
        println!("查询结果: {}", result.to_string());
        
        // 使用update_record更新记录
        println!("\n3. 使用update_record更新记录:");
        let update_affected = db.update_record("metrics", "value = 80.0, tags = 'host=server01,region=us-west,updated=true'", Some("id = 200")).unwrap();
        println!("更新记录成功，影响行数: {}", update_affected);
        
        // 查询验证更新
        let updated_result = db.execute_query("metrics", &["id", "metric_name", "value", "tags"], Some("id = 200"), None).unwrap();
        println!("更新后查询结果: {}", updated_result.to_string());
        
        // 使用execute_query进行更复杂的查询
        println!("\n4. 使用execute_query进行更复杂的查询:");
        let complex_result = db.execute_query("metrics", &["id", "metric_name", "value", "timestamp"], Some("value > 90.0 AND timestamp < 1609459200000 + 600000"), Some(5)).unwrap();
        println!("查询结果: {}", complex_result.to_string());
        
        println!("\n=== 时间序列功能测试完成 ===");
        println!("所有测试通过!");
    }
}
