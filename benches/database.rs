use criterion::{black_box, criterion_group, criterion_main, Criterion};
use remdb::table::*;
use remdb::types::*;
use remdb::platform::*;

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
        },
        FieldDef {
            name: "metric_name",
            data_type: DataType::String,
            size: 32,
            offset: 4,
        },
        FieldDef {
            name: "value",
            data_type: DataType::Float64,
            size: 8,
            offset: 36,
        },
        FieldDef {
            name: "timestamp",
            data_type: DataType::Timestamp,
            size: 8,
            offset: 44,
        },
        FieldDef {
            name: "tags",
            data_type: DataType::String,
            size: 64,
            offset: 52,
        },
    ],
    primary_key: 0,
    secondary_index: Some(3), // 时间戳作为辅助索引
    secondary_index_type: IndexType::SortedArray,
    record_size: 116, // 4 + 32 + 8 + 8 + 64 = 116字节
    max_records: 1000, // 时间序列测试使用较大的记录数
};

// 测试表的插入操作性能
fn bench_table_insert(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("table_insert");
    group.sample_size(1000);
    
    group.bench_function("single_insert", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            // 创建测试记录
            let mut record_data = [0u8; 8];
            let id: i32 = 1;
            let value: f32 = 3.14;
            
            unsafe {
                // 初始化测试记录
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
                
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 只插入一条记录进行测试
                black_box(table.insert(record_data.as_ptr()).unwrap());
            }
        })
    });
    
    group.bench_function("batch_insert", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            // 创建测试记录数组
            let mut records = [0u8; 8 * 10]; // 10条记录
            let mut out_ids = [0usize; 10];
            
            unsafe {
                // 初始化测试记录数组
                for i in 0..10 {
                    let record_ptr = records.as_mut_ptr().add(i * 8);
                    let id: i32 = i as i32;
                    let value: f32 = i as f32 * 1.0;
                    
                    core::ptr::copy_nonoverlapping(
                        &id as *const i32 as *const u8,
                        record_ptr,
                        4
                    );
                    core::ptr::copy_nonoverlapping(
                        &value as *const f32 as *const u8,
                        record_ptr.add(4),
                        4
                    );
                }
                
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 执行批量插入
                black_box(table.batch_insert(records.as_ptr(), 10, out_ids.as_mut_ptr()).unwrap());
            }
        })
    });
    
    group.finish();
}

// 测试表的查询操作性能
fn bench_table_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("table_query");
    group.sample_size(100); // 减少样本大小，避免内存问题
    
    group.bench_function("get_by_id", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..100 {
                    let mut record_data = [0u8; 8];
                    let id: i32 = i as i32;
                    let value: f32 = i as f32 * 1.0;
                    
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
                    
                    table.insert(record_data.as_ptr()).unwrap();
                }
                
                let mut result_data = [0u8; 8];
                
                // 查询一条记录
                black_box(table.get_by_id(50, result_data.as_mut_ptr()).unwrap());
            }
        })
    });
    
    group.bench_function("iterate", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..100 {
                    let mut record_data = [0u8; 8];
                    let id: i32 = i as i32;
                    let value: f32 = i as f32 * 1.0;
                    
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
                    
                    table.insert(record_data.as_ptr()).unwrap();
                }
                
                let mut count = 0;
                
                // 遍历记录
                table.iterate(|_id, data_ptr| {
                    let _id = core::ptr::read(data_ptr as *const i32);
                    let _value = core::ptr::read(data_ptr.add(4) as *const f32);
                    
                    count += 1;
                    
                    true // 继续遍历
                }).unwrap();
                
                black_box(count);
            }
        })
    });
    
    group.finish();
}

