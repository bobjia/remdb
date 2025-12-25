use remdb::{database, table, Result}; use remdb::types::RecordHeader;

// 定义测试表
table!(
    TEST_TABLE,
    100, // 最大记录数
    primary_key: id,
    secondary_index: name,
    fields: {
        id: i64,
        name: str(20),
        value: i32
    }
);

// 定义数据库
database!(
    TEST_DB,
    tables: [TEST_TABLE]
);

// 手动计算表所需的总内存大小
// 记录大小：id(8字节) + name(20字节) + value(4字节) = 32字节
const RECORD_SIZE: usize = 8 + 20 + 4;
const TABLE_DATA_SIZE: usize = RECORD_SIZE * TEST_TABLE.max_records;
const STATUS_ARRAY_SIZE: usize = core::mem::size_of::<RecordHeader>() * TEST_TABLE.max_records;
const FREE_SLOTS_SIZE: usize = core::mem::size_of::<usize>() * TEST_TABLE.max_records;
const TABLE_MEM_SIZE: usize = TABLE_DATA_SIZE + STATUS_ARRAY_SIZE + FREE_SLOTS_SIZE;

// 静态变量，具有'static生命周期
static mut TABLE_MEM: [u8; TABLE_MEM_SIZE] = [0; TABLE_MEM_SIZE];

// 静态表数组，具有'static生命周期
static mut TABLES: [Option<remdb::MemoryTable>; 8] = [const { None }; 8];
static mut PRIMARY_INDICES: [Option<remdb::PrimaryIndex>; 8] = [const { None }; 8];
static mut SECONDARY_INDICES: [Option<remdb::AnySecondaryIndex>; 8] = [const { None }; 8];

