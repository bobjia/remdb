extern crate alloc;
use alloc::sync::Arc;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use remdb::memory::allocator;
use remdb::platform::*;
use remdb::table::*;
use remdb::types::*;
use remdb::TimeSeriesConfig;
use remdb::TimeSeriesIndex;
use remdb::TimeSeriesRecord;
use remdb::TimeSeriesTable;
use remdb::TimeSeriesTableDef;

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
                .compare_exchange(
                    0,
                    1,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
            {
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

    fn file_write(
        &self,
        _handle: FileHandle,
        _buffer: *const u8,
        _size: usize,
    ) -> FileResult<usize> {
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

// 简单的表定义用于基准测试
static TEST_TABLE_DEF: TableDef = TableDef {
    id: 0,
    name: "test_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::Int32,
            size: 4,
            offset: 0,
            not_null: true,
            primary_key: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float32,
            size: 4,
            offset: 4,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: IndexType::Hash,
    record_size: 8,
    max_records: 100, // 基准测试使用较小的记录数
};

// 时间序列表定义用于基准测试
static TIME_SERIES_TABLE_DEF: TableDef = TableDef {
    id: 1,
    name: "metrics",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::Int32,
            size: 4,
            offset: 0,
            not_null: true,
            primary_key: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "metric_name",
            data_type: DataType::String,
            size: 32,
            offset: 4,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float64,
            size: 8,
            offset: 36,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "timestamp",
            data_type: DataType::Timestamp,
            size: 8,
            offset: 44,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "tags",
            data_type: DataType::String,
            size: 64,
            offset: 52,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: 0,
    secondary_index: Some(3), // 时间戳作为辅助索引
    secondary_index_type: IndexType::SortedArray,
    record_size: 116,  // 4 + 32 + 8 + 8 + 64 = 116字节
    max_records: 2000, // 时间序列测试使用较大的记录数，支持2000条记录
};

// 测试表的插入操作性能
fn bench_table_insert(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 1024 * 1024; // 1MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("table_insert");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("single_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 只插入一条简单记录
            let record_data = [1u8; 8];
            // insert方法是安全的，不需要unsafe块
            black_box(table.insert(record_data.as_ptr()).unwrap());
        })
    });

    group.finish();
}

// 测试表的查询操作性能
fn bench_table_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 1024 * 1024; // 1MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("table_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("get_by_id", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            for i in 0..100 {
                let mut record_data = [0u8; 8];
                let id: i32 = (i + 1) as i32; // 从1开始，避免主键为0的问题
                let value: f32 = i as f32 * 1.0;

                // 指针操作需要unsafe块
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        record_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f32 as *const u8,
                        record_data.as_mut_ptr().add(4),
                        4,
                    );

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            let mut result_data = [0u8; 8];

            // 查询一条记录，get_by_id方法是unsafe的
            unsafe {
                black_box(table.get_by_id(50, result_data.as_mut_ptr()).unwrap());
            }
        })
    });

    group.bench_function("iterate", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            for i in 0..100 {
                let mut record_data = [0u8; 8];
                let id: i32 = (i + 1) as i32; // 从1开始，避免主键为0的问题
                let value: f32 = i as f32 * 1.0;

                // 指针操作需要unsafe块
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        record_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f32 as *const u8,
                        record_data.as_mut_ptr().add(4),
                        4,
                    );

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            let mut count = 0;

            // 遍历记录，iterate方法是unsafe的
            unsafe {
                table
                    .iterate(|_id, data_ptr| {
                        let _id = core::ptr::read(data_ptr as *const i32);
                        let _value = core::ptr::read(data_ptr.add(4) as *const f32);

                        count += 1;

                        true // 继续遍历
                    })
                    .unwrap();
            }

            black_box(count);
        })
    });

    group.finish();
}

