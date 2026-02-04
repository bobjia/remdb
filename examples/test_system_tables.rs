extern crate alloc;

use remdb::*;

// 定义内存缓冲区（增大到4MB以容纳系统表）
static mut DB_MEMORY: [u8; 4194304] = [0u8; 4194304];

fn main() {
    unsafe {
        // 初始化内存分配器
        let _ = memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 初始化平台抽象层
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
                Err(())
            }
            fn file_close(&self, _handle: platform::FileHandle) -> platform::FileResult<()> {
                Err(())
            }
            fn file_write(
                &self,
                _handle: platform::FileHandle,
                _buffer: *const u8,
                _size: usize,
            ) -> platform::FileResult<usize> {
                Err(())
            }
            fn file_read(
                &self,
                _handle: platform::FileHandle,
                _buffer: *mut u8,
                _size: usize,
            ) -> platform::FileResult<usize> {
                Err(())
            }
            fn file_seek(
                &self,
                _handle: platform::FileHandle,
                _offset: i64,
                _whence: platform::SeekWhence,
            ) -> platform::FileResult<u64> {
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

        // 创建简单的数据库配置
        static ALLOCATOR: config::DefaultMemoryAllocator = config::DefaultMemoryAllocator;
        static CONFIG: config::DbConfig = config::DbConfig {
            tables: vec![],
            total_memory: 4194304, // 4MB
            low_power_mode_supported: false,
            low_power_max_records: None,
            default_max_records: 100000,
            memory_allocator: &ALLOCATOR,
            wal_config: config::WALConfig {
                log_path: "./wal",
                log_mode: config::LogMode::Async,
                checkpoint_interval_ms: 60000,
                log_file_size_limit: 16 * 1024 * 1024,
                log_prealloc_size: 0,
                log_segment_size: 16 * 1024 * 1024,
                retained_checkpoints: 2,
            },
            time_series_defaults: config::TimeSeriesConfig::DEFAULT,
            #[cfg(feature = "pubsub")]
            pubsub_config: None,
            #[cfg(feature = "ha")]
            ha_config: None,
        };

        // 初始化全局数据库
        println!("Initializing database...");
        match init_global_db(&CONFIG) {
            Ok(db) => {
                println!("Database initialized successfully!");
                println!("Number of tables: {}", db.table_count());
                
                // 检查系统表是否创建成功
                for (i, table_opt) in db.get_all_tables().iter().enumerate() {
                    if let Some(table) = table_opt {
                        println!("Table {}: {} (max_records: {})\n", i, table.def.name, table.def.max_records);
                    }
                }
                
                println!("Test completed successfully!");
            }
            Err(e) => {
                println!("Failed to initialize database: {:?}", e);
            }
        }
    }
}
