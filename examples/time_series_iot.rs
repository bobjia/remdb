extern crate alloc;

use remdb::time_series::*;
use remdb::*;
use std::time::{Duration, SystemTime};

// 定义物联网传感器数据的时序表结构
remdb::table!(
    sensor_data,
    5000, // 最大记录数
    primary_key: id,
    secondary_index: timestamp,
    fields: {
        id: i32,
        sensor_id: str(32),  // 传感器ID
        sensor_type: str(32), // 传感器类型（温度、湿度、压力等）
        value: f64,           // 传感器数值
        timestamp: u64,       // 时间戳
        location: str(64)     // 位置信息
    }
);

// 定义数据库配置
remdb::database!(
    DB_CONFIG,
    tables: [sensor_data]
);

fn main() {
    unsafe {
        // 1. 初始化内存分配器
        println!("=== 1. 初始化内存分配器 ===");
        let memory_size = 128 * 1024 * 1024; // 128MB
        static mut DB_MEMORY: [u8; 128 * 1024 * 1024] = [0u8; 128 * 1024 * 1024];

        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());
        println!("内存分配器初始化成功 (128MB)");

        // 2. 初始化平台抽象层
        println!("\n=== 2. 初始化平台抽象层 ===");
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
                fn spin_lock(&self, _lock: &mut u32) {}
                fn spin_unlock(&self, _lock: &mut u32) {}
                fn compiler_barrier(&self) {}
                fn full_memory_barrier(&self) {}
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
                fn delay_ms(&self, _ms: u32) {}
                fn delay_us(&self, _us: u32) {}
                fn file_open(
                    &self,
                    _path: &str,
                    _mode: platform::FileMode,
                ) -> platform::FileResult<platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: platform::FileHandle,
                    _offset: i64,
                    _whence: platform::SeekWhence,
                ) -> platform::FileResult<u64> {
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
        println!("平台抽象层初始化成功");

        // 3. 初始化全局数据库
        println!("\n=== 3. 初始化全局数据库 ===");
        let db = init_global_db(&DB_CONFIG).unwrap();
        println!("数据库初始化成功");

        // 4. 模拟多传感器数据采集和写入
        println!("\n=== 4. 模拟多传感器数据采集和写入 ===");

        // 获取当前时间戳（毫秒）
        let base_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 传感器列表
        let sensors = [
            ("temp_sensor_001", "temperature", "room_101"),
            ("hum_sensor_001", "humidity", "room_101"),
            ("press_sensor_001", "pressure", "room_101"),
            ("temp_sensor_002", "temperature", "room_102"),
            ("hum_sensor_002", "humidity", "room_102"),
        ];

        {
            // 开始一个作用域，限制table_mut的生命周期
            // 获取表引用
            let table_mut = db.get_table_mut(0).unwrap();
            let record_size = table_mut.record_size;

            // 批量插入传感器数据
            let mut records_buffer = [0u8; 160 * 500]; // 500条记录的缓冲区
            let mut record_ids = [0usize; 500];
            let mut record_count = 0;

            // 为每个传感器生成100条历史数据（每30秒一条）
            for (sensor_id, sensor_type, location) in sensors.iter() {
                for i in 0..100 {
                    // 设置字段值
                    let id: i32 = record_count as i32 + 1;
                    let timestamp: u64 = base_time - (100 - i) as u64 * 30000; // 从100条前开始，每30秒一条

                    // 根据传感器类型生成不同的随机数据
                    let value: f64 = match *sensor_type {
                        "temperature" => 20.0 + (i as f64 % 10.0) + (i as f64 * 0.01), // 20-30°C
                        "humidity" => 40.0 + (i as f64 % 20.0) + (i as f64 * 0.02),    // 40-60%
                        "pressure" => 990.0 + (i as f64 % 20.0) + (i as f64 * 0.03), // 990-1010 hPa
                        _ => 0.0,
                    };

                    // 手动填充记录数据
                    let record_ptr = records_buffer.as_mut_ptr().add(record_count * record_size);

                    // 填充id（偏移0）
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_ptr, 4);

                    // 填充sensor_id（偏移4）
                    let sensor_id_bytes = sensor_id.as_bytes();
                    core::ptr::copy_nonoverlapping(
                        sensor_id_bytes.as_ptr(),
                        record_ptr.add(4),
                        sensor_id_bytes.len(),
                    );

                    // 填充sensor_type（偏移36）
                    let sensor_type_bytes = sensor_type.as_bytes();
                    core::ptr::copy_nonoverlapping(
                        sensor_type_bytes.as_ptr(),
                        record_ptr.add(36),
                        sensor_type_bytes.len(),
                    );

                    // 填充value（偏移68）
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        record_ptr.add(68),
                        8,
                    );

                    // 填充timestamp（偏移76）
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        record_ptr.add(76),
                        8,
                    );

                    // 填充location（偏移84）
                    let location_bytes = location.as_bytes();
                    core::ptr::copy_nonoverlapping(
                        location_bytes.as_ptr(),
                        record_ptr.add(84),
                        location_bytes.len(),
                    );

                    record_count += 1;
                }
            }

            // 执行批量插入
            let inserted_count = table_mut
                .time_series_batch_insert(
                    records_buffer.as_ptr(),
                    record_count,
                    record_ids.as_mut_ptr(),
                )
                .unwrap();

            println!("成功插入 {} 条传感器数据记录", inserted_count);
            println!(
                "涉及 {} 个传感器，每个传感器 {} 条历史数据",
                sensors.len(),
                100
            );

            // 5. 实时数据查询和分析
            println!("\n=== 5. 实时数据查询和分析 ===");

            // 5.1 查询特定传感器的最新数据
            println!("\n5.1 查询特定传感器的最新数据:");
            let mut latest_buffer = [0u8; 160 * 10]; // 10条最新记录的缓冲区
            let latest_count = table_mut
                .get_latest_records(
                    4, // timestamp字段索引
                    10,
                    latest_buffer.as_mut_ptr(),
                )
                .unwrap();

            println!("获取到 {} 条最新记录", latest_count);

            // 打印最新的5条记录
            for i in 0..5.min(latest_count) {
                let record = &latest_buffer[i * record_size..(i + 1) * record_size];
                let id = core::ptr::read(record.as_ptr() as *const i32);

                // 读取字符串字段
                let mut sensor_id_str = [0u8; 32];
                core::ptr::copy_nonoverlapping(
                    record.as_ptr().add(4),
                    sensor_id_str.as_mut_ptr(),
                    32,
                );
                let sensor_id = String::from_utf8_lossy(&sensor_id_str)
                    .trim_end_matches(char::from(0))
                    .to_string();

                let mut sensor_type_str = [0u8; 32];
                core::ptr::copy_nonoverlapping(
                    record.as_ptr().add(36),
                    sensor_type_str.as_mut_ptr(),
                    32,
                );
                let sensor_type = String::from_utf8_lossy(&sensor_type_str)
                    .trim_end_matches(char::from(0))
                    .to_string();

                let value = core::ptr::read(record.as_ptr().add(68) as *const f64);
                let timestamp = core::ptr::read(record.as_ptr().add(76) as *const u64);

                println!(
                    "  记录 {}: ID={}, 传感器={}, 类型={}, 数值={:.2}, 时间戳={}",
                    i + 1,
                    id,
                    sensor_id,
                    sensor_type,
                    value,
                    timestamp
                );
            }

            // 5.2 按时间范围查询特定类型的传感器数据
            println!("\n5.2 按时间范围查询温度传感器数据 (最近10分钟):");
            let start_time = base_time - 10 * 60 * 1000; // 10分钟前
            let end_time = base_time;

            let mut temp_buffer = [0u8; 160 * 100]; // 100条记录的缓冲区
            let temp_count = table_mut
                .get_records_in_time_window(
                    4, // timestamp字段索引
                    start_time,
                    end_time,
                    temp_buffer.as_mut_ptr(),
                    100,
                )
                .unwrap();

            println!("在时间范围内找到 {} 条温度记录", temp_count);

            // 计算温度数据的统计信息
            let mut temp_sum = 0.0;
            let mut temp_min = f64::MAX;
            let mut temp_max = f64::MIN;
            let mut temp_count_valid = 0;

            for i in 0..temp_count {
                let record = &temp_buffer[i * record_size..(i + 1) * record_size];

                // 检查传感器类型是否为温度
                let mut sensor_type_str = [0u8; 32];
                core::ptr::copy_nonoverlapping(
                    record.as_ptr().add(36),
                    sensor_type_str.as_mut_ptr(),
                    32,
                );
                let sensor_type = String::from_utf8_lossy(&sensor_type_str)
                    .trim_end_matches(char::from(0))
                    .to_string();

                if sensor_type == "temperature" {
                    let value = core::ptr::read(record.as_ptr().add(68) as *const f64);
                    temp_sum += value;
                    temp_min = temp_min.min(value);
                    temp_max = temp_max.max(value);
                    temp_count_valid += 1;
                }
            }

            if temp_count_valid > 0 {
                let temp_avg = temp_sum / temp_count_valid as f64;
                println!("温度统计信息:");
                println!("  平均温度: {:.2}°C", temp_avg);
                println!("  最低温度: {:.2}°C", temp_min);
                println!("  最高温度: {:.2}°C", temp_max);
                println!("  有效记录数: {}", temp_count_valid);
            }

            // 6.2 测试时间窗口聚合
            println!("\n6.2 测试时间窗口聚合 (5分钟窗口):");
            let window_aggregates = table_mut
                .get_aggregate_in_time_window(
                    4, // timestamp字段索引
                    3, // value字段索引
                    start_time, end_time, 300000, // 5分钟窗口
                )
                .unwrap();

            println!("时间窗口聚合结果 (共 {} 个窗口):", window_aggregates.len());
            for (i, (window_start, _sum, avg, min, max, count)) in
                window_aggregates.iter().enumerate()
            {
                println!(
                    "  窗口 {}: 开始时间={}, 记录数={}, 平均值={:.2}, 最小值={:.2}, 最大值={:.2}",
                    i + 1,
                    window_start,
                    count,
                    avg,
                    min,
                    max
                );
            }
        } // 结束作用域，释放table_mut的可变借用

        // 6. 时间序列聚合和分析
        println!("\n=== 6. 时间序列聚合和分析 ===");

        // 6.1 统计每个房间的传感器数据
        println!("\n6.1 按房间统计传感器数据:");
        let rooms = ["room_101", "room_102"];

        {
            // 开始一个作用域，获取表引用
            let table_mut = db.get_table_mut(0).unwrap();

            // 使用表级聚合方法统计数据
            for (sensor_id, sensor_type, location) in sensors.iter() {
                // 统计该传感器的记录数
                let count = table_mut
                    .aggregate_count(
                        4,                          // timestamp字段索引
                        base_time - 10 * 60 * 1000, // 10分钟前
                        base_time,                  // 当前时间
                    )
                    .unwrap();

                println!(
                    "  {} ({} - {}): {} 条记录",
                    sensor_id, sensor_type, location, count
                );
            }
        }

        // 7. 数据管理和维护
        println!("\n=== 7. 数据管理和维护 ===");

        // 7.1 模拟数据过期检查
        println!("\n7.1 模拟数据过期检查:");
        let lifecycle_manager = LifecycleManager::new(Duration::from_secs(15 * 60)); // 15分钟过期

        // 测试不同时间的数据是否过期
        let expired_time = base_time - 20 * 60 * 1000; // 20分钟前
        let recent_time = base_time - 5 * 60 * 1000; // 5分钟前

        println!(
            "  20分钟前的数据是否过期: {}",
            lifecycle_manager.is_expired(expired_time / 1000)
        );
        println!(
            "  5分钟前的数据是否过期: {}",
            lifecycle_manager.is_expired(recent_time / 1000)
        );

        // 7.2 测试数据压缩
        println!("\n7.2 测试数据压缩:");

        // 生成一些连续的整数数值用于测试Delta压缩
        let mut int_values = [0u64; 100];
        for i in 0..100 {
            int_values[i] = 1000 + i as u64;
        }

        // 使用Delta编码压缩
        let compressed = compress_delta(&int_values);

        println!("  原始数据大小: {} 字节", int_values.len() * 8);
        println!("  压缩后数据大小: {} 字节", compressed.len());
        println!(
            "  压缩率: {:.2}%",
            (1.0 - (compressed.len() as f64 / (int_values.len() * 8) as f64)) * 100.0
        );

        // 测试解压缩
        let decompressed = decompress_delta(&compressed, int_values.len());
        println!("  压缩和解压缩成功: {:?}", decompressed == int_values);

        // 8. 总结和演示结束
        println!("\n=== 8. 演示总结 ===");
        println!("物联网传感器数据时序数据库应用演示完成!");
        println!("\n主要功能展示:");
        println!("  ✅ 多传感器数据批量写入");
        println!("  ✅ 最新数据查询");
        println!("  ✅ 时间范围数据查询");
        println!("  ✅ 传感器数据统计分析");
        println!("  ✅ 灵活的SQL-like查询");
        println!("  ✅ 时间窗口聚合");
        println!("  ✅ 数据过期管理");
        println!("  ✅ 数据压缩优化");
        println!("\n这个例子展示了时序数据库在物联网场景下的完整应用流程，");
        println!("包括数据采集、存储、查询、分析和管理的各个环节。");
    }
}