// 测试表的更新和删除操作性能
fn bench_table_update_delete(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("table_update_delete");
    group.sample_size(100); // 减少样本大小，避免内存问题
    
    group.bench_function("update_record", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..100 {
                    let mut record_data = [0u8; 8];
                    let id: i32 = i as i32;
                    let value: f32 = i as f32 * 1.0;
                    
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
                    
                    table.insert(record_data.as_ptr()).unwrap();
                }
                
                // 创建更新用的记录数据
                let mut update_data = [0u8; 8];
                let id: i32 = 1;
                let new_value: f32 = 6.28;
                
                core::ptr::copy_nonoverlapping(
                    &id as *const i32 as *const u8,
                    update_data.as_mut_ptr(),
                    4
                );
                core::ptr::copy_nonoverlapping(
                    &new_value as *const f32 as *const u8,
                    update_data.as_mut_ptr().add(4),
                    4
                );
                
                // 更新一条记录
                black_box(table.update(50, update_data.as_ptr()).unwrap());
            }
        })
    });
    
    group.bench_function("delete_record", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..100 {
                    let mut record_data = [0u8; 8];
                    let id: i32 = i as i32;
                    let value: f32 = i as f32 * 1.0;
                    
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
                    
                    table.insert(record_data.as_ptr()).unwrap();
                }
                
                // 删除一条记录
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
    
    let mut group = c.benchmark_group("table_field_operations");
    group.sample_size(100); // 减少样本大小，避免内存问题
    
    group.bench_function("get_field", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
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
                
                table.insert(record_data.as_ptr()).unwrap();
                
                // 获取记录数据
                let mut result_data = [0u8; 8];
                table.get_by_id(0, result_data.as_mut_ptr()).unwrap();
                
                // 获取一个字段值
                black_box(table.get_field(result_data.as_ptr(), 0).unwrap());
            }
        })
    });
    
    group.bench_function("set_field", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 8 * 100]; // 8字节记录 * 100条
            let mut status_buffer: [RecordHeader; 100] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 100];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TEST_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
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
                
                table.insert(record_data.as_ptr()).unwrap();
                
                // 获取记录数据
                let mut result_data = [0u8; 8];
                table.get_by_id(0, result_data.as_mut_ptr()).unwrap();
                
                // 创建测试值
                let float_value = Value { float32: 6.28 };
                
                // 设置一个字段值
                black_box(table.set_field(result_data.as_mut_ptr(), 1, &float_value).unwrap());
            }
        })
    });
    
    group.finish();
}

// 测试时间序列数据的插入性能
fn bench_time_series_insert(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("time_series_insert");
    group.sample_size(500);
    
    group.bench_function("single_metric_insert", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 116 * 1000]; // 116字节记录 * 1000条
            let mut status_buffer: [RecordHeader; 1000] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 1000];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TIME_SERIES_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 准备时间序列数据
                let mut metric_data = [0u8; 116];
                let id: i32 = 1;
                let metric_name = "cpu_usage";
                let value: f64 = 45.5;
                let timestamp: u64 = 1234567890;
                let tags = "host=server1,env=prod";
                
                // 设置字段值
                core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, metric_data.as_mut_ptr(), 4);
                core::ptr::copy_nonoverlapping(metric_name.as_ptr(), metric_data.as_mut_ptr().add(4), metric_name.len());
                core::ptr::copy_nonoverlapping(&value as *const f64 as *const u8, metric_data.as_mut_ptr().add(36), 8);
                core::ptr::copy_nonoverlapping(&timestamp as *const u64 as *const u8, metric_data.as_mut_ptr().add(44), 8);
                core::ptr::copy_nonoverlapping(tags.as_ptr(), metric_data.as_mut_ptr().add(52), tags.len());
                
                // 插入单条时间序列数据
                black_box(table.insert(metric_data.as_ptr()).unwrap());
            }
        })
    });
    
    group.bench_function("batch_metric_insert", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 116 * 1000]; // 116字节记录 * 1000条
            let mut status_buffer: [RecordHeader; 1000] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 1000];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TIME_SERIES_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 准备批量时间序列数据
                let mut metrics_data = [0u8; 116 * 10]; // 10条记录
                let mut out_ids = [0usize; 10];
                
                // 初始化测试数据
                for i in 0..10 {
                    let record_ptr = metrics_data.as_mut_ptr().add(i * 116);
                    let id: i32 = i as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";
                    
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, record_ptr, 4);
                    core::ptr::copy_nonoverlapping(metric_name.as_ptr(), record_ptr.add(4), metric_name.len());
                    core::ptr::copy_nonoverlapping(&value as *const f64 as *const u8, record_ptr.add(36), 8);
                    core::ptr::copy_nonoverlapping(&timestamp as *const u64 as *const u8, record_ptr.add(44), 8);
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), record_ptr.add(52), tags.len());
                }
                
                // 执行批量插入
                black_box(table.batch_insert(metrics_data.as_ptr(), 10, out_ids.as_mut_ptr()).unwrap());
            }
        })
    });
    
    group.finish();
}