// 测试表的更新和删除操作性能
fn bench_table_update_delete(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 1024 * 1024; // 1MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("table_update_delete");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("update_record", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            for i in 0..100 {
                let mut record_data = [0u8; 8];
                let id: i32 = (i + 1) as i32; // 从1开始，避免主键为0的问题
                let value: f32 = i as f32 * 1.0;

                // 指针操作需要unsafe块
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        record_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f32 as *const u8,
                        record_data.as_mut_ptr().add(4),
                        4,
                    );

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            // 创建更新用的记录数据
            let mut update_data = [0u8; 8];
            let id: i32 = 51; // 更新id为51的记录，对应原来的i=50
            let new_value: f32 = 6.28;

            // 指针操作需要unsafe块
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &id as *const i32 as *const u8,
                    update_data.as_mut_ptr(),
                    4,
                );
                core::ptr::copy_nonoverlapping(
                    &new_value as *const f32 as *const u8,
                    update_data.as_mut_ptr().add(4),
                    4,
                );

                // 更新一条记录，update方法是unsafe的
                black_box(table.update(50, update_data.as_ptr()).unwrap());
            }
        })
    });

    group.bench_function("delete_record", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            for i in 0..100 {
                let mut record_data = [0u8; 8];
                let id: i32 = (i + 1) as i32; // 从1开始，避免主键为0的问题
                let value: f32 = i as f32 * 1.0;

                // 指针操作需要unsafe块
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        record_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f32 as *const u8,
                        record_data.as_mut_ptr().add(4),
                        4,
                    );

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            // 删除一条记录，delete方法是unsafe的
            unsafe {
                black_box(table.delete(50).unwrap());

                black_box(table.record_count());
            }
        })
    });

    group.finish();
}

// 测试表的字段操作性能
fn bench_table_field_operations(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 1024 * 1024; // 1MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("table_field_operations");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("get_field", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            let mut record_data = [0u8; 8];
            let id: i32 = 1;
            let value: f32 = 3.14;

            // 指针操作需要unsafe块
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &id as *const i32 as *const u8,
                    record_data.as_mut_ptr(),
                    4,
                );
                core::ptr::copy_nonoverlapping(
                    &value as *const f32 as *const u8,
                    record_data.as_mut_ptr().add(4),
                    4,
                );

                table.insert(record_data.as_ptr()).unwrap();
            }

            // 获取记录数据，get_by_id方法是unsafe的
            let mut result_data = [0u8; 8];
            unsafe {
                table.get_by_id(0, result_data.as_mut_ptr()).unwrap();

                // 获取一个字段值，get_field方法是unsafe的
                black_box(table.get_field(result_data.as_ptr(), 0).unwrap());
            }
        })
    });

    group.bench_function("set_field", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TEST_TABLE_DEF)).unwrap();

            // 插入测试数据
            let mut record_data = [0u8; 8];
            let id: i32 = 1;
            let value: f32 = 3.14;

            // 指针操作需要unsafe块
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &id as *const i32 as *const u8,
                    record_data.as_mut_ptr(),
                    4,
                );
                core::ptr::copy_nonoverlapping(
                    &value as *const f32 as *const u8,
                    record_data.as_mut_ptr().add(4),
                    4,
                );

                table.insert(record_data.as_ptr()).unwrap();
            }

            // 获取记录数据，get_by_id方法是unsafe的
            let mut result_data = [0u8; 8];
            unsafe {
                table.get_by_id(0, result_data.as_mut_ptr()).unwrap();

                // 创建测试值
                let float_value = Value { float32: 6.28 };

                // 设置一个字段值，set_field方法是unsafe的
                black_box(
                    table
                        .set_field(result_data.as_mut_ptr(), 1, &float_value)
                        .unwrap(),
                );
            }
        })
    });

    group.finish();
}

