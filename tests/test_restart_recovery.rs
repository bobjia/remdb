use remdb::config::*;
use remdb::platform::*;
use remdb::*;
use std::fs;
use std::path::Path;
use std::string::ToString;

// 简单的表定义用于测试
static TEST_TABLE_DEF: std::sync::LazyLock<TableDef> = std::sync::LazyLock::new(|| TableDef {
    id: 0,
    name: "test_table".to_string(),
    fields: vec![
        FieldDef {
            name: "id".to_string(),
            data_type: DataType::UInt32,
            size: 4,
            offset: 0,
            primary_key: true,
            not_null: true,
            unique: true,
            auto_increment: true,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "name".to_string(),
            data_type: DataType::String,
            size: 32,
            offset: 4,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
        FieldDef {
            name: "value".to_string(),
            data_type: DataType::Float32,
            size: 4,
            offset: 36,
            primary_key: false,
            not_null: false,
            unique: false,
            auto_increment: false,
            default_value: None,
            vector_metadata: None,
        },
    ],
    primary_key: vec![0],
    secondary_index: None,
    secondary_index_type: IndexType::SortedArray,
    record_size: 40,
    max_records: 100,
    version: 1,
    created_at: 0,
    updated_at: 0,
});

// 数据库配置
static TEST_DB_CONFIG: std::sync::LazyLock<DbConfig> = std::sync::LazyLock::new(|| DbConfig {
    tables: vec![TEST_TABLE_DEF.clone()],
    total_memory: 1024 * 1024, // 1MB
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 100000,
    memory_allocator: unsafe {
        static mut DEFAULT_ALLOCATOR: DefaultMemoryAllocator = DefaultMemoryAllocator;
        &mut DEFAULT_ALLOCATOR
    },
    wal_config: WALConfig {
        log_path: "./test_wal",
        log_mode: LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
    },
    time_series_defaults: TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: Some(HAConfig {
        node_id: 1,
        ha_role: remdb::ha::HARole::Auto,
        replication_mode: remdb::ha::ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
        replication_port: 6668,
    }),
});

// 测试平台实现
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

    fn file_open(&self, path: &str, mode: FileMode) -> FileResult<FileHandle> {
        // 使用std::fs::OpenOptions实现文件操作
        use std::fs::OpenOptions;

        // 确保目录存在
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).ok();
        }

        match mode {
            FileMode::Read => match OpenOptions::new().read(true).open(path) {
                Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                Err(_) => Err(()),
            },
            FileMode::Write => {
                match OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)
                {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
            }
            FileMode::ReadWrite => {
                match OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(path)
                {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
            }
            FileMode::Append => match OpenOptions::new().append(true).create(true).open(path) {
                Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                Err(_) => Err(()),
            },
        }
    }

    fn file_close(&self, handle: FileHandle) -> FileResult<()> {
        // 使用std::fs::File实现文件关闭
        let _file = unsafe { Box::from_raw(handle as *mut std::fs::File) };
        Ok(())
    }

    fn file_write(&self, handle: FileHandle, buffer: *const u8, size: usize) -> FileResult<usize> {
        // 使用std::fs::File实现文件写入
        use std::io::Write;

        let file = unsafe { &mut *(handle as *mut std::fs::File) };
        let slice = unsafe { std::slice::from_raw_parts(buffer, size) };

        match file.write(slice) {
            Ok(n) => Ok(n),
            Err(_) => Err(()),
        }
    }

    fn file_read(&self, handle: FileHandle, buffer: *mut u8, size: usize) -> FileResult<usize> {
        // 使用std::fs::File实现文件读取
        use std::io::Read;

        let file = unsafe { &mut *(handle as *mut std::fs::File) };
        let slice = unsafe { std::slice::from_raw_parts_mut(buffer, size) };

        match file.read(slice) {
            Ok(n) => Ok(n),
            Err(_) => Err(()),
        }
    }

    fn file_seek(&self, handle: FileHandle, offset: i64, whence: SeekWhence) -> FileResult<u64> {
        // 使用std::fs::File实现文件定位
        use std::io::Seek;

        let file = unsafe { &mut *(handle as *mut std::fs::File) };
        let seek_from = match whence {
            SeekWhence::SeekSet => std::io::SeekFrom::Start(offset as u64),
            SeekWhence::SeekCur => std::io::SeekFrom::Current(offset),
            SeekWhence::SeekEnd => std::io::SeekFrom::End(offset),
        };

        match file.seek(seek_from) {
            Ok(pos) => Ok(pos),
            Err(_) => Err(()),
        }
    }

    fn file_remove(&self, path: &str) -> FileResult<()> {
        // 使用std::fs::remove_file实现文件删除
        use std::fs::remove_file;

        match remove_file(path) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    fn file_size(&self, path: &str) -> FileResult<usize> {
        // 使用std::fs::metadata实现文件大小获取
        use std::fs::metadata;

        match metadata(path) {
            Ok(meta) => Ok(meta.len() as usize),
            Err(_) => Err(()),
        }
    }

    fn crc32(&self, data: *const u8, size: usize) -> u32 {
        // 简单的XOR校验和实现
        let slice = unsafe { std::slice::from_raw_parts(data, size) };
        let mut checksum = 0u32;
        for &byte in slice {
            checksum ^= byte as u32;
        }
        checksum
    }
}

