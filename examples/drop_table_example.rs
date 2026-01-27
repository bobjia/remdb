//! DROP TABLE 操作示例
//! 
//! 该示例展示了如何使用remdb的DROP TABLE操作，包括：
//! 1. 创建表
//! 2. 使用SQL DROP TABLE语句删除表
//! 3. 使用SQL DROP TABLE IF EXISTS语句删除不存在的表

// 引入alloc模块
extern crate alloc;
use alloc::vec::Vec;

use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 32 * 1024 * 1024] = [0u8; 32 * 1024 * 1024]; // 32MB

// 定义静态数据库配置
static DB_CONFIG: config::DbConfig = config::DbConfig {
    total_memory: 32 * 1024 * 1024, // 32MB
    default_max_records: 100,
    low_power_mode_supported: false,
    low_power_max_records: Some(50),
    wal_config: config::WALConfig {
        log_path: ".",
        log_mode: config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 0,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 1,
    },
    tables: Vec::new(),
    memory_allocator: &config::DefaultMemoryAllocator,
    time_series_defaults: time_series::TimeSeriesConfig {
        partition_duration_secs: 3600,
        retention_period_secs: 86400,
        max_partitions: 100,
        compression: time_series::CompressionType::None,
    },
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,
};

fn main() {
    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len()).unwrap();

        // 初始化平台抽象层
        // 使用一个简单的平台实现，所有文件操作都返回成功
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
                // 返回一个非空指针作为有效的FileHandle
                Ok(1 as *const u8)
            }
            fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_write(
                &self,
                _handle: platform::FileHandle,
                _buffer: *const u8,
                size: usize,
            ) -> platform::FileResult<usize> {
                Ok(size)
            }
            fn file_read(
                &self,
                _handle: platform::FileHandle,
                _buffer: *mut u8,
                _size: usize,
            ) -> platform::FileResult<usize> {
                // 对于读取操作，返回0表示文件为空，这样会创建新的日志头
                Ok(0)
            }
            fn file_seek(
                &self,
                _handle: platform::FileHandle,
                _offset: i64,
                _whence: platform::SeekWhence,
            ) -> platform::FileResult<u64> {
                Ok(0)
            }
            fn file_remove(&self, _path: &str) -> platform::FileResult<()> {
                Ok(())
            }
            fn file_size(&self, _path: &str) -> platform::FileResult<usize> {
                Ok(0)
            }
            fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                0
            }
        }
        static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
        platform::init_platform(&DUMMY_PLATFORM);

        // 创建数据库实例
        let mut db = RemDb::new(&DB_CONFIG);
        db.init().unwrap();

        println!("=== DROP TABLE 操作示例 ===");

        // 1. 创建表
        println!("\n1. 创建测试表:");
        db.create_table(
            "test_table",
            &[
                (
                    "id",
                    DataType::Int64,
                    0,
                    None,
                    None,
                ),
                (
                    "name",
                    DataType::String,
                    0,
                    None,
                    None,
                ),
            ],
            Some(vec![0]),
        )
        .unwrap();
        println!("   创建表 'test_table' 成功");

        // 验证表存在
        let table = db.get_table(1).unwrap();
        println!("   验证表存在: 表名 = '{}'", table.def.name);

        // 2. 使用SQL DROP TABLE语句删除表
        println!("\n2. 使用SQL DROP TABLE语句删除表:");
        let drop_sql = "DROP TABLE test_table;";
        let drop_query = sql::parse_sql_query(drop_sql).unwrap();
        sql::execute_query(&mut db, &drop_query).unwrap();
        println!("   使用SQL语句删除表 'test_table' 成功");

        // 验证表不存在
        let result = db.get_table(1);
        println!("   验证表不存在: 结果 = {:?}", result.is_err());

        // 3. 重新创建表用于后续测试
        println!("\n3. 重新创建测试表:");
        db.create_table(
            "test_table",
            &[
                (
                    "id",
                    DataType::Int64,
                    0,
                    None,
                    None,
                ),
                (
                    "name",
                    DataType::String,
                    0,
                    None,
                    None,
                ),
            ],
            Some(vec![0]),
        )
        .unwrap();
        println!("   重新创建表 'test_table' 成功");

        // 4. 使用RemDb::drop_table方法删除表
        println!("\n4. 使用RemDb::drop_table方法删除表:");
        db.drop_table("test_table", false, false).unwrap();
        println!("   使用drop_table方法删除表 'test_table' 成功");

        // 验证表不存在
        let result = db.get_table(1);
        println!("   验证表不存在: 结果 = {:?}", result.is_err());

        // 5. 使用SQL DROP TABLE IF EXISTS语句删除不存在的表
        println!("\n5. 使用SQL DROP TABLE IF EXISTS语句删除不存在的表:");
        let drop_if_exists_sql = "DROP TABLE IF EXISTS non_existent_table;";
        let drop_if_exists_query = sql::parse_sql_query(drop_if_exists_sql).unwrap();
        let result = sql::execute_query(&mut db, &drop_if_exists_query);
        println!("   使用IF EXISTS删除不存在的表: 结果 = {:?}", result.is_ok());

        // 6. 使用RemDb::drop_table方法删除不存在的表（使用if_exists=true）
        println!("\n6. 使用RemDb::drop_table方法删除不存在的表:");
        let result = db.drop_table("non_existent_table", true, false);
        println!("   使用if_exists=true删除不存在的表: 结果 = {:?}", result.is_ok());

        println!("\n=== DROP TABLE 操作示例完成 ===");
    }
}
