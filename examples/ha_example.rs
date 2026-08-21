// remdbHA 主从复制示例

#![cfg(feature = "ha")]
#![allow(unsafe_code)]

#[macro_use]
extern crate remdb;

use core::ptr::NonNull;
use remdb::*;
use remdb::ha::{HARole, ReplicationMode, HAConfig};

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

// 定义数据库配置 - 主节点
remdb::database!(
    MASTER_DB_CONFIG,
    tables: [users],
    low_power: false
);

// 定义数据库配置 - 从节点
remdb::database!(
    SLAVE_DB_CONFIG,
    tables: [users],
    low_power: false
);

// 定义平台实现
struct DummyPlatform;
impl remdb::platform::Platform for DummyPlatform {
    fn get_timestamp(&self) -> u64 {
        0
    }
    fn get_timestamp_us(&self) -> u64 {
        0
    }
    fn memcpy(&self, dest: &mut [u8], src: &[u8]) {
        let len = dest.len().min(src.len());
        dest[..len].copy_from_slice(&src[..len]);
    }
    fn memset(&self, dest: &mut [u8], value: u8) {
        dest.fill(value);
    }
    fn delay_ms(&self, _ms: u32) {
    }
    fn delay_us(&self, _us: u32) {
    }
    fn file_open(&self, _path: &str, _mode: remdb::platform::FileMode) -> remdb::platform::FileResult<remdb::platform::FileHandle> {
        Err(())
    }
    fn file_close(&self, _handle: remdb::platform::FileHandle) -> remdb::platform::FileResult<()> {
        Err(())
    }
    fn file_write(&self, _handle: remdb::platform::FileHandle, _buf: &[u8]) -> remdb::platform::FileResult<usize> {
        Err(())
    }
    fn file_read(&self, _handle: remdb::platform::FileHandle, _buf: &mut [u8]) -> remdb::platform::FileResult<usize> {
        Err(())
    }
    fn file_seek(&self, _handle: remdb::platform::FileHandle, _offset: i64, _whence: remdb::platform::SeekWhence) -> remdb::platform::FileResult<u64> {
        Err(())
    }
    fn file_remove(&self, _path: &str) -> remdb::platform::FileResult<()> {
        Err(())
    }
    fn file_size(&self, _path: &str) -> remdb::platform::FileResult<usize> {
        Err(())
    }
    fn crc32(&self, _data: &[u8]) -> u32 {
        0
    }
}
static DUMMY_PLATFORM: DummyPlatform = DummyPlatform;

// 主节点示例
fn master_example() {
    println!("=== 主节点示例 ===");
    
    unsafe {
        // 初始化内存分配器
        memory::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        platform::init_platform(&DUMMY_PLATFORM);
        // 初始化全局数据库
        let db = init_global_db(&MASTER_DB_CONFIG).expect("Failed to initialize database");
        
        // 开始事务
        let mut tx_buffer: transaction::Transaction = core::mem::MaybeUninit::uninit().assume_init();
        
        let mut log_buffer = [transaction::LogItem {
            op_type: transaction::LogOperation::Insert,
            table_id: 0,
            record_id: 0,
            data_size: 0,
            old_data: [0u8; 512],
            new_data: [0u8; 512],
            tx_id: 0,
            timestamp: 0,
            checksum: 0,
        }; 10];
        
        let tx = transaction::begin(
            transaction::TransactionType::ReadWrite,
            transaction::IsolationLevel::Serializable,
            &mut tx_buffer,
            log_buffer.as_mut_ptr(),
            10
        ).expect("Failed to begin transaction");
        
        // 创建测试记录
        let mut record_data = [0u8; 40]; // 计算记录大小：u32(4) + str(32) + u8(1) + bool(1) = 38字节（对齐到8字节为40字节）
        
        // 设置字段值
        let id: u32 = 1;
        let name = "test_user";
        let age: u8 = 30;
        let active = true;
        
        // 手动填充记录数据
        core::ptr::copy_nonoverlapping(
            &id as *const u32 as *const u8,
            record_data.as_mut_ptr(),
            4
        );
        
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            record_data.as_mut_ptr().add(4),
            name.len()
        );
        
        core::ptr::write(record_data.as_mut_ptr().add(36) as *mut u8, age);
        core::ptr::write(record_data.as_mut_ptr().add(37) as *mut bool, active);
        
        // 插入记录
        let table_mut = db.get_table_mut(0).expect("Failed to get table");
        let record_id = table_mut.insert(&record_data).expect("Failed to insert record");
        
        // 提交事务
        transaction::commit().expect("Failed to commit transaction");
        
        println!("主节点：成功插入一条记录，ID: {}", record_id);
        println!("主节点：WAL日志已自动复制到从节点");
        
        // 运行一段时间，等待复制完成
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    
    println!("主节点示例完成");
}

// 从节点示例
fn slave_example() {
    println!("\n=== 从节点示例 ===");
    
    unsafe {
        // 初始化内存分配器
        memory::init_global_allocator(
            DB_MEMORY.as_mut_ptr(),
            DB_MEMORY.len()
        );
        
        // 初始化平台抽象层
        platform::init_platform(&DUMMY_PLATFORM);
        // 初始化全局数据库
        let db = init_global_db(&SLAVE_DB_CONFIG).expect("Failed to initialize database");
        
        // 运行一段时间，等待从主节点同步数据
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        // 读取数据（应该是从主节点复制过来的）
        let table = db.get_table(0).expect("Failed to get table");
        let record_id = 1;
        
        // 尝试获取记录，get_by_id如果失败会返回错误
        let mut result_data = [0u8; 40];
        match table.get_by_id(record_id, &mut result_data) {
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
    }
    
    println!("从节点示例完成");
}

fn main() {
    // 解析命令行参数，确定运行模式
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() != 2 {
        println!("用法：ha_example <master|slave>");
        return;
    }
    
    match args[1].as_str() {
        "master" => master_example(),
        "slave" => slave_example(),
        _ => println!("无效参数，使用 master 或 slave")
    }
}