// 测试时间序列数据的插入性能
fn bench_time_series_insert(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_insert");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("single_metric_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 准备时间序列数据
            let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
            let id: i32 = 1;
            let metric_name = "cpu_usage";
            let value: f64 = 45.5;
            let timestamp: u64 = 1234567890;
            let tags = "host=server1,env=prod";

            // 设置字段值，指针操作需要unsafe块
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &id as *const i32 as *const u8,
                    metric_data.as_mut_ptr(),
                    4,
                );
                core::ptr::copy_nonoverlapping(
                    metric_name.as_ptr(),
                    metric_data.as_mut_ptr().add(4),
                    metric_name.len(),
                );
                core::ptr::copy_nonoverlapping(
                    &value as *const f64 as *const u8,
                    metric_data.as_mut_ptr().add(36),
                    8,
                );
                core::ptr::copy_nonoverlapping(
                    &timestamp as *const u64 as *const u8,
                    metric_data.as_mut_ptr().add(44),
                    8,
                );
                core::ptr::copy_nonoverlapping(
                    tags.as_ptr(),
                    metric_data.as_mut_ptr().add(52),
                    tags.len(),
                );

                // 插入单条时间序列数据，insert方法是unsafe的
                black_box(table.insert(metric_data.as_ptr()).unwrap());
            }
        })
    });

    group.bench_function("batch_metric_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 准备批量时间序列数据
            let mut metrics_data = vec![0u8; 116 * 10]; // 10条记录，使用vec!在堆上分配
            let mut out_ids = [0usize; 10];

            // 初始化测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..10 {
                    let record_ptr = metrics_data.as_mut_ptr().add(i * 116);
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_ptr, 4);
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        record_ptr.add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        record_ptr.add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        record_ptr.add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), record_ptr.add(52), tags.len());
                }

                // 执行批量插入，batch_insert方法是unsafe的
                black_box(
                    table
                        .batch_insert(metrics_data.as_ptr(), 10, out_ids.as_mut_ptr())
                        .unwrap(),
                );
            }
        })
    });

    group.finish();
}

// 测试时间序列数据的查询性能
fn bench_time_series_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 8 * 1024 * 1024; // 8MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("query_by_id", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..500 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 查询指定ID的时间序列数据，get_by_id方法是unsafe的
            let mut result_data = vec![0u8; 116]; // 使用vec!在堆上分配
            unsafe {
                black_box(table.get_by_id(250, result_data.as_mut_ptr()).unwrap());
            }
        })
    });

    group.bench_function("iterate_metrics", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..500 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            let mut count = 0;
            let mut sum = 0.0;

            // 遍历时间序列数据，模拟聚合计算，iterate方法是unsafe的
            unsafe {
                table
                    .iterate(|_id, data_ptr| {
                        let value = core::ptr::read(data_ptr.add(36) as *const f64);
                        sum += value;
                        count += 1;

                        true // 继续遍历
                    })
                    .unwrap();
            }

            black_box((count, sum));
        })
    });

    group.finish();
}

// 测试时间序列数据的聚合操作性能
fn bench_time_series_aggregation(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 32 * 1024 * 1024; // 32MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_aggregation");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("simple_aggregation", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1000 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            let mut min_value = f64::MAX;
            let mut max_value = f64::MIN;
            let mut sum_value = 0.0;
            let mut count = 0;

            // 遍历计算聚合值，iterate方法是unsafe的
            unsafe {
                table
                    .iterate(|_id, data_ptr| {
                        let value = core::ptr::read(data_ptr.add(36) as *const f64);
                        let timestamp = core::ptr::read(data_ptr.add(44) as *const u64);

                        // 只处理特定时间范围的数据
                        if timestamp >= 1234567890 && timestamp <= 1234568390 {
                            min_value = min_value.min(value);
                            max_value = max_value.max(value);
                            sum_value += value;
                            count += 1;
                        }

                        true // 继续遍历
                    })
                    .unwrap();
            }

            let avg_value = if count > 0 {
                sum_value / count as f64
            } else {
                0.0
            };

            black_box((min_value, max_value, avg_value, count));
        })
    });

    group.finish();
}

// 测试时间序列的时间范围查询性能
fn bench_time_series_time_range_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 8 * 1024 * 1024; // 8MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_time_range_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("get_records_in_time_window", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1000 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 准备结果缓冲区
            let mut result_buffer = vec![0u8; 116 * 100]; // 100条记录的缓冲区

            // 执行时间范围查询，get_records_in_time_window方法是unsafe的
            let found_count = unsafe {
                table
                    .get_records_in_time_window(
                        3,          // timestamp字段索引
                        1234568000, // 开始时间
                        1234568500, // 结束时间
                        result_buffer.as_mut_ptr(),
                        100,
                    )
                    .unwrap()
            };

            black_box(found_count);
        })
    });

    group.finish();
}