fn main() -> Result<()> 
{
    // 初始化平台
    struct SimplePlatform;
    
    impl remdb::platform::Platform for SimplePlatform {
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
            // 简单的忙等待延迟
            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < ms as u128 {
                core::hint::spin_loop();
            }
        }
        
        fn delay_us(&self, us: u32) {
            // 简单的忙等待延迟
            let start = std::time::Instant::now();
            while start.elapsed().as_micros() < us as u128 {
                core::hint::spin_loop();
            }
        }
        
        fn file_open(&self, path: &str, mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
            // 使用std::fs::File实现文件操作
            use std::fs::OpenOptions;
            
            let file = match mode {
                remdb::platform::FileMode::Read => OpenOptions::new().read(true).open(path),
                remdb::platform::FileMode::Write => OpenOptions::new().write(true).create(true).truncate(true).open(path),
                remdb::platform::FileMode::ReadWrite => OpenOptions::new().read(true).write(true).create(true).open(path),
                remdb::platform::FileMode::Append => OpenOptions::new().append(true).create(true).open(path),
            };
            
            match file {
                Ok(file) => {
                    let file_ptr = Box::into_raw(Box::new(file)) as remdb::platform::FileHandle;
                    Ok(file_ptr)
                },
                Err(_) => Err(()),
            }
        }
        
        fn file_close(&self, handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
            // 使用std::fs::File实现文件关闭
            let _file = unsafe { Box::from_raw(handle as *mut std::fs::File) };
            Ok(())
        }
        
        fn file_write(&self, handle: remdb::platform::FileHandle, buffer: *const u8, size: usize) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件写入
            use std::io::Write;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let slice = unsafe { std::slice::from_raw_parts(buffer, size) };
            
            match file.write(slice) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }
        
        fn file_read(&self, handle: remdb::platform::FileHandle, buffer: *mut u8, size: usize) -> remdb::platform::FileResult<usize> {
            // 使用std::fs::File实现文件读取
            use std::io::Read;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let slice = unsafe { std::slice::from_raw_parts_mut(buffer, size) };
            
            match file.read(slice) {
                Ok(n) => Ok(n),
                Err(_) => Err(()),
            }
        }
        
        fn file_seek(&self, handle: remdb::platform::FileHandle, offset: i64, whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
            // 使用std::fs::File实现文件定位
            use std::io::Seek;
            
            let file = unsafe { &mut *(handle as *mut std::fs::File) };
            let seek_from = match whence {
                remdb::platform::SeekWhence::SeekSet => std::io::SeekFrom::Start(offset as u64),
                remdb::platform::SeekWhence::SeekCur => std::io::SeekFrom::Current(offset),
                remdb::platform::SeekWhence::SeekEnd => std::io::SeekFrom::End(offset),
            };
            
            match file.seek(seek_from) {
                Ok(pos) => Ok(pos),
                Err(_) => Err(()),
            }
        }
        
        fn file_remove(&self, path: &str) -> remdb::platform::FileResult<()> {
            // 使用std::fs::remove_file实现文件删除
            use std::fs::remove_file;
            
            match remove_file(path) {
                Ok(_) => Ok(()),
                Err(_) => Err(()),
            }
        }
        
        fn file_size(&self, path: &str) -> remdb::platform::FileResult<usize> {
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
    
    static SIMPLE_PLATFORM: SimplePlatform = SimplePlatform;
    
    unsafe {
        // 初始化平台
        remdb::platform::init_platform(&SIMPLE_PLATFORM);
        // 初始化表
        let table_ptr = TABLE_MEM.as_mut_ptr();
        let status_ptr = table_ptr.add(TABLE_DATA_SIZE);
        let free_slots_ptr = status_ptr.add(STATUS_ARRAY_SIZE);
        
        // 初始化表，MemoryTable::new返回Option<MemoryTable>
        TABLES[0] = remdb::MemoryTable::new(
            &TEST_TABLE,
            table_ptr,
            status_ptr as *mut RecordHeader,
            free_slots_ptr as *mut usize
        );
        
        // 初始化数据库
        let db = remdb::init_global_db(
            &TEST_DB,
            &mut TABLES,
            &mut PRIMARY_INDICES,
            &mut SECONDARY_INDICES
        )?;
        
        // 插入测试数据
        println!("插入测试数据...");
        for i in 0..10 {
            // 创建对齐的记录数据
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
            let mut record = AlignedRecord([0; 32]);
            
            // 获取表引用
            let table = db.get_table_mut(0)?;
            
            // 设置字段值
            let value = remdb::Value { int64: i as i64 };
            table.set_field(record.0.as_mut_ptr(), 0, &value)?;
            
            let name = format!("item_{}", i);
            let name_value = remdb::Value { string: { 
                let mut s = [0u8; 64];
                // 填充name，剩余空间用0填充
                for (i, c) in name.as_bytes().iter().enumerate() {
                    if i < s.len() {
                        s[i] = *c;
                    } else {
                        break;
                    }
                }
                s
            } };
            table.set_field(record.0.as_mut_ptr(), 1, &name_value)?;
            
            let value_value = remdb::Value { int32: i * 100 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?;
            
            // 插入记录
            let record_id = table.insert(record.0.as_ptr())?;
            println!("插入记录ID: {}", record_id);
        }
        
        // 保存完整快照
        println!("保存完整快照到 full_snapshot.remd...");
        db.save_snapshot("full_snapshot.remd")?;
        println!("完整快照保存成功");
        
        // 修改部分数据
        println!("\n修改部分数据...");
        {
            let table = db.get_table_mut(0)?;
            
            // 修改第5条记录
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]);
            let mut record = AlignedRecord([0; 32]);
            
            table.get_by_id(5, record.0.as_mut_ptr())?;
            let value_value = remdb::Value { int32: 5555 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?;
            
            table.delete(5)?;
            table.insert(record.0.as_ptr())?;
            
            // 新增一条记录
            let mut new_record = AlignedRecord([0; 32]);
            let id_value = remdb::Value { int64: 10 };
            table.set_field(new_record.0.as_mut_ptr(), 0, &id_value)?;
            
            let name = "item_10";
            let name_value = remdb::Value { string: {
                let mut s = [0u8; 64];
                for (i, c) in name.as_bytes().iter().enumerate() {
                    if i < s.len() {
                        s[i] = *c;
                    } else {
                        break;
                    }
                }
                s
            } };
            table.set_field(new_record.0.as_mut_ptr(), 1, &name_value)?;
            
            let value_value = remdb::Value { int32: 1000 };
            table.set_field(new_record.0.as_mut_ptr(), 2, &value_value)?;
            
            table.insert(new_record.0.as_ptr())?;
            println!("修改了第5条记录，新增了第10条记录");
        }
        
        // 保存增量快照
        println!("保存增量快照到 incremental_snapshot.remd...");
        db.save_incremental_snapshot("incremental_snapshot.remd")?;
        println!("增量快照保存成功");
        
        // 再次修改数据
        println!("\n再次修改数据...");
        {
            let table = db.get_table_mut(0)?;
            
            // 修改第6条记录
            #[repr(align(8))]
            struct AlignedRecord([u8; 32]);
            let mut record = AlignedRecord([0; 32]);
            
            table.get_by_id(6, record.0.as_mut_ptr())?;
            let value_value = remdb::Value { int32: 6666 };
            table.set_field(record.0.as_mut_ptr(), 2, &value_value)?;
            
            table.delete(6)?;
            table.insert(record.0.as_ptr())?;
            println!("修改了第6条记录");
        }
        
        // 恢复增量快照
        println!("\n恢复增量快照...");
        db.restore_snapshot("incremental_snapshot.remd")?;
        println!("增量快照恢复成功");
        
        // 验证增量快照恢复后的数据
        println!("\n验证增量快照恢复后的数据...");
        let table = db.get_table(0)?;
        
        // 遍历所有记录，检查已使用的记录
        let mut used_records_after_inc = 0;
        let mut found_modified_record = false;
        let mut found_new_record = false;
        
        for i in 0..table.max_records() {
            let status_ptr = table.get_status_ptr(i);
            if (*status_ptr).status == remdb::types::RecordStatus::Used {
                // 创建对齐的记录数据
                #[repr(align(8))]
                struct AlignedRecord([u8; 32]); // 手动指定32字节大小（id:8 + name:20 + value:4）
                let mut record = AlignedRecord([0; 32]);
                
                let record_ptr = table.get_record_ptr(i);
                remdb::platform::memcpy(record.0.as_mut_ptr(), record_ptr, 32);
                
                // 获取字段值
                let id_value = table.get_field(record.0.as_ptr(), 0)?;
                let name_value = table.get_field(record.0.as_ptr(), 1)?;
                let value_value = table.get_field(record.0.as_ptr(), 2)?;
                
                // 提取值
                let id = id_value.int64 as u64;
                let name_bytes = name_value.string.as_slice();
                let name_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(name_bytes.len());
                let name = std::str::from_utf8(&name_bytes[..name_len]).unwrap_or("invalid_utf8");
                let value = value_value.int32;
                
                println!("记录索引 {}: id={}, name={:?}, value={}", i, id, name, value);
                
                // 检查修改后的记录
                if id == 5 && value == 5555 {
                    found_modified_record = true;
                }
                
                // 检查新增的记录
                if id == 10 && value == 1000 {
                    found_new_record = true;
                }
                
                used_records_after_inc += 1;
            }
        }
        
        // 验证结果
        if found_modified_record {
            println!("✓ 找到修改后的记录");
        } else {
            println!("✗ 未找到修改后的记录");
            return Err(remdb::RemDbError::RecordNotFound);
        }
        
        if found_new_record {
            println!("✓ 找到新增的记录");
        } else {
            println!("✗ 未找到新增的记录");
            return Err(remdb::RemDbError::RecordNotFound);
        }
        
        if used_records_after_inc == 11 {
            println!("✓ 记录数正确，共 {} 条记录", used_records_after_inc);
        } else {
            println!("✗ 记录数不正确，期望 11 条，实际 {} 条", used_records_after_inc);
            return Err(remdb::RemDbError::RecordNotFound);
        }
        
        println!("\n所有测试通过！增量快照功能正常工作。");
        Ok(())
    }
}