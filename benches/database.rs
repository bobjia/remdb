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
    record_size: 8,
    max_records: 100, // 基准测试使用较小的记录数
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

criterion_group!(benches, bench_table_insert, bench_table_query, bench_table_update_delete, bench_table_field_operations);
criterion_main!(benches);