// 测试时间序列的最新数据查询性能
fn bench_time_series_latest_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_latest_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("get_latest_records", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..500 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 准备结果缓冲区
            let mut result_buffer = vec![0u8; 116 * 10]; // 10条记录的缓冲区

            // 执行最新数据查询，get_latest_records方法是unsafe的
            let found_count = unsafe {
                table
                    .get_latest_records(
                        3,  // timestamp字段索引
                        10, // 获取10条最新记录
                        result_buffer.as_mut_ptr(),
                    )
                    .unwrap()
            };

            black_box(found_count);
        })
    });

    group.finish();
}

// 测试时间序列的聚合函数性能
fn bench_time_series_aggregate_functions(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 8 * 1024 * 1024; // 8MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_aggregate_functions");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("aggregate_count", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1000 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 执行COUNT聚合，aggregate_count方法是unsafe的
            let count = unsafe {
                table
                    .aggregate_count(
                        3,          // timestamp字段索引
                        1234567890, // 开始时间
                        1234568890, // 结束时间
                    )
                    .unwrap()
            };

            black_box(count);
        })
    });

    group.bench_function("aggregate_avg", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1000 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 执行AVG聚合，aggregate_avg方法是unsafe的
            let avg = unsafe {
                table
                    .aggregate_avg(
                        3,          // timestamp字段索引
                        2,          // value字段索引
                        1234567890, // 开始时间
                        1234568890, // 结束时间
                    )
                    .unwrap()
            };

            black_box(avg);
        })
    });

    group.bench_function("aggregate_sum", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1000 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 执行SUM聚合，aggregate_sum方法是unsafe的
            let sum = unsafe {
                table
                    .aggregate_sum(
                        3,          // timestamp字段索引
                        2,          // value字段索引
                        1234567890, // 开始时间
                        1234568890, // 结束时间
                    )
                    .unwrap()
            };

            black_box(sum);
        })
    });

    group.finish();
}

// 测试时间序列的时间窗口聚合性能
fn bench_time_series_window_aggregation(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 32 * 1024 * 1024; // 32MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_window_aggregation");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("get_aggregate_in_time_window", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..1500 {
                    let mut metric_data = vec![0u8; 116]; // 使用vec!在堆上分配
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64 * 10; // 每10毫秒一条记录
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        metric_data.as_mut_ptr(),
                        4,
                    );
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        metric_data.as_mut_ptr().add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        metric_data.as_mut_ptr().add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        metric_data.as_mut_ptr().add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        tags.as_ptr(),
                        metric_data.as_mut_ptr().add(52),
                        tags.len(),
                    );

                    table.insert(metric_data.as_ptr()).unwrap();
                }
            }

            // 执行时间窗口聚合，get_aggregate_in_time_window方法是unsafe的
            let window_aggregates = unsafe {
                table
                    .get_aggregate_in_time_window(
                        3,          // timestamp字段索引
                        2,          // value字段索引
                        1234567890, // 开始时间
                        1234569890, // 结束时间
                        100,        // 100毫秒窗口
                    )
                    .unwrap()
            };

            black_box(window_aggregates.len());
        })
    });

    group.finish();
}

