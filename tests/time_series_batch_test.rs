extern crate alloc;

use core::ptr::NonNull;
use remdb::types::time_utils;
use remdb::*;
use serial_test::serial;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152]; // 增加到2MB

// 定义时间序列表结构
remdb::table!(
    test_metrics,
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
    TEST_DB_CONFIG,
    tables: [test_metrics]
);

// 初始化测试环境
unsafe fn init_test_env() -> &'static mut RemDb {
    // 使用生成的数据库配置静态变量
    let config = &TEST_DB_CONFIG;

    // 清空内存缓冲区，确保每次测试都有干净的环境
    core::ptr::write_bytes(DB_MEMORY.as_mut_ptr(), 0, DB_MEMORY.len());

    // 重新初始化内存分配器（每次测试都重置）
    let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

    // 初始化平台抽象层
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

    // 初始化全局数据库
    let db = init_global_db(config).unwrap();

    db
}

// 辅助函数：根据字段名获取字段索引
unsafe fn get_field_index(table: &crate::table::MemoryTable, field_name: &str) -> usize {
    for (i, field) in table.def.fields.iter().enumerate() {
        if field.name == field_name {
            return i;
        }
    }
    panic!("字段 {} 未找到", field_name);
}

// 测试时间序列批量插入
#[test]
#[serial]
fn test_time_series_batch_insert() {
    unsafe {
        let db = init_test_env();
        let table_mut = db.get_table_mut(0).unwrap();

        // 打印字段偏移信息
        println!("=== 字段偏移信息 ===");
        println!("记录大小: {}", table_mut.record_size);
        for (i, field) in table_mut.def.fields.iter().enumerate() {
            println!(
                "字段 {}: 名称={}, 大小={}, 偏移={}",
                i, field.name, field.size, field.offset
            );
        }

        // 获取字段索引
        let id_field_index = get_field_index(table_mut, "id");
        let metric_name_field_index = get_field_index(table_mut, "metric_name");
        let value_field_index = get_field_index(table_mut, "value");
        let timestamp_field_index = get_field_index(table_mut, "timestamp");
        let tags_field_index = get_field_index(table_mut, "tags");

        println!("=== 字段索引 ===");
        println!("id: {}", id_field_index);
        println!("metric_name: {}", metric_name_field_index);
        println!("value: {}", value_field_index);
        println!("timestamp: {}", timestamp_field_index);
        println!("tags: {}", tags_field_index);

        // 生成测试数据
        let mut records_buffer = [0u8; 120 * 100]; // 100条记录，使用正确的记录大小
        let mut record_ids = [0usize; 100];

        for i in 0..100 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) * 0.5 + 50.0; // 50.0 to 99.5
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";

            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * table_mut.record_size);

            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr.add(table_mut.def.fields[id_field_index].offset),
                4,
            );

            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[metric_name_field_index].offset),
                name_bytes.len(),
            );

            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(table_mut.def.fields[value_field_index].offset),
                8,
            );

            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(table_mut.def.fields[timestamp_field_index].offset),
                8,
            );

            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[tags_field_index].offset),
                tags_bytes.len(),
            );
        }

        // 使用时间序列批量插入优化
        let inserted_count = table_mut
            .time_series_batch_insert(records_buffer.as_ptr(), 100, record_ids.as_mut_ptr())
            .unwrap();

        assert_eq!(
            inserted_count, 100,
            "批量插入失败，预期插入100条，实际插入{}",
            inserted_count
        );
        assert_eq!(
            table_mut.record_count(),
            100,
            "记录数不符，预期100，实际{}",
            table_mut.record_count()
        );
    }
}

