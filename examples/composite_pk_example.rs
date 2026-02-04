extern crate alloc;
use remdb::{DataType, RemDb, Result};

/// 定义静态内存缓冲区
static mut DB_MEMORY: [u8; 2097152] = [0u8; 2097152]; // 2MB内存缓冲区

/// 定义静态数据库配置
static DB_CONFIG: remdb::config::DbConfig = remdb::config::DbConfig {
    tables: vec![],
    total_memory: 2097152, // 2MB
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 1000,
    memory_allocator: unsafe {
        // 使用静态DEFAULT_ALLOCATOR
        static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
        &mut DEFAULT_ALLOCATOR
    },
    wal_config: remdb::config::WALConfig {
        log_path: "wal",
        log_mode: remdb::config::LogMode::Sync,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 16 * 1024 * 1024,
        log_prealloc_size: 1 * 1024 * 1024,
        log_segment_size: 16 * 1024 * 1024,
        retained_checkpoints: 3,
    },
    time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: None,
};

/// 复合主键示例
fn main() -> Result<()> {
    // 初始化内存分配器
    unsafe {
        remdb::memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len())?;

        // 初始化平台抽象层
        #[cfg(feature = "posix")]
        remdb::platform::init_platform(remdb::platform::posix::get_posix_platform());
        #[cfg(not(feature = "posix"))]
        {
            // 在非posix平台上，使用一个简单的平台实现
            struct DummyPlatform;
            impl remdb::platform::Platform for DummyPlatform {
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
                    _mode: remdb::platform::FileMode,
                ) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
                    Err(())
                }
                fn file_close(&self, _handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
                    Err(())
                }
                fn file_write(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _buffer: *const u8,
                    _size: usize,
                ) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn file_read(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _buffer: *mut u8,
                    _size: usize,
                ) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn file_seek(
                    &self,
                    _handle: remdb::platform::FileHandle,
                    _offset: i64,
                    _whence: remdb::platform::SeekWhence,
                ) -> remdb::platform::FileResult<u64> {
                    Err(())
                }
                fn file_remove(&self, _path: &str) -> remdb::platform::FileResult<()> {
                    Err(())
                }
                fn file_size(&self, _path: &str) -> remdb::platform::FileResult<usize> {
                    Err(())
                }
                fn crc32(&self, _data: *const u8, _size: usize) -> u32 {
                    0
                }
            }
            static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;
            remdb::platform::init_platform(&DUMMY_PLATFORM);
        }
    }
    
    // 创建数据库实例
    let mut db = RemDb::new(&DB_CONFIG);
    db.init()?;
    
    println!("=== 复合主键示例 ===");
    
    // 创建带有复合主键的表
    let fields = [
        ("device_id", DataType::UInt32, 0, None, None),
        ("metric_id", DataType::UInt32, 0, None, None),
        ("timestamp", DataType::UInt64, 0, None, None),
        ("value", DataType::Float64, 0, None, None),
    ];
    
    // 定义复合主键为(device_id, metric_id, timestamp)
    let primary_key = Some(vec![0, 1, 2]);
    
    db.create_table("metrics", &fields, primary_key)?;
    println!("成功创建带有复合主键的表: metrics (device_id, metric_id, timestamp)");
    
    // 获取表
    let table_id = 1; // 系统表占用0，新表ID为1
    let table = db.get_table(table_id)?;
    
    println!("表结构:");
    println!("  字段数: {}", table.def.fields.len());
    println!("  复合主键字段索引: {:?}", table.def.primary_key);
    
    for (i, field) in table.def.fields.iter().enumerate() {
        println!("  字段 {}: {} ({:?})", i, field.name, field.data_type);
    }
    
    println!("\n=== 示例完成 ===");
    Ok(())
}