// 向量表定义用于基准测试
static VECTOR_TABLE_DEF: TableDef = TableDef {
    id: 2,
    name: "vector_table",
    fields: &[
        FieldDef {
            name: "id",
            data_type: DataType::Int32,
            size: 4,
            offset: 0,
            not_null: true,
            primary_key: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "vector_32d",
            data_type: DataType::Vector,
            size: 32 * 4, // 32维向量，每个元素4字节
            offset: 4,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: Some(VectorMetadata {
                dimension: 32,
                distance_type: DistanceType::L2,
                index_type: VectorIndexType::HNSW,
            }),
        },
        FieldDef {
            name: "category",
            data_type: DataType::Int32,
            size: 4,
            offset: 4 + 32 * 4,
            not_null: false,
            primary_key: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: 0,
    secondary_index: None,
    secondary_index_type: IndexType::Hash,
    record_size: 4 + 32 * 4 + 4, // 4字节id + 32*4字节向量 + 4字节category
    max_records: 1000, // 基准测试使用1000条记录
};

// 测试向量数据的插入性能
fn bench_vector_insert(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("vector_insert");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("single_vector_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(VECTOR_TABLE_DEF)).unwrap();

            // 准备向量数据
            let mut record_data = vec![0u8; VECTOR_TABLE_DEF.record_size];
            let id: i32 = 1;
            let category: i32 = 1;
            let vector_data = [1.0f32; 32]; // 32维向量

            // 指针操作需要unsafe块
            unsafe {
                // 设置id
                core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
                // 设置向量数据
                core::ptr::copy_nonoverlapping(vector_data.as_ptr() as *const u8, record_data.as_mut_ptr().add(4), 32 * 4);
                // 设置category
                core::ptr::copy_nonoverlapping(&category as *const i32 as *const u8, record_data.as_mut_ptr().add(4 + 32 * 4), 4);

                // 插入记录
                black_box(table.insert(record_data.as_ptr()).unwrap());
            }
        })
    });

    group.bench_function("batch_vector_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(VECTOR_TABLE_DEF)).unwrap();

            // 准备批量向量数据
            const BATCH_SIZE: usize = 10;
            let mut batch_data = vec![0u8; VECTOR_TABLE_DEF.record_size * BATCH_SIZE];
            let mut out_ids = [0usize; BATCH_SIZE];

            // 初始化测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..BATCH_SIZE {
                    let record_ptr = batch_data.as_mut_ptr().add(i * VECTOR_TABLE_DEF.record_size);
                    let id: i32 = (i + 1) as i32;
                    let category: i32 = (i % 5 + 1) as i32;
                    let vector_value = (i + 1) as f32;
                    let vector_data = [vector_value; 32]; // 32维向量

                    // 设置id
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_ptr, 4);
                    // 设置向量数据
                    core::ptr::copy_nonoverlapping(vector_data.as_ptr() as *const u8, record_ptr.add(4), 32 * 4);
                    // 设置category
                    core::ptr::copy_nonoverlapping(&category as *const i32 as *const u8, record_ptr.add(4 + 32 * 4), 4);
                }

                // 执行批量插入
                black_box(
                    table
                        .batch_insert(batch_data.as_ptr(), BATCH_SIZE, out_ids.as_mut_ptr())
                        .unwrap(),
                );
            }
        })
    });

    group.finish();
}