// 测试时间范围查询
#[test]
#[serial]
fn test_time_range_query() {
    unsafe {
        let db = init_test_env();
        let table_mut = db.get_table_mut(0).unwrap();

        // 获取字段索引
        let id_field_index = get_field_index(table_mut, "id");
        let metric_name_field_index = get_field_index(table_mut, "metric_name");
        let value_field_index = get_field_index(table_mut, "value");
        let timestamp_field_index = get_field_index(table_mut, "timestamp");
        let tags_field_index = get_field_index(table_mut, "tags");

        // 生成测试数据
        let mut records_buffer = [0u8; 120 * 100]; // 使用正确的记录大小
        let mut record_ids = [0usize; 100];

        for i in 0..100 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) * 0.5 + 50.0; // 50.0 to 99.5
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";

            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * table_mut.record_size);

            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr.add(table_mut.def.fields[id_field_index].offset),
                4,
            );

            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[metric_name_field_index].offset),
                name_bytes.len(),
            );

            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(table_mut.def.fields[value_field_index].offset),
                8,
            );

            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(table_mut.def.fields[timestamp_field_index].offset),
                8,
            );

            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[tags_field_index].offset),
                tags_bytes.len(),
            );
        }

        // 插入测试数据
        table_mut
            .time_series_batch_insert(records_buffer.as_ptr(), 100, record_ids.as_mut_ptr())
            .unwrap();

        // 测试时间范围查询
        let start_time = 1609459200000;
        let end_time = 1609459200000 + 30 * 60000; // 30分钟

        let mut result_buffer = [0u8; 120 * 50]; // 使用正确的记录大小
        let found_count = table_mut
            .get_records_in_time_window(
                timestamp_field_index, // 使用正确的timestamp字段索引
                start_time,
                end_time,
                result_buffer.as_mut_ptr(),
                50,
            )
            .unwrap();

        assert_eq!(
            found_count, 31,
            "时间范围查询失败，预期找到31条，实际找到{}",
            found_count
        );

        // 验证第一条记录
        let first_record = &result_buffer[0..table_mut.record_size];
        let id = core::ptr::read(
            first_record
                .as_ptr()
                .add(table_mut.def.fields[id_field_index].offset) as *const i32,
        );
        let value = core::ptr::read(
            first_record
                .as_ptr()
                .add(table_mut.def.fields[value_field_index].offset) as *const f64,
        );
        let timestamp = core::ptr::read(
            first_record
                .as_ptr()
                .add(table_mut.def.fields[timestamp_field_index].offset) as *const u64,
        );

        assert_eq!(id, 1, "第一条记录ID不符，预期1，实际{}", id);
        assert_eq!(value, 50.0, "第一条记录value不符，预期50.0，实际{}", value);
        assert_eq!(
            timestamp, 1609459200000,
            "第一条记录timestamp不符，预期1609459200000，实际{}",
            timestamp
        );
    }
}

// 测试聚合功能
#[test]
#[serial]
fn test_aggregation_functions() {
    unsafe {
        let db = init_test_env();
        let table_mut = db.get_table_mut(0).unwrap();

        // 获取字段索引
        let id_field_index = get_field_index(table_mut, "id");
        let metric_name_field_index = get_field_index(table_mut, "metric_name");
        let value_field_index = get_field_index(table_mut, "value");
        let timestamp_field_index = get_field_index(table_mut, "timestamp");
        let tags_field_index = get_field_index(table_mut, "tags");

        // 生成测试数据
        let mut records_buffer = [0u8; 120 * 10]; // 使用正确的记录大小
        let mut record_ids = [0usize; 10];

        for i in 0..10 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) + 1.0; // 1.0 to 10.0
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";

            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * table_mut.record_size);

            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr.add(table_mut.def.fields[id_field_index].offset),
                4,
            );

            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[metric_name_field_index].offset),
                name_bytes.len(),
            );

            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(table_mut.def.fields[value_field_index].offset),
                8,
            );

            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(table_mut.def.fields[timestamp_field_index].offset),
                8,
            );

            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[tags_field_index].offset),
                tags_bytes.len(),
            );
        }

        // 插入测试数据
        table_mut
            .time_series_batch_insert(records_buffer.as_ptr(), 10, record_ids.as_mut_ptr())
            .unwrap();

        // 测试聚合功能
        let start_time = 1609459200000;
        let end_time = 1609459200000 + 10 * 60000; // 10分钟

        // 测试count
        let count = table_mut
            .aggregate_count(timestamp_field_index, start_time, end_time)
            .unwrap();
        assert_eq!(count, 10, "聚合count失败，预期10，实际{}", count);

        // 测试sum
        let sum = table_mut
            .aggregate_sum(
                timestamp_field_index,
                value_field_index,
                start_time,
                end_time,
            )
            .unwrap();
        assert_eq!(sum, 55.0, "聚合sum失败，预期55.0，实际{}", sum);

        // 测试avg
        let avg = table_mut
            .aggregate_avg(
                timestamp_field_index,
                value_field_index,
                start_time,
                end_time,
            )
            .unwrap();
        assert_eq!(avg, 5.5, "聚合avg失败，预期5.5，实际{}", avg);

        // 测试min
        let min = table_mut
            .aggregate_min(
                timestamp_field_index,
                value_field_index,
                start_time,
                end_time,
            )
            .unwrap();
        assert_eq!(min, 1.0, "聚合min失败，预期1.0，实际{}", min);

        // 测试max
        let max = table_mut
            .aggregate_max(
                timestamp_field_index,
                value_field_index,
                start_time,
                end_time,
            )
            .unwrap();
        assert_eq!(max, 10.0, "聚合max失败，预期10.0，实际{}", max);
    }
}

