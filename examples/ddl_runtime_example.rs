// 运行时DDL配置示例

// 运行时DDL配置示例
use remdb::config::{DbConfig, LogMode, WALConfig};
#[cfg(feature = "ha")]
use remdb::ha::{HAConfig, HARole, ReplicationMode};
use remdb::memory::allocator::init_global_allocator;
use remdb::{
    types::{DataType, IndexType},
    RemDb,
};

fn main() {
    println!("=== RemDb Runtime DDL Configuration Example ===\n");

    // 初始化全局内存分配器
    static mut MEMORY_BUFFER: [u8; 2097152] = [0; 2097152]; // 2MB
    unsafe {
        #[allow(static_mut_refs)]
        init_global_allocator(MEMORY_BUFFER.as_mut_ptr(), MEMORY_BUFFER.len())
            .expect("Failed to initialize global allocator");

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

    // 创建数据库配置
    static CONFIG: DbConfig = DbConfig {
        tables: vec![],
        total_memory: 2097152, // 2MB
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10, // 减少默认最大记录数，避免内存不足
        memory_allocator: unsafe {
            // 使用静态DEFAULT_ALLOCATOR
            static mut DEFAULT_ALLOCATOR: remdb::config::DefaultMemoryAllocator = remdb::config::DefaultMemoryAllocator;
            &mut DEFAULT_ALLOCATOR
        },
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: LogMode::Sync,
            checkpoint_interval_ms: 60000,
            log_file_size_limit: 16 * 1024 * 1024,
            log_prealloc_size: 1 * 1024 * 1024,
            log_segment_size: 16 * 1024 * 1024,
            retained_checkpoints: 3,
            max_consecutive_invalid: 100,
            skip_threshold: 1000,
            skip_block_size: 1024 * 1024,
            max_skip_attempts: 3,
        },
        time_series_defaults: remdb::time_series::TimeSeriesConfig::DEFAULT,
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(HAConfig {
            ha_role: HARole::Auto,
            replication_mode: ReplicationMode::Async,
            node_id: 1,
            heartbeat_interval_ms: 1000,
            failure_detection_ms: 3000,
            sync_timeout_ms: 2000,
            master_address: None,
            master_port: None,
            replication_port: 5556,
        }),
    };

    // 创建数据库实例
    let mut db = RemDb::new(&CONFIG);

    // 初始化数据库和平台
    db.init().expect("Failed to initialize database");

    println!("1. Testing DDL API - DdlExecutor trait");
    println!("=========================================");

    // 使用DdlExecutor trait创建表
    let result = db.create_table(
        "users",
        &[
            ("id", DataType::UInt32, 0, None, None),
            ("name", DataType::VarChar, 32, None, None),
            ("age", DataType::UInt8, 0, None, None),
            ("active", DataType::Bool, 0, None, None),
        ],
        Some(vec![0]), // 主键为id字段
    );

    match result {
        Ok(_) => println!("   ✓ Table 'users' created successfully!"),
        Err(e) => println!("   ✗ Failed to create table: {:?} ", e),
    }

    // 使用DdlExecutor trait创建索引
    let result = db.create_index("users", "name", IndexType::BTree);

    match result {
        Ok(_) => println!("   ✓ Index on 'users.name' created successfully!"),
        Err(e) => println!("   ✗ Failed to create index: {:?} ", e),
    }

    println!("\n2. Testing SQL DDL Statements");
    println!("===========================");

    // 使用SQL语句创建表
    let result = db.sql_query(
        "CREATE TABLE products (id INTEGER PRIMARY KEY, name VARCHAR(32), price FLOAT, in_stock BOOL);",
    );

    match result {
        Ok(_) => println!("   ✓ Table 'products' created successfully via SQL!"),
        Err(e) => println!("   ✗ Failed to create table via SQL: {:?} ", e),
    }

    // 使用SQL语句创建索引
    let result = db.sql_query("CREATE INDEX idx_product_name ON products (name) USING BTree;");

    match result {
        Ok(_) => println!("   ✓ Index 'idx_product_name' created successfully via SQL!"),
        Err(e) => println!("   ✗ Failed to create index via SQL: {:?}", e),
    }

    println!("\n=== Example Completed ===");
}
