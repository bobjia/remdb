extern crate alloc;

use core::ptr::NonNull;
use remdb::*;
use remdb_macros::MemdbTable;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 262144] = [0u8; 262144];

// 使用DDL创建表和索引，覆盖所有支持的数据类型
#[derive(MemdbTable)]
#[memdb_schema(ddl = "CREATE TABLE sensor_data (
    id INTEGER PRIMARY KEY,
    timestamp TIMESTAMP NOT NULL,
    sensor_id TEXT NOT NULL,
    value_int64 INT64,
    value_uint64 UINT64,
    value_int32 INT32,
    value_uint32 UINT32,
    value_int16 INT16,
    value_uint16 UINT16,
    value_int8 INT8,
    value_uint8 UINT8,
    value_real REAL,
    value_bool BOOLEAN,
    value_string TEXT
);
CREATE INDEX idx_sensor_timestamp ON sensor_data USING btree (timestamp);
CREATE INDEX idx_sensor_id ON sensor_data USING hash (sensor_id);
CREATE INDEX idx_sensor_value ON sensor_data USING ttree (value_real);
CREATE INDEX idx_sensor_bool ON sensor_data USING sortedarray (value_bool);
")]
struct Database;

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
            let sensor_id = format!("sensor_{}", i % 3);
            
            let sensor_data = SensorData {
                id: i as i64,
                timestamp: timestamp,
                sensor_id: sensor_id.clone(),
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
                value_string: Some(format!("data_point_{}", i)),
            };
            
            // 使用API插入数据
            let table_mut = db.get_table_mut(0).unwrap();
            
            // 手动序列化数据（简化示例）
            let mut record_data = [0u8; 88]; // 计算记录大小
            
            // 填充字段值
            core::ptr::copy_nonoverlapping(&sensor_data.id as *const i64 as *const u8, record_data.as_mut_ptr(), 8);
            core::ptr::copy_nonoverlapping(&sensor_data.timestamp as *const u64 as *const u8, record_data.as_mut_ptr().add(8), 8);
            
            // 填充sensor_id字符串
            let sensor_id_bytes = sensor_data.sensor_id.as_bytes();
            core::ptr::copy_nonoverlapping(sensor_id_bytes.as_ptr(), record_data.as_mut_ptr().add(16), sensor_id_bytes.len());
            
            // 填充其他字段...（省略部分代码以简化示例）
            
            let record_id = table_mut.insert(record_data.as_ptr()).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id, record_id);
        }
        
        // 3. 使用API查询
        println!("\n3. Querying using API...");
        
        // 3.1 根据主键查询
        println!("   3.1 Query by primary key (id = 5):");
        let table_mut = db.get_table_mut(0).unwrap();
        let mut result_data = [0u8; 88];
        if let Ok(_) = table_mut.get_by_id(5, result_data.as_mut_ptr()) {
            // 读取并打印结果
            let result_id = core::ptr::read(result_data.as_ptr() as *const i64);
            let result_timestamp = core::ptr::read(result_data.as_ptr().add(8) as *const u64);
            let result_sensor_id = core::str::from_utf8(&result_data[16..80]).unwrap().trim_end_matches(char::from(0));
            println!("      Found: ID={}, Timestamp={}, SensorID={}",
                     result_id, result_timestamp, result_sensor_id);
        }
        
        // 4. 使用SQL查询
        println!("\n4. Querying using SQL...");
        
        // 4.1 查询所有数据
        println!("   4.1 SELECT * FROM SENSOR_DATA:");
        match db.sql_query("SELECT id, timestamp, sensor_id, value_real FROM SENSOR_DATA") {
            Ok(result_set) => {
                println!("      SQL query executed successfully, result count: {}", result_set.row_count());
            },
            Err(e) => {
                println!("      SQL query error: {:?}", e);
            }
        }
        
        // 4.2 使用WHERE条件查询
        println!("\n   4.2 SELECT * FROM SENSOR_DATA WHERE sensor_id = 'sensor_0' AND value_bool = true:");
        match db.sql_query("SELECT id, sensor_id, value_bool FROM SENSOR_DATA WHERE sensor_id = 'sensor_0' AND value_bool = true") {
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
            let sensor_id = format!("sensor_{}", i % 3);
            
            let sensor_data = SensorData {
                id: i as i64,
                timestamp: timestamp,
                sensor_id: sensor_id.clone(),
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
                value_string: Some(format!("data_point_{}", i)),
            };
            
            // 手动序列化数据
            let mut record_data = [0u8; 88];
            core::ptr::copy_nonoverlapping(&sensor_data.id as *const i64 as *const u8, record_data.as_mut_ptr(), 8);
            core::ptr::copy_nonoverlapping(&sensor_data.timestamp as *const u64 as *const u8, record_data.as_mut_ptr().add(8), 8);
            
            let sensor_id_bytes = sensor_data.sensor_id.as_bytes();
            core::ptr::copy_nonoverlapping(sensor_id_bytes.as_ptr(), record_data.as_mut_ptr().add(16), sensor_id_bytes.len());
            
            let table_mut = db.get_table_mut(0).unwrap();
            let record_id = table_mut.insert(record_data.as_ptr()).unwrap();
            println!("   Inserted data point {} for sensor {}, record_id: {}", i, sensor_id, record_id);
        }
        
        // 7. 保持增量快照
        println!("\n7. Creating incremental snapshot...");
        // 示例：创建增量快照（实际API可能需要不同的参数）
        println!("   Incremental snapshot created (simulated)");
        
        println!("\n=== Example completed successfully! ===");
    }
}
