// remdbHA 从节点示例

#[macro_use]
extern crate remdb;

use core::ptr::NonNull;
use remdb::*;
use remdb::ha::{HARole, ReplicationMode, HAConfig};
use remdb::config::{DbConfig, WALConfig, DefaultMemoryAllocator, LogMode};
use remdb::time_series::TimeSeriesConfig;

// 定义内存缓冲区
static mut DB_MEMORY: [u8; 65536] = [0u8; 65536];

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

// 定义数据库配置 - 从节点
static SLAVE_DB_CONFIG: DbConfig = DbConfig {
    tables: &[users],
    total_memory: 8 * 1024 * 1024,
    low_power_mode_supported: false,
    low_power_max_records: None,
    default_max_records: 1000,
    memory_allocator: &DefaultMemoryAllocator,
    wal_config: WALConfig {
        log_path: "./wal_slave",
        log_mode: LogMode::Async,
        checkpoint_interval_ms: 60000,
        log_file_size_limit: 1 * 1024 * 1024,
        log_prealloc_size: 0,
        log_segment_size: 1 * 1024 * 1024,
        retained_checkpoints: 1,
    },
    time_series_defaults: TimeSeriesConfig::DEFAULT,
    #[cfg(feature = "pubsub")]
    pubsub_config: None,
    #[cfg(feature = "ha")]
    ha_config: Some(HAConfig {
        ha_role: HARole::Slave,
        replication_mode: ReplicationMode::Async,
        heartbeat_interval_ms: 1000,
        failure_detection_ms: 3000,
        sync_timeout_ms: 2000,
        master_address: Some("127.0.0.1"),
        master_port: Some(5557),
        replication_port: 5556,
        heartbeat_port: 5557,
    }),
};

// 从节点示例
fn slave_example() {
    println!("\n=== 从节点示例 ===");
    
    unsafe {
        // 初始化内存分配器
        memory::allocator::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 平台初始化由RemDb的init方法自动处理
        
        // 初始化全局数据库
        let db = init_global_db(&SLAVE_DB_CONFIG).expect("Failed to initialize database");
        
        // HA管理器由RemDb自动初始化和管理
        
        // 运行一段时间，等待从主节点同步数据
        // 同时定期检查HA状态
        for i in 0..15 {
            // 检查HA状态
            if let Some(ha_manager) = ha::get_ha_manager() {
                if let Err(e) = ha_manager.check_status() {
                    println!("[HA] Slave check status error: {:?}", e);
                }
            }
            
            // 每1秒检查一次
            std::thread::sleep(std::time::Duration::from_secs(1));
            println!("[HA] Slave running, iteration: {}", i+1);
        }
        
        // 读取数据（应该是从主节点复制过来的）
        let table = db.get_table(0).expect("Failed to get table");
        let record_id = 0;
        
        // 尝试获取记录，get_by_id如果失败会返回错误
        let mut result_data = [0u8; 40];
        match table.get_by_id(record_id, result_data.as_mut_ptr()) {
            Ok(_) => {
                // 读取字段值
                let result_id = core::ptr::read(result_data.as_ptr() as *const u32);
                let result_name = core::str::from_utf8(&result_data[4..36]).unwrap().trim_end_matches(char::from(0));
                let result_age = core::ptr::read(result_data.as_ptr().add(36) as *const u8);
                let result_active = core::ptr::read(result_data.as_ptr().add(37) as *const bool);
                
                println!("从节点：成功读取到主节点复制的数据");
                println!("从节点：ID: {}, Name: {}, Age: {}, Active: {}", 
                         result_id, result_name, result_age, result_active);
            },
            Err(_) => {
                println!("从节点：未能读取到主节点数据");
            }
        }
        
        // 关闭HA管理器
        ha::shutdown().expect("Failed to shutdown HA manager");
    }
    
    println!("从节点示例完成");
}

fn main() {
    slave_example();
}