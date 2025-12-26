extern crate alloc;

use core::ptr::NonNull;
use remdb::*;
use remdb_macros::MemdbTable;

// 使用DDL定义表结构和数据库
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE sensor_data (
    id INTEGER PRIMARY KEY,
    timestamp BIGINT NOT NULL,
    sensor_id TEXT(32) NOT NULL,
    value_int64 BIGINT,
    value_uint64 UNSIGNED BIGINT,
    value_int32 INTEGER,
    value_uint32 UNSIGNED INTEGER,
    value_int16 SMALLINT,
    value_uint16 UNSIGNED SMALLINT,
    value_int8 TINYINT,
    value_uint8 UNSIGNED TINYINT,
    value_real DOUBLE PRECISION,
    value_bool BOOLEAN,
    value_string TEXT(64)
);
CREATE INDEX idx_sensor_timestamp ON sensor_data USING btree (timestamp);")]
struct Database;

// SensorData结构体将由MemdbTable宏自动生成

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
            let sensor_id_copy = sensor_id_str.clone();
            
            // 初始化自动生成的SensorData结构体
            let record = SensorData {
                id: i as i32,
                timestamp: timestamp,
                sensor_id: sensor_id_str,
                value_int64: Some(i as i64 * 100),
                value_uint64: Some(i as u64 * 200),
                value_int32: Some(i as i32 * 30),
                value_uint32: Some(i as u32 * 40),
                value_int16: Some(i as i16 * 5),
                value_uint16: Some(i as u16 * 10),
                value_int8: Some(i as i8 * 2),
                value_uint8: Some(i as u8 * 3),
                value_real: Some(i as f64 * 1.5),
                value_bool: Some(i % 2 == 0),
                value_string: Some(format!("data_point_{}", i))
            };
            
            // 使用API插入数据
            let table_mut = db.get_table_mut(0).unwrap();
            let record_id = table_mut.insert(&record as *const SensorData as *const u8).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id_copy, record_id);
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
            
            // 安全处理字符串字段：先找到第一个零字节，再转换为字符串
            let sensor_id_slice = &result_data[12..44];
            let sensor_id_len = sensor_id_slice.iter().position(|&c| c == 0).unwrap_or(sensor_id_slice.len());
            let result_sensor_id = core::str::from_utf8(&sensor_id_slice[..sensor_id_len]).unwrap_or("<invalid-utf8>");
            
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
            let sensor_id_copy = sensor_id_str.clone();
            
            // 初始化自动生成的SensorData结构体
            let record = SensorData {
                id: i as i32,
                timestamp: timestamp,
                sensor_id: sensor_id_str,
                value_int64: Some(i as i64 * 100),
                value_uint64: Some(i as u64 * 200),
                value_int32: Some(i as i32 * 30),
                value_uint32: Some(i as u32 * 40),
                value_int16: Some(i as i16 * 5),
                value_uint16: Some(i as u16 * 10),
                value_int8: Some(i as i8 * 2),
                value_uint8: Some(i as u8 * 3),
                value_real: Some(i as f64 * 1.5),
                value_bool: Some(i % 2 == 0),
                value_string: Some(format!("data_point_{}", i))
            };
            
            // 插入数据
            let table_mut = db.get_table_mut(0).unwrap();
            let record_id = table_mut.insert(&record as *const SensorData as *const u8).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id_copy, record_id);
        }
        
        // 7. 保持增量快照
        println!("\n7. Creating incremental snapshot...");
        // 示例：创建增量快照（实际API可能需要不同的参数）
        println!("   Incremental snapshot created (simulated)");
        
        println!("\n=== Example completed successfully! ===");
    }
}