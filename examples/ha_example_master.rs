// remdbHA 主节点示例
#![cfg(feature = "ha")]

#[macro_use]
extern crate remdb;

use core::ptr::NonNull;
use remdb::config::{DbConfig, DefaultMemoryAllocator, LogMode, WALConfig};
use remdb::ha::{HAConfig, HARole, ReplicationMode};
use remdb::time_series::TimeSeriesConfig;
use remdb::*;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 8388608] = [0u8; 8388608]; // 8MB内存缓冲区

// 定义表结构
remdb::table!(
    users,
    100, // 最大记录数
    primary_key: id,
    fields: {
        id: u32,
        name: str(32), // 32字节定长字符串
        age: u8,
        active: bool
    }
);

// 定义数据库配置 - 主节点
static MASTER_DB_CONFIG: std::sync::LazyLock<DbConfig> = std::sync::LazyLock::new(|| DbConfig {
    tables: vec![users.clone()],
    total_memory: 8 * 1024 * 1024,
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 1000,
    memory_allocator: &DefaultMemoryAllocator,
    wal_config: WALConfig {
        log_path: "./wal",
        log_mode: LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 1 * 1024 * 1024,
        log_prealloc_size: 0,
        log_segment_size: 1 * 1024 * 1024,
        retained_checkpoints: 1,
        max_consecutive_invalid: 100,
        skip_threshold: 1000,
        skip_block_size: 1024 * 1024,
        max_skip_attempts: 3,
        compression_type: config::WALCompressionType::None,
        compression_level: 3,
    },
    time_series_defaults: TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: Some(HAConfig {
        node_id: 1,
        ha_role: HARole::Master,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: None,
        master_port: None,
        replication_port: 5556,
    }),
});

// 主节点示例
fn master_example() {
    println!("=== 主节点示例 ===");

    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(DB_MEMORY.as_mut_ptr(), DB_MEMORY.len());

        // 平台初始化由RemDb的init方法自动处理

        // 初始化全局数据库
        let db = init_global_db(&MASTER_DB_CONFIG).expect("Failed to initialize database");

        // HA管理器由RemDb自动初始化和管理
        println!("[DEBUG] {}:{}: Before transaction begin", file!(), line!());

        // 创建测试记录
        let mut record_data = [0u8; 40]; // 计算记录大小：u32(4) + str(32) + u8(1) + bool(1) = 38字节（对齐到8字节为40字节）

        // 设置字段值
        let id: u32 = 1;
        let name = "test_user";
        let age: u8 = 30;
        let active = true;

        // 手动填充记录数据
        core::ptr::copy_nonoverlapping(&id as *const u32 as *const u8, record_data.as_mut_ptr(), 4);

        core::ptr::copy_nonoverlapping(name.as_ptr(), record_data.as_mut_ptr().add(4), name.len());

        core::ptr::write(record_data.as_mut_ptr().add(36) as *mut u8, age);
        core::ptr::write(record_data.as_mut_ptr().add(37) as *mut bool, active);

        // 开始事务
        let mut tx_buffer = transaction::Transaction::default();

        let mut log_buffer = vec![transaction::VariableSizeLogItem::default(); 10];

        let _tx = transaction::begin(
            transaction::TransactionType::ReadWrite,
            transaction::IsolationLevel::Serializable,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10,
        )
        .expect("Failed to begin transaction");

        // 插入记录
        let table_mut = db.get_table_mut(0).expect("Failed to get table");
        let record_id = table_mut
            .insert(record_data.as_ptr())
            .expect("Failed to insert record");

        // 提交事务
        transaction::commit().expect("Failed to commit transaction");

        println!("主节点：成功插入一条记录，ID: {}", record_id);
        println!("主节点：等待WAL日志被自动复制到从节点");

        // 运行一段时间，等待从节点连接并同步数据
        // 同时定期检查HA状态
        for i in 0..20 {
            // 检查HA状态
            if let Some(ha_manager) = ha::get_ha_manager() {
                if let Err(e) = ha_manager.check_status() {
                    println!("[HA] Master check status error: {:?}", e);
                }
            }

            // 每1秒检查一次
            std::thread::sleep(std::time::Duration::from_secs(1));
            println!("[HA] Master running, iteration: {}", i + 1);
        }

        // 关闭HA管理器
        ha::shutdown().expect("Failed to shutdown HA manager");
    }

    println!("主节点示例完成");
}

fn main() {
    master_example();
}