// 测试时间序列数据的查询性能
fn bench_time_series_query(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("time_series_query");
    group.sample_size(200);
    
    group.bench_function("query_by_id", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 116 * 1000]; // 116字节记录 * 1000条
            let mut status_buffer: [RecordHeader; 1000] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 1000];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TIME_SERIES_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..500 {
                    let mut metric_data = [0u8; 116];
                    let id: i32 = i as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";
                    
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, metric_data.as_mut_ptr(), 4);
                    core::ptr::copy_nonoverlapping(metric_name.as_ptr(), metric_data.as_mut_ptr().add(4), metric_name.len());
                    core::ptr::copy_nonoverlapping(&value as *const f64 as *const u8, metric_data.as_mut_ptr().add(36), 8);
                    core::ptr::copy_nonoverlapping(&timestamp as *const u64 as *const u8, metric_data.as_mut_ptr().add(44), 8);
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), metric_data.as_mut_ptr().add(52), tags.len());
                    
                    table.insert(metric_data.as_ptr()).unwrap();
                }
                
                let mut result_data = [0u8; 116];
                
                // 查询指定ID的时间序列数据
                black_box(table.get_by_id(250, result_data.as_mut_ptr()).unwrap());
            }
        })
    });
    
    group.bench_function("iterate_metrics", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 116 * 1000]; // 116字节记录 * 1000条
            let mut status_buffer: [RecordHeader; 1000] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 1000];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TIME_SERIES_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..500 {
                    let mut metric_data = [0u8; 116];
                    let id: i32 = i as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 45.5 + i as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";
                    
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, metric_data.as_mut_ptr(), 4);
                    core::ptr::copy_nonoverlapping(metric_name.as_ptr(), metric_data.as_mut_ptr().add(4), metric_name.len());
                    core::ptr::copy_nonoverlapping(&value as *const f64 as *const u8, metric_data.as_mut_ptr().add(36), 8);
                    core::ptr::copy_nonoverlapping(&timestamp as *const u64 as *const u8, metric_data.as_mut_ptr().add(44), 8);
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), metric_data.as_mut_ptr().add(52), tags.len());
                    
                    table.insert(metric_data.as_ptr()).unwrap();
                }
                
                let mut count = 0;
                let mut sum = 0.0;
                
                // 遍历时间序列数据，模拟聚合计算
                table.iterate(|_id, data_ptr| {
                    let value = core::ptr::read(data_ptr.add(36) as *const f64);
                    sum += value;
                    count += 1;
                    
                    true // 继续遍历
                }).unwrap();
                
                black_box((count, sum));
            }
        })
    });
    
    group.finish();
}

// 测试时间序列数据的聚合操作性能
fn bench_time_series_aggregation(c: &mut Criterion) {
    // 初始化平台
    init_platform(&TEST_PLATFORM);
    
    let mut group = c.benchmark_group("time_series_aggregation");
    group.sample_size(100);
    
    group.bench_function("simple_aggregation", |b| {
        b.iter(|| {
            // 每个迭代创建新的内存缓冲区和表实例
            let mut data_buffer = [0u8; 116 * 1000]; // 116字节记录 * 1000条
            let mut status_buffer: [RecordHeader; 1000] = core::array::from_fn(|_| RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
            let mut free_slots_buffer = [0usize; 1000];
            
            unsafe {
                // 创建表
                let mut table = MemoryTable::new(
                    &TIME_SERIES_TABLE_DEF,
                    data_buffer.as_mut_ptr(),
                    status_buffer.as_mut_ptr(),
                    free_slots_buffer.as_mut_ptr()
                ).unwrap();
                
                // 插入测试数据
                for i in 0..1000 {
                    let mut metric_data = [0u8; 116];
                    let id: i32 = i as i32;
                    let metric_name = "cpu_usage";
                    let value: f64 = 40.0 + (i % 20) as f64;
                    let timestamp: u64 = 1234567890 + i as u64;
                    let tags = "host=server1,env=prod";
                    
                    core::ptr::copy_nonoverlapping(&id as *const i32 as *const u8, metric_data.as_mut_ptr(), 4);
                    core::ptr::copy_nonoverlapping(metric_name.as_ptr(), metric_data.as_mut_ptr().add(4), metric_name.len());
                    core::ptr::copy_nonoverlapping(&value as *const f64 as *const u8, metric_data.as_mut_ptr().add(36), 8);
                    core::ptr::copy_nonoverlapping(&timestamp as *const u64 as *const u8, metric_data.as_mut_ptr().add(44), 8);
                    core::ptr::copy_nonoverlapping(tags.as_ptr(), metric_data.as_mut_ptr().add(52), tags.len());
                    
                    table.insert(metric_data.as_ptr()).unwrap();
                }
                
                let mut min_value = f64::MAX;
                let mut max_value = f64::MIN;
                let mut sum_value = 0.0;
                let mut count = 0;
                
                // 遍历计算聚合值
                table.iterate(|_id, data_ptr| {
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
                }).unwrap();
                
                let avg_value = if count > 0 { sum_value / count as f64 } else { 0.0 };
                
                black_box((min_value, max_value, avg_value, count));
            }
        })
    });
    
    group.finish();
}

criterion_group!(benches, bench_table_insert, bench_table_query, bench_table_update_delete, bench_table_field_operations, bench_time_series_insert, bench_time_series_query, bench_time_series_aggregation);
criterion_main!(benches);