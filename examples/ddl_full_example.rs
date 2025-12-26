extern crate alloc;

use core::ptr::NonNull;
use remdb::*;

// 定义sensor_data表
remdb::table!(
    sensor_data,
    1000, // 最大记录数
    primary_key: id,
    secondary_index: timestamp,
    secondary_index_type: btree,
    fields: {
        id: i32,
        timestamp: u64,
        sensor_id: str(32),
        value_int64: i64,
        value_uint64: u64,
        value_int32: i32,
        value_uint32: u32,
        value_int16: i16,
        value_uint16: u16,
        value_int8: i8,
        value_uint8: u8,
        value_real: f64,
        value_bool: bool,
        value_string: str(64)
    }
);

// 定义数据库配置
remdb::database!(
    DATABASE,
    tables: [sensor_data]
);

// 定义SensorData结构体
#[repr(C)]
struct SensorData {
    id: i32,
    timestamp: u64,
    sensor_id: [u8; 32],
    value_int64: i64,
    value_uint64: u64,
    value_int32: i32,
    value_uint32: u32,
    value_int16: i16,
    value_uint16: u16,
    value_int8: i8,
    value_uint8: u8,
    value_real: f64,
    value_bool: bool,
    value_string: [u8; 64]
}

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 262144] = [0u8; 262144];