static TEST_PLATFORM: TestPlatform = TestPlatform;

#[test]
fn test_restart_recovery() -> Result<()> {
    // 清理测试目录
    let _ = fs::remove_dir_all("./test_snapshots");
    let _ = fs::remove_dir_all("./test_wal");

    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);

        // 预分配内存缓冲区并初始化全局分配器
        let mut memory_buffer = Vec::with_capacity(1000000); // 1MB
        memory_buffer.set_len(1000000);
        remdb::memory::allocator::init_global_allocator(memory_buffer.as_mut_ptr(), 1000000)
            .unwrap();

        // 重置全局数据库实例和事务管理器
        remdb::reset_global_db();
        crate::transaction::init_tx_manager();

        // 使用init_global_db初始化数据库
        let db = remdb::init_global_db(&TEST_DB_CONFIG)?;

        // 插入测试数据
        let test_data = [
            (1u32, "item1", 10.5f32),
            (2u32, "item2", 20.7f32),
            (3u32, "item3", 30.9f32),
            (4u32, "item4", 40.1f32),
            (5u32, "item5", 50.3f32),
        ];

        for (id, name, value) in test_data {
            let mut record_data = [0u8; 40];

            // 写入id
            core::ptr::copy_nonoverlapping(
                &id as *const u32 as *const u8,
                record_data.as_mut_ptr(),
                4,
            );

            // 写入name
            let name_bytes = name.as_bytes();
            let name_len = name_bytes.len().min(32);
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                record_data.as_mut_ptr().add(4),
                name_len,
            );

            // 写入value
            core::ptr::copy_nonoverlapping(
                &value as *const f32 as *const u8,
                record_data.as_mut_ptr().add(36),
                4,
            );

            db.get_table_mut(0)
                .unwrap()
                .insert(record_data.as_ptr())
                .unwrap();
        }

        // 保存全量快照
        fs::create_dir_all("./test_snapshots").ok();
        db.save_snapshot("./test_snapshots/full_test.remd")?;

        println!("第一阶段：写入数据并保存快照完成");

        // 重置全局数据库实例和事务管理器
        remdb::reset_global_db();
        crate::transaction::init_tx_manager();

        // 重新初始化数据库
        let db = remdb::init_global_db(&TEST_DB_CONFIG)?;

        // 加载快照
        db.restore_snapshot("./test_snapshots/full_test.remd")?;

        println!("第二阶段：重启数据库并恢复快照完成");

        // 验证数据一致性
        let table = db.get_table(0).unwrap();
        let mut count = 0;

        // 使用iterate方法遍历表中的所有记录
        table
            .iterate(|_id, record_ptr| {
                count += 1;

                // 读取记录数据
                let record_data = core::slice::from_raw_parts(record_ptr, 40);
                let id = core::ptr::read_unaligned(record_data.as_ptr() as *const u32);
                let name_bytes = &record_data[4..36];
                let name = std::str::from_utf8(name_bytes)
                    .unwrap()
                    .split_terminator('\0')
                    .next()
                    .unwrap();
                let value = core::ptr::read_unaligned(record_data.as_ptr().add(36) as *const f32);

                println!("恢复的数据: id={}, name={}, value={}", id, name, value);

                // 验证数据正确性
                assert!(id >= 1 && id <= 5, "Invalid id: {}", id);
                assert!(name.starts_with("item"), "Invalid name: {}", name);
                assert!(value >= 10.0 && value <= 60.0, "Invalid value: {}", value);

                true // 继续迭代
            })
            .unwrap();

        // 验证记录数量
        assert_eq!(count, 5, "Expected 5 records, got {}", count);

        println!("第三阶段：数据一致性验证完成，共恢复 {} 条记录", count);
    }

    // 清理测试目录
    let _ = fs::remove_dir_all("./test_snapshots");
    let _ = fs::remove_dir_all("./test_wal");

    println!("测试完成，所有数据一致性验证通过！");
    Ok(())
}