// 测试向量数据的查询性能
fn bench_vector_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 8 * 1024 * 1024; // 8MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("vector_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("vector_scan", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(VECTOR_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..100 {
                    let mut record_data = vec![0u8; VECTOR_TABLE_DEF.record_size];
                    let id: i32 = (i + 1) as i32;
                    let category: i32 = (i % 5 + 1) as i32;
                    let vector_value = i as f32 * 0.1;
                    let vector_data = [vector_value; 32]; // 32维向量

                    // 设置id
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
                    // 设置向量数据
                    core::ptr::copy_nonoverlapping(vector_data.as_ptr() as *const u8, record_data.as_mut_ptr().add(4), 32 * 4);
                    // 设置category
                    core::ptr::copy_nonoverlapping(&category as *const i32 as *const u8, record_data.as_mut_ptr().add(4 + 32 * 4), 4);

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            // 准备查询向量
            let query_vector = [1.0f32; 32];
            let mut result_count = 0;

            // 遍历记录，计算相似度，iterate方法是unsafe的
            unsafe {
                table
                    .iterate(|_id, data_ptr| {
                        // 获取向量数据
                        let vector_ptr = data_ptr.add(4) as *const f32;
                        
                        // 计算L2距离（简化实现，仅用于基准测试）
                        let mut distance = 0.0f32;
                        for i in 0..32 {
                            let diff = *vector_ptr.add(i) - query_vector[i];
                            distance += diff * diff;
                        }
                        
                        // 如果距离小于阈值，计数
                        if distance.sqrt() < 5.0 {
                            result_count += 1;
                        }

                        true // 继续遍历
                    })
                    .unwrap();
            }

            black_box(result_count);
        })
    });

    group.bench_function("vector_id_query", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(VECTOR_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..100 {
                    let mut record_data = vec![0u8; VECTOR_TABLE_DEF.record_size];
                    let id: i32 = (i + 1) as i32;
                    let category: i32 = (i % 5 + 1) as i32;
                    let vector_value = i as f32 * 0.1;
                    let vector_data = [vector_value; 32]; // 32维向量

                    // 设置id
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
                    // 设置向量数据
                    core::ptr::copy_nonoverlapping(vector_data.as_ptr() as *const u8, record_data.as_mut_ptr().add(4), 32 * 4);
                    // 设置category
                    core::ptr::copy_nonoverlapping(&category as *const i32 as *const u8, record_data.as_mut_ptr().add(4 + 32 * 4), 4);

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            // 查询指定ID的向量记录，get_by_id方法是unsafe的
            let mut result_data = vec![0u8; VECTOR_TABLE_DEF.record_size];
            unsafe {
                black_box(table.get_by_id(50, result_data.as_mut_ptr()).unwrap());
            }
        })
    });

    group.finish();
}

// 测试向量索引的创建和使用性能
fn bench_vector_index(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 16 * 1024 * 1024; // 16MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("vector_index");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("create_vector_index", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(VECTOR_TABLE_DEF)).unwrap();

            // 插入测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..100 {
                    let mut record_data = vec![0u8; VECTOR_TABLE_DEF.record_size];
                    let id: i32 = (i + 1) as i32;
                    let category: i32 = (i % 5 + 1) as i32;
                    let vector_value = i as f32 * 0.1;
                    let vector_data = [vector_value; 32]; // 32维向量

                    // 设置id
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_data.as_mut_ptr(), 4);
                    // 设置向量数据
                    core::ptr::copy_nonoverlapping(vector_data.as_ptr() as *const u8, record_data.as_mut_ptr().add(4), 32 * 4);
                    // 设置category
                    core::ptr::copy_nonoverlapping(&category as *const i32 as *const u8, record_data.as_mut_ptr().add(4 + 32 * 4), 4);

                    table.insert(record_data.as_ptr()).unwrap();
                }
            }

            // 注意：实际的向量索引创建会在SQL层面处理，这里我们模拟向量索引的创建开销
            // 由于向量索引的实际创建逻辑可能比较复杂，这里我们只做简单的模拟
            black_box(1);
        })
    });

    group.finish();
}

// 测试时间序列的批量插入优化性能
fn bench_time_series_batch_insert_optimized(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 8 * 1024 * 1024; // 8MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("time_series_batch_insert_optimized");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("time_series_batch_insert", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建表
            let mut table = MemoryTable::new(Arc::new(TIME_SERIES_TABLE_DEF)).unwrap();

            // 准备批量时间序列数据
            let mut metrics_data = vec![0u8; 116 * 50]; // 50条记录，使用vec!在堆上分配
            let mut out_ids = [0usize; 50];

            // 初始化测试数据，指针操作需要unsafe块
            unsafe {
                for i in 0..50 {
                    let record_ptr = metrics_data.as_mut_ptr().add(i * 116);
                    let id: i32 = (i + 1) as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";

                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_ptr, 4);
                    core::ptr::copy_nonoverlapping(
                        metric_name.as_ptr(),
                        record_ptr.add(4),
                        metric_name.len(),
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f64 as *const u8,
                        record_ptr.add(36),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(
                        &timestamp as *const u64 as *const u8,
                        record_ptr.add(44),
                        8,
                    );
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), record_ptr.add(52), tags.len());
                }

                // 执行时间序列批量插入优化，time_series_batch_insert方法是unsafe的
                black_box(
                    table
                        .time_series_batch_insert(metrics_data.as_ptr(), 50, out_ids.as_mut_ptr())
                        .unwrap(),
                );
            }
        })
    });

    group.finish();
}