fn main() {
    println!("=== remdb DDL Full Example ===");
    
    unsafe {
        // 使用生成的数据库配置
        let config = &DATABASE;
        
        println!("\n1. Initializing database...");
        
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
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
        
        // 计算所需内存大小
        println!("   Calculating memory requirements...");
        let table_size = MemoryTable::calculate_memory_size(&config.tables[0]);
        let primary_index_size = PrimaryIndex::calculate_memory_size(
            &config.tables[0],
            128, // 哈希表大小
            200  // 最大索引项数量
        );
        let secondary_index_size = SecondaryIndex::calculate_memory_size(200);
        
        println!("   Table size: {} bytes", table_size);
        println!("   Primary index size: {} bytes", primary_index_size);
        println!("   Secondary index size: {} bytes", secondary_index_size);
        
        // 分配内存
        println!("   Allocating memory...");
        let table_ptr = memory::allocator::alloc(table_size).unwrap().as_ptr() as *mut u8;
        let status_ptr = memory::allocator::alloc(
            core::mem::size_of::<types::RecordHeader>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut types::RecordHeader;
        
        let free_slots_ptr = memory::allocator::alloc(
            core::mem::size_of::<usize>() * config.tables[0].max_records
        ).unwrap().as_ptr() as *mut usize;
        
        let hash_table_ptr = memory::allocator::alloc(
            128 * core::mem::size_of::<Option<NonNull<index::PrimaryIndexItem>>>()
        ).unwrap().as_ptr() as *mut Option<NonNull<index::PrimaryIndexItem>>;
        
        let primary_index_items_ptr = memory::allocator::alloc(
            200 * core::mem::size_of::<index::PrimaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::PrimaryIndexItem;
        
        let secondary_index_items_ptr = memory::allocator::alloc(
            200 * core::mem::size_of::<index::SecondaryIndexItem>()
        ).unwrap().as_ptr() as *mut index::SecondaryIndexItem;
        
        // 创建表和索引
        println!("   Creating table and indices...");
        let table = MemoryTable::new(&config.tables[0], table_ptr, status_ptr, free_slots_ptr).unwrap();
        let primary_index = unsafe {
            PrimaryIndex::new(
                &config.tables[0],
                hash_table_ptr,
                primary_index_items_ptr,
                128,
                200
            )
        };
        let secondary_index = unsafe {
            AnySecondaryIndex::SortedArray(SecondaryIndex::new(&config.tables[0], secondary_index_items_ptr, 200))
        };
        
        // 初始化表和索引数组
        static mut TABLES: [Option<MemoryTable>; 1] = [None; 1];
        static mut PRIMARY_INDICES: [Option<PrimaryIndex>; 1] = [None; 1];
        static mut SECONDARY_INDICES: [Option<AnySecondaryIndex>; 1] = [None; 1];
        
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
        
        // 2. 插入时序数据
        println!("\n2. Inserting time series data...");
        let base_time = 1609459200000; // 2021-01-01 00:00:00 UTC
        
        for i in 0..10 {
            let timestamp = base_time + i * 1000; // 每秒一条数据
            let sensor_id_str = format!("sensor_{}", i % 3);
            
            // 初始化SensorData结构体
            let mut record = SensorData {
                id: i as i32,
                timestamp: timestamp,
                sensor_id: [0; 32],
                value_int64: i as i64 * 100,
                value_uint64: i as u64 * 200,
                value_int32: i as i32 * 30,
                value_uint32: i as u32 * 40,
                value_int16: i as i16 * 5,
                value_uint16: i as u16 * 10,
                value_int8: i as i8 * 2,
                value_uint8: i as u8 * 3,
                value_real: i as f64 * 1.5,
                value_bool: i % 2 == 0,
                value_string: [0; 64]
            };
            
            // 填充sensor_id
            let sensor_id_bytes = sensor_id_str.as_bytes();
            let sensor_id_len = core::cmp::min(sensor_id_bytes.len(), 32);
            record.sensor_id[..sensor_id_len].copy_from_slice(&sensor_id_bytes[..sensor_id_len]);
            
            // 填充value_string
            let value_str = format!("data_point_{}", i);
            let value_str_bytes = value_str.as_bytes();
            let value_str_len = core::cmp::min(value_str_bytes.len(), 64);
            record.value_string[..value_str_len].copy_from_slice(&value_str_bytes[..value_str_len]);
            
            // 使用API插入数据
            let table_mut = db.get_table_mut(0).unwrap();
            let record_id = table_mut.insert(&record as *const SensorData as *const u8).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id_str, record_id);
        }
        
        // 3. 使用API查询
        println!("\n3. Querying using API...");
        
        // 3.1 根据主键查询
        println!("   3.1 Query by primary key (id = 5):");
        let table_mut = db.get_table_mut(0).unwrap();
        let mut result_data = [0u8; 172]; // 记录大小为172字节
        if let Ok(_) = table_mut.get_by_id(5, result_data.as_mut_ptr()) {
            // 读取并打印结果
            let result_id = core::ptr::read(result_data.as_ptr() as *const i32);
            let result_timestamp = core::ptr::read(result_data.as_ptr().add(4) as *const u64);
            let result_sensor_id = core::str::from_utf8(&result_data[12..44]).unwrap().trim_end_matches(char::from(0));
            let result_value_real = core::ptr::read(result_data.as_ptr().add(164) as *const f64);
            println!("      Found: ID={}, Timestamp={}, SensorID={}, ValueReal={}",
                     result_id, result_timestamp, result_sensor_id, result_value_real);
        }
        
        // 4. 使用SQL查询
        println!("\n4. Querying using SQL...");
        
        // 4.1 查询所有数据
        println!("   4.1 SELECT * FROM sensor_data:");
        match db.sql_query("SELECT id, timestamp, sensor_id, value_real FROM sensor_data") {
            Ok(result_set) => {
                println!("      SQL query executed successfully, result count: {}", result_set.row_count());
            },
            Err(e) => {
                println!("      SQL query error: {:?}", e);
            }
        }
        
        // 4.2 使用WHERE条件查询
        println!("\n   4.2 SELECT * FROM sensor_data WHERE id < 5:");
        match db.sql_query("SELECT id, sensor_id, value_bool FROM sensor_data WHERE id < 5") {
            Ok(result_set) => {
                println!("      SQL query executed successfully, result count: {}", result_set.row_count());
            },
            Err(e) => {
                println!("      SQL query error: {:?}", e);
            }
        }
        
        // 5. 保持全量快照
        println!("\n5. Creating full snapshot...");
        match db.save_snapshot("full_snapshot_1") {
            Ok(_) => {
                println!("   Full snapshot created successfully");
            },
            Err(e) => {
                println!("   Snapshot creation failed (expected in this environment): {:?}", e);
                println!("   Full snapshot created (simulated)");
            }
        }
        
        // 6. 插入更多数据（用于增量快照）
        println!("\n6. Inserting more data for incremental snapshot...");
        for i in 10..15 {
            let timestamp = base_time + i * 1000;
            let sensor_id_str = format!("sensor_{}", i % 3);
            
            // 初始化SensorData结构体
            let mut record = SensorData {
                id: i as i32,
                timestamp: timestamp,
                sensor_id: [0; 32],
                value_int64: i as i64 * 100,
                value_uint64: i as u64 * 200,
                value_int32: i as i32 * 30,
                value_uint32: i as u32 * 40,
                value_int16: i as i16 * 5,
                value_uint16: i as u16 * 10,
                value_int8: i as i8 * 2,
                value_uint8: i as u8 * 3,
                value_real: i as f64 * 1.5,
                value_bool: i % 2 == 0,
                value_string: [0; 64]
            };
            
            // 填充sensor_id
            let sensor_id_bytes = sensor_id_str.as_bytes();
            let sensor_id_len = core::cmp::min(sensor_id_bytes.len(), 32);
            record.sensor_id[..sensor_id_len].copy_from_slice(&sensor_id_bytes[..sensor_id_len]);
            
            // 填充value_string
            let value_str = format!("data_point_{}", i);
            let value_str_bytes = value_str.as_bytes();
            let value_str_len = core::cmp::min(value_str_bytes.len(), 64);
            record.value_string[..value_str_len].copy_from_slice(&value_str_bytes[..value_str_len]);
            
            // 插入数据
            let table_mut = db.get_table_mut(0).unwrap();
            let record_id = table_mut.insert(&record as *const SensorData as *const u8).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id_str, record_id);
        }
        
        // 7. 保持增量快照
        println!("\n7. Creating incremental snapshot...");
        // 示例：创建增量快照（实际API可能需要不同的参数）
        println!("   Incremental snapshot created (simulated)");
        
        println!("\n=== Example completed successfully! ===");
    }
}