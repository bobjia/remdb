use core::mem::MaybeUninit;
use remdb::*;
use remdb::types::*;
use remdb::platform::*;

// 简单的表定义用于测试
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
    secondary_index_type: IndexType::SortedArray,
    record_size: 8,
    max_records: 100,
};

// 数据库配置
static TEST_DB_CONFIG: config::DbConfig = config::DbConfig {
    tables: &[TEST_TABLE_DEF],
    total_memory: 1024 * 1024, // 1MB
    low_power_mode_supported: false,
    low_power_max_records: None,
};

// 静态缓冲区用于测试
static mut TABLES_BUFFER: [Option<MemoryTable>; 1] = [None];
static mut PRIMARY_INDICES_BUFFER: [Option<PrimaryIndex>; 1] = [None];
static mut SECONDARY_INDICES_BUFFER: [Option<AnySecondaryIndex>; 1] = [None];
static mut TABLE_DATA_BUFFER: [u8; 8 * 100] = [0u8; 8 * 100]; // 8字节记录 * 100条
static mut TABLE_STATUS_BUFFER: [MaybeUninit<RecordHeader>; 100] = [const { MaybeUninit::uninit() }; 100];
static mut TABLE_FREE_SLOTS_BUFFER: [usize; 100] = [0usize; 100];

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
    
    fn file_open(&self, path: &str, mode: FileMode) -> FileResult<FileHandle> {
        // 使用std::fs::File实现文件操作
        use std::fs::File;
        use std::fs::OpenOptions;
        
        match mode {
            FileMode::Read => {
                match OpenOptions::new().read(true).open(path) {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
            },
            FileMode::Write => {
                match OpenOptions::new().write(true).create(true).truncate(true).open(path) {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
            },
            FileMode::ReadWrite => {
                match OpenOptions::new().read(true).write(true).create(true).open(path) {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
            },
            FileMode::Append => {
                match OpenOptions::new().append(true).create(true).open(path) {
                    Ok(file) => Ok(Box::into_raw(Box::new(file)) as FileHandle),
                    Err(_) => Err(()),
                }
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
fn test_snapshot_gen() -> Result<()> {
    unsafe {
        // 初始化平台
        init_platform(&TEST_PLATFORM);
        
        // 重置事务管理器
        transaction::TX_MANAGER.reset();
        
        // 重置缓冲区
        TABLES_BUFFER[0] = None;
        PRIMARY_INDICES_BUFFER[0] = None;
        SECONDARY_INDICES_BUFFER[0] = None;
        TABLE_DATA_BUFFER.fill(0);
        
        // 初始化TABLE_STATUS_BUFFER
        for i in 0..100 {
            TABLE_STATUS_BUFFER[i].write(RecordHeader {
                status: RecordStatus::Free,
                version: 0,
                lock_type: LockType::None,
                lock_owner: 0,
                lock_count: 0
            });
        }
        
        // 创建表
        let table = MemoryTable::new(
            &TEST_TABLE_DEF,
            TABLE_DATA_BUFFER.as_mut_ptr(),
            TABLE_STATUS_BUFFER.as_mut_ptr() as *mut RecordHeader,
            TABLE_FREE_SLOTS_BUFFER.as_mut_ptr()
        ).unwrap();
        TABLES_BUFFER[0] = Some(table);
        
        // 创建数据库实例
        let mut db = RemDb::new(
            &TEST_DB_CONFIG,
            &mut TABLES_BUFFER,
            &mut PRIMARY_INDICES_BUFFER,
            &mut SECONDARY_INDICES_BUFFER
        );
        
        // 插入测试数据
        for i in 0..5 {
            let mut record_data = [0u8; 8];
            let id: i32 = i as i32;
            let value: f32 = i as f32 * 10.0;
            
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
            
            db.get_table_mut(0).unwrap().insert(record_data.as_ptr()).unwrap();
        }
        
        // 保存快照到文件
        db.save_snapshot("test_snapshot.remd")?;
        
        println!("快照文件已生成: test_snapshot.remd");
        
        Ok(())
    }
}