// 测试获取最新记录
#[test]
#[serial]
fn test_get_latest_records() {
    unsafe {
        let db = init_test_env();
        let table_mut = db.get_table_mut(0).unwrap();

        // 获取字段索引
        let id_field_index = get_field_index(table_mut, "id");
        let metric_name_field_index = get_field_index(table_mut, "metric_name");
        let value_field_index = get_field_index(table_mut, "value");
        let timestamp_field_index = get_field_index(table_mut, "timestamp");
        let tags_field_index = get_field_index(table_mut, "tags");

        // 生成测试数据
        let mut records_buffer = [0u8; 120 * 50]; // 使用正确的记录大小
        let mut record_ids = [0usize; 50];

        for i in 0..50 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) * 0.5 + 50.0; // 50.0 to 74.5
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";

            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * table_mut.record_size);

            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr.add(table_mut.def.fields[id_field_index].offset),
                4,
            );

            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[metric_name_field_index].offset),
                name_bytes.len(),
            );

            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(table_mut.def.fields[value_field_index].offset),
                8,
            );

            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(table_mut.def.fields[timestamp_field_index].offset),
                8,
            );

            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[tags_field_index].offset),
                tags_bytes.len(),
            );
        }

        // 插入测试数据
        table_mut
            .time_series_batch_insert(records_buffer.as_ptr(), 50, record_ids.as_mut_ptr())
            .unwrap();

        // 测试获取最新记录
        let mut latest_buffer = [0u8; 120 * 10]; // 使用正确的记录大小
        let latest_count = table_mut
            .get_latest_records(
                timestamp_field_index, // 使用正确的timestamp字段索引
                10,
                latest_buffer.as_mut_ptr(),
            )
            .unwrap();

        assert_eq!(
            latest_count, 10,
            "获取最新记录失败，预期10条，实际{}",
            latest_count
        );

        // 验证获取到的记录数
        assert_eq!(
            latest_count, 10,
            "获取最新记录失败，预期10条，实际{}",
            latest_count
        );

        // 验证第一条最新记录（应该是timestamp最大的）
        let latest_record = &latest_buffer[0..table_mut.record_size];
        let timestamp = core::ptr::read(
            latest_record
                .as_ptr()
                .add(table_mut.def.fields[timestamp_field_index].offset) as *const u64,
        );

        // 验证时间戳是否为最大的
        let expected_max_timestamp = 1609459200000 + 49 * 60000;
        assert_eq!(
            timestamp, expected_max_timestamp,
            "最新记录timestamp不符，预期{}，实际{}",
            expected_max_timestamp, timestamp
        );

        // 验证所有记录的时间戳是降序排列的
        let mut prev_timestamp = u64::MAX;
        for i in 0..latest_count {
            let record = &latest_buffer[i * table_mut.record_size..(i + 1) * table_mut.record_size];
            let current_timestamp = core::ptr::read(
                record
                    .as_ptr()
                    .add(table_mut.def.fields[timestamp_field_index].offset)
                    as *const u64,
            );
            assert!(
                current_timestamp <= prev_timestamp,
                "记录时间戳不是降序排列"
            );
            prev_timestamp = current_timestamp;
        }
    }
}