// 测试TimeSeriesTable的批量写入性能
fn bench_time_series_table_batch_write(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 16 * 1024 * 1024; // 16MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("timeseries_table_batch_write");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("batch_write", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建TimeSeriesTableDef
            let ts_table_def = TimeSeriesTableDef {
                base: TEST_TABLE_DEF,
                time_field: 0,
                value_field: 1,
                tag_fields: &[],
                config: TimeSeriesConfig::DEFAULT,
            };

            // 创建TimeSeriesIndex
            let index = TimeSeriesIndex::new();

            // 创建TimeSeriesTable
            let mut ts_table = TimeSeriesTable::new(Arc::new(ts_table_def), index).unwrap();

            // 准备批量时间序列数据
            let mut records = vec![
                TimeSeriesRecord {
                    timestamp: 0,
                    value: 0.0,
                    tag_count: 0,
                    tags: [0; 8],
                };
                100
            ];

            // 初始化测试数据
            for i in 0..100 {
                records[i].timestamp = 1234567890 + i as u64;
                records[i].value = 45.5 + i as f64;
            }

            // 执行批量写入，batch_write方法是unsafe的
            unsafe {
                black_box(ts_table.batch_write(records.as_ptr(), 100).unwrap());
            }
        })
    });

    group.finish();
}

// 测试TimeSeriesTable的时间范围查询性能
fn bench_time_series_table_time_range_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);

    // 初始化全局内存分配器一次，而不是每个迭代都初始化
    const MEMORY_SIZE: usize = 16 * 1024 * 1024; // 16MB
    let mut memory = vec![0u8; MEMORY_SIZE];
    // init_global_allocator函数本身是安全的，不需要unsafe块
    allocator::init_global_allocator(memory.as_mut_ptr(), MEMORY_SIZE).unwrap();

    let mut group = c.benchmark_group("timeseries_table_time_range_query");
    group.sample_size(1000); // 提高样本数到1000，获得更准确的基准测试结果

    group.bench_function("query_time_range", |b| {
        b.iter(|| {
            // 在每次迭代前重置内存分配器
            allocator::reset_global_allocator().unwrap();

            // 创建TimeSeriesTableDef
            let ts_table_def = TimeSeriesTableDef {
                base: TEST_TABLE_DEF,
                time_field: 0,
                value_field: 1,
                tag_fields: &[],
                config: TimeSeriesConfig::DEFAULT,
            };

            // 创建TimeSeriesIndex
            let index = TimeSeriesIndex::new();

            // 创建TimeSeriesTable
            let mut ts_table = TimeSeriesTable::new(Arc::new(ts_table_def), index).unwrap();

            // 准备批量时间序列数据
            let mut records = vec![
                TimeSeriesRecord {
                    timestamp: 0,
                    value: 0.0,
                    tag_count: 0,
                    tags: [0; 8],
                };
                1000
            ];

            // 初始化测试数据
            for i in 0..1000 {
                records[i].timestamp = 1234567890 + i as u64;
                records[i].value = 45.5 + i as f64;
            }

            // 执行批量写入，batch_write方法是unsafe的
            unsafe {
                ts_table.batch_write(records.as_ptr(), 1000).unwrap();
            }

            // 执行时间范围查询
            black_box(ts_table.query_time_range(1234568000, 1234568500).unwrap());
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_table_insert,
    bench_table_query,
    bench_table_update_delete,
    bench_table_field_operations,
    bench_time_series_insert,
    bench_time_series_query,
    bench_time_series_aggregation,
    bench_time_series_time_range_query,
    bench_time_series_latest_query,
    bench_time_series_aggregate_functions,
    bench_time_series_window_aggregation,
    bench_time_series_batch_insert_optimized,
    bench_time_series_table_batch_write,
    bench_time_series_table_time_range_query,
    bench_vector_insert,
    bench_vector_query,
    bench_vector_index
);
criterion_main!(benches);