// 测试时间窗口聚合
#[test]
#[serial]
fn test_time_window_aggregation() {
    unsafe {
        let db = init_test_env();
        let table_mut = db.get_table_mut(0).unwrap();

        // 获取字段索引
        let id_field_index = get_field_index(table_mut, "id");
        let metric_name_field_index = get_field_index(table_mut, "metric_name");
        let value_field_index = get_field_index(table_mut, "value");
        let timestamp_field_index = get_field_index(table_mut, "timestamp");
        let tags_field_index = get_field_index(table_mut, "tags");

        // 生成测试数据
        let mut records_buffer = [0u8; 120 * 60]; // 使用正确的记录大小
        let mut record_ids = [0usize; 60];

        for i in 0..60 {
            // 设置字段值
            let id: i32 = i as i32 + 1;
            let metric_name = "cpu_usage";
            let value: f64 = (i as f64) * 0.5 + 50.0; // 50.0 to 79.5
            let timestamp: u64 = 1609459200000 + (i as u64) * 60000; // 每分钟一条记录
            let tags = "host=server01,region=us-west";

            // 手动填充记录数据
            let record_ptr = records_buffer.as_mut_ptr().add(i * table_mut.record_size);

            // 填充id
            core::ptr::copy_nonoverlapping(
                &id as *const i32 as *const u8,
                record_ptr.add(table_mut.def.fields[id_field_index].offset),
                4,
            );

            // 填充metric_name
            let name_bytes = metric_name.as_bytes();
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[metric_name_field_index].offset),
                name_bytes.len(),
            );

            // 填充value
            core::ptr::copy_nonoverlapping(
                &value as *const f64 as *const u8,
                record_ptr.add(table_mut.def.fields[value_field_index].offset),
                8,
            );

            // 填充timestamp
            core::ptr::copy_nonoverlapping(
                &timestamp as *const u64 as *const u8,
                record_ptr.add(table_mut.def.fields[timestamp_field_index].offset),
                8,
            );

            // 填充tags
            let tags_bytes = tags.as_bytes();
            core::ptr::copy_nonoverlapping(
                tags_bytes.as_ptr(),
                record_ptr.add(table_mut.def.fields[tags_field_index].offset),
                tags_bytes.len(),
            );
        }

        // 插入测试数据
        table_mut
            .time_series_batch_insert(records_buffer.as_ptr(), 60, record_ids.as_mut_ptr())
            .unwrap();

        // 测试时间窗口聚合
        let start_time = 1609459200000;
        let end_time = 1609459200000 + 60 * 60000; // 60分钟

        let window_aggregates = table_mut
            .get_aggregate_in_time_window(
                timestamp_field_index, // 使用正确的timestamp字段索引
                value_field_index,     // 使用正确的value字段索引
                start_time,
                end_time,
                120000, // 2分钟窗口
            )
            .unwrap();

        assert_eq!(
            window_aggregates.len(),
            30,
            "时间窗口聚合失败，预期30个窗口，实际{}",
            window_aggregates.len()
        );

        // 验证第一个窗口
        let first_window = &window_aggregates[0];
        assert_eq!(first_window.0, 1609459200000, "第一个窗口开始时间不符");
        assert_eq!(
            first_window.5, 2,
            "第一个窗口记录数不符，预期2，实际{}",
            first_window.5
        );
        assert_eq!(
            first_window.1, 100.5,
            "第一个窗口sum不符，预期100.5，实际{}",
            first_window.1
        );
        assert_eq!(
            first_window.2, 50.25,
            "第一个窗口avg不符，预期50.25，实际{}",
            first_window.2
        );
        assert_eq!(
            first_window.3, 50.0,
            "第一个窗口min不符，预期50.0，实际{}",
            first_window.3
        );
        assert_eq!(
            first_window.4, 50.5,
            "第一个窗口max不符，预期50.5，实际{}",
            first_window.4
        );
    }
}

// 测试时间工具函数
#[test]
fn test_time_utils() {
    // 测试时间转换
    assert_eq!(time_utils::seconds_to_millis(1), 1000);
    assert_eq!(time_utils::millis_to_seconds(1000), 1);
    assert_eq!(time_utils::micros_to_millis(1000), 1);
    assert_eq!(time_utils::millis_to_micros(1), 1000);
    assert_eq!(time_utils::nanos_to_millis(1000000), 1);
    assert_eq!(time_utils::millis_to_nanos(1), 1000000);

    // 测试时间差
    assert_eq!(time_utils::time_diff(1000, 2000), 1000);
    assert_eq!(time_utils::time_diff(2000, 1000), 1000);

    // 测试时间范围检查
    assert!(time_utils::is_in_time_range(500, 100, 1000));
    assert!(!time_utils::is_in_time_range(1500, 100, 1000));
    assert!(time_utils::is_in_time_range(100, 100, 1000));
    assert!(time_utils::is_in_time_range(1000, 100, 1000));
}
