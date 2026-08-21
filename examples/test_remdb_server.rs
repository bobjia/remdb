#![cfg(all(feature = "pubsub", feature = "ha"))]
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex}; 
use std::thread; 
use std::time::Duration; 
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode}; 
use remdb::pubsub::topics::*; 
use remdb::{RemDb, config::{DbConfig, MemoryAllocator, HAConfig, WALConfig}};
use remdb::ha::{HARole, ReplicationMode}; 
use remdb::time_series::table::TimeSeriesConfig; 
use remdb::time_series::compression::CompressionType; 
use core::ptr::NonNull; 
use std::env; 

// 简单的内存分配器实现 
struct SimpleAllocator; 

impl MemoryAllocator for SimpleAllocator { 
    fn allocate(&self, _size: usize) -> Option<NonNull<u8>> { 
        static mut BUFFER: [u8; 4 * 1024 * 1024] = [0u8; 4 * 1024 * 1024]; 
        unsafe { 
            Some(NonNull::new(BUFFER.as_mut_ptr()).unwrap()) 
        } 
    } 
    
    fn deallocate(&self, _ptr: NonNull<u8>, _size: usize) { 
        // 简化实现，不实际释放内存 
    } 
} 

// 显式实现Sync trait 
unsafe impl Sync for SimpleAllocator {} 

// 静态内存分配器实例 
static ALLOCATOR: SimpleAllocator = SimpleAllocator; 

// 主题ID定义 
const WAL_TOPIC_ID: u16 = 1; 
const TABLES_TOPIC_ID: u16 = 2; 
const METRICS_TOPIC_ID: u16 = 3; 
const HEALTH_STATUS_TOPIC_ID: u16 = 4; 

fn main() { 
    // 解析命令行参数 
    let args: Vec<String> = env::args().collect(); 
    
    // 默认配置
    let mut role = HARole::Master;
    let mut master_ip: Option<&'static str> = None;
    let mut master_port = None; 
    let mut replication_mode = ReplicationMode::Async; // 默认异步复制
    
    // 解析命令行参数
    if args.len() > 1 {
        match args[1].as_str() {
            "master" => role = HARole::Master,
            "slave" => role = HARole::Slave,
            _ => {
                println!("Invalid role. Use 'master' or 'slave'.");
                println!("Usage: {} [master|slave] [replication_mode] [master_ip] [master_port]", args[0]);
                println!("Replication modes: sync, async (default: async)");
                return;
            }
        }
        
        // 解析复制模式
        if args.len() > 2 {
            match args[2].as_str() {
                "sync" => replication_mode = ReplicationMode::Sync,
                "async" => replication_mode = ReplicationMode::Async,
                _ => {
                    println!("Invalid replication mode. Use 'sync' or 'async'.");
                    println!("Usage: {} [master|slave] [replication_mode] [master_ip] [master_port]", args[0]);
                    return;
                }
            }
        }
        
        // 如果是从节点，解析主节点IP和端口
        if role == HARole::Slave {
            let required_args = if args.len() > 2 && (args[2] == "sync" || args[2] == "async") {
                5 // 包含复制模式的情况
            } else {
                4 // 不包含复制模式的情况
            };
            
            if args.len() < required_args {
                println!("Slave role requires master IP and port.");
                println!("Usage: {} slave [sync|async] <master_ip> <master_port>", args[0]);
                return;
            }
            
            let ip_arg_index = if args.len() > 2 && (args[2] == "sync" || args[2] == "async") {
                3
            } else {
                2
            };
            
            let port_arg_index = ip_arg_index + 1;
            
            // 使用Box::leak将String转换为&'static str
            master_ip = Some(Box::leak(args[ip_arg_index].clone().into_boxed_str()));
            master_port = Some(args[port_arg_index].parse::<u16>().expect("Invalid master port"));
        }
    } 
    
    println!("Starting RemDB Server..."); 
    println!("Role: {:?}", role); 
    println!("Replication Mode: {:?}", replication_mode); 
    if let (Some(ip), Some(port)) = (&master_ip, master_port) { 
        println!("Master: {}:{}", ip, port); 
    } 
    
    // 在堆上分配内存用于全局分配器 
    let mut mem_buffer = Box::new([0u8; 4 * 1024 * 1024]); // 4MB 
    
    // 初始化全局内存分配器 - 必须在创建数据库实例之前执行 
    let ptr = mem_buffer.as_mut_ptr(); 
    remdb::memory::allocator::init_global_allocator(ptr, mem_buffer.len()).expect("Failed to initialize global allocator"); 
    
    // 定义数据库配置
    let config = Box::leak(Box::new(DbConfig {
        tables: &[], // 空的数据库配置
        total_memory: 4 * 1024 * 1024, // 4MB，与全局缓冲区大小一致
        low_power_mode_supported: false,
        low_power_max_records: None,
        default_max_records: 10000,
        memory_allocator: &ALLOCATOR, // 使用我们的静态内存分配器
        wal_config: WALConfig {
            log_path: "./wal",
            log_mode: remdb::config::LogMode::Async,
            checkpoint_interval_ms: 60000, // 60秒
            log_file_size_limit: 16 * 1024 * 1024, // 16MB
            log_prealloc_size: 4 * 1024 * 1024, // 4MB
            log_segment_size: 16 * 1024 * 1024, // 16MB
            retained_checkpoints: 2,
        },
        #[cfg(feature = "pubsub")]
        pubsub_config: None,
        #[cfg(feature = "ha")]
        ha_config: Some(HAConfig {
            node_id: 1,
            ha_role: role,
            replication_mode: replication_mode,
            heartbeat_interval_ms: 5000, // 5秒
            failure_detection_ms: 15000, // 15秒
            sync_timeout_ms: 5000, // 5秒
            master_address: master_ip,
            master_port: master_port,
            replication_port: 5556,
        }),
        time_series_defaults: TimeSeriesConfig {
            partition_duration_secs: 3600, // 1小时
            retention_period_secs: 7 * 24 * 3600, // 7天
            compression: CompressionType::None,
            max_partitions: 100,
        },
    }));
    
    // 创建数据库实例
    let mut db = RemDb::new(config); 
    
    // 初始化数据库 
    db.init().expect("Failed to initialize database"); 
    
    // 将数据库实例包装在Arc<Mutex>中以便在多个线程中共享 
    let db_shared = Arc::new(Mutex::new(db)); 
    
    // 创建发布/订阅配置 
    let pubsub_config = PubSubConfig { 
        udp_mode: UdpMode::Broadcast, 
        multicast_addr: None, 
        port: 5555, 
        max_topics: 32, 
        max_subscribers_per_topic: 16, 
        buffer_size: 4096, 
        enable_nack: true, 
        retransmit_timeout: Duration::from_millis(100), 
        max_retransmits: 3, 
        heartbeat_interval: Duration::from_secs(5), 
        frame_pool_size: 128, 
    }; 
    
    // 创建发布/订阅实例 
    let mut pubsub = PubSub::new(pubsub_config).expect("Failed to create PubSub instance"); 
    pubsub.init().expect("Failed to initialize PubSub"); 
    
    // 注册所有预定义主题 
    pubsub.register_topic(WAL_TOPIC, WAL_TOPIC_ID).expect("Failed to register WAL topic"); 
    pubsub.register_topic(TABLES_TOPIC, TABLES_TOPIC_ID).expect("Failed to register tables topic"); 
    // pubsub.register_topic(METRICS_TOPIC, METRICS_TOPIC_ID).expect("Failed to register metrics topic"); 
    // pubsub.register_topic(HEALTH_STATUS_TOPIC, HEALTH_STATUS_TOPIC_ID).expect("Failed to register health status topic"); 
    
    println!("RemDB Server started successfully!"); 
    println!("Listening on UDP port 5555"); 
    println!("Topics available:"); 
    println!("- WAL (ID: {}) - All WAL operations", WAL_TOPIC_ID); 
    println!("- TABLES (ID: {}) - Table creation/deletion events", TABLES_TOPIC_ID); 
    // println!("- METRICS (ID: {}) - Database metrics", METRICS_TOPIC_ID); 
    // println!("- HEALTH_STATUS (ID: {}) - Health status updates", HEALTH_STATUS_TOPIC_ID); 
    println!("- HEARTBEAT - Sent every 5 seconds"); 
    
    // 启动心跳发送线程 
    let pubsub_clone = Arc::new(Mutex::new(pubsub)); 
    let running = Arc::new(Mutex::new(true)); 
    let running_clone = running.clone(); 
    let server_clone = pubsub_clone.clone(); 
    
    let _heartbeat_thread = thread::spawn(move || { 
        let interval = Duration::from_secs(5); 
        while *running_clone.lock().unwrap() { 
            // 发送心跳帧 
            match server_clone.lock().unwrap().publish(0, b"heartbeat") { 
                Ok(_) => println!("Heartbeat sent"), 
                Err(e) => println!("Failed to send heartbeat: {:?}", e), 
            } 
            thread::sleep(interval); 
        } 
    }); 
    
    // 只有主节点执行SQL操作，从节点接收复制数据 
    if role == HARole::Master { 
        // 启动SQL操作线程，使用SQL动态创建表并操作数据 
        let db_clone = db_shared.clone(); 
        let server_clone_sql = pubsub_clone.clone(); 
        let running_clone_sql = running.clone(); 
        
        let _sql_thread = thread::spawn(move || { 
            let server_clone = server_clone_sql; 
            let db = db_clone; 
            
            println!("Starting SQL operations..."); 
            
            // 使用SQL语句创建表 
            let create_table_sql = "CREATE TABLE users (\n            id INTEGER PRIMARY KEY AUTOINCREMENT,\n            name TEXT NOT NULL,\n            age INTEGER,\n            email TEXT UNIQUE,\n            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP\n        )"; 
            
            match db.lock().unwrap().sql_query(create_table_sql) { 
                Ok(result) => println!("Created table users: {}", result.to_string()), 
                Err(e) => println!("Failed to create table: {:?}", e), 
            } 
            
            // 直接使用预定义的表内容主题ID发布，不需要动态注册 
            let users_table_topic = format!("table.{}", "users"); 
            let table_topic_id = 12; // 预定义的表内容主题ID 
            
            // 循环执行SQL操作，模拟数据变化 
            let mut counter = 0; 
            while *running_clone_sql.lock().unwrap() { 
                counter += 1; 
                
                // 插入数据 
                let insert_sql = format!("INSERT INTO users (name, age, email) VALUES ('User{}', {}, 'user{}@example.com')", counter, 20 + counter % 50, counter); 
                match db.lock().unwrap().sql_query(&insert_sql) { 
                    Ok(result) => { 
                        println!("Inserted record: {}", result.to_string()); 
                        // 发布表内容变更 
                        let table_data = format!("INSERT: Table=users, Record=User{}", counter); 
                        match server_clone.lock().unwrap().publish(table_topic_id, table_data.as_bytes()) { 
                            Ok(_) => println!("Published {}: {}", users_table_topic, table_data), 
                            Err(e) => println!("Failed to publish {}: {:?}", users_table_topic, e), 
                        } 
                    }, 
                    Err(e) => println!("Failed to insert record: {:?}", e), 
                } 
                
                // 等待一段时间 
                thread::sleep(Duration::from_secs(5)); 
                
                // 每10条记录更新一次 
                if counter % 10 == 0 { 
                    let update_sql = format!("UPDATE users SET age = age + 1 WHERE id = {}", counter - 5); 
                    match db.lock().unwrap().sql_query(&update_sql) { 
                        Ok(result) => { 
                            println!("Updated record: {}", result.to_string()); 
                            // 发布表内容变更 
                            let table_data = format!("UPDATE: Table=users, Record=User{}", counter - 5); 
                            match server_clone.lock().unwrap().publish(table_topic_id, table_data.as_bytes()) { 
                                Ok(_) => println!("Published {}: {}", users_table_topic, table_data), 
                                Err(e) => println!("Failed to publish {}: {:?}", users_table_topic, e), 
                            } 
                        }, 
                        Err(e) => println!("Failed to update record: {:?}", e), 
                    } 
                } 
                
                // 每15条记录删除一次 
                if counter % 15 == 0 { 
                    let delete_sql = format!("DELETE FROM users WHERE id = {}", counter - 10); 
                    match db.lock().unwrap().sql_query(&delete_sql) { 
                        Ok(result) => { 
                            println!("Deleted record: {}", result.to_string()); 
                            // 发布表内容变更 
                            let table_data = format!("DELETE: Table=users, Record=User{}", counter - 10); 
                            match server_clone.lock().unwrap().publish(table_topic_id, table_data.as_bytes()) { 
                                Ok(_) => println!("Published {}: {}", users_table_topic, table_data), 
                                Err(e) => println!("Failed to publish {}: {:?}", users_table_topic, e), 
                            } 
                        }, 
                        Err(e) => println!("Failed to delete record: {:?}", e), 
                    } 
                } 
                
                // 每5条记录查询一次，验证数据 
                if counter % 5 == 0 { 
                    let select_sql = "SELECT * FROM users ORDER BY id DESC LIMIT 5"; 
                    match db.lock().unwrap().sql_query(select_sql) { 
                        Ok(result) => println!("Query result: {}", result.to_string()), 
                        Err(e) => println!("Failed to query records: {:?}", e), 
                    } 
                } 
            } 
        }); 
    } else { 
        // 从节点定期查询数据，验证复制是否正常 
        let db_clone = db_shared.clone(); 
        let running_clone_slave = running.clone(); 
        
        let _slave_query_thread = thread::spawn(move || { 
            let db = db_clone; 
            
            println!("Starting slave query thread..."); 
            
            // 等待一段时间，确保主节点已经创建表 
            thread::sleep(Duration::from_secs(10)); 
            
            while *running_clone_slave.lock().unwrap() { 
                // 查询数据，验证复制是否正常 
                let select_sql = "SELECT * FROM users ORDER BY id DESC LIMIT 10"; 
                match db.lock().unwrap().sql_query(select_sql) { 
                    Ok(result) => println!("Slave query result: {}", result.to_string()), 
                    Err(e) => println!("Slave failed to query records: {:?}", e), 
                } 
                
                // 每10秒查询一次 
                thread::sleep(Duration::from_secs(10)); 
            } 
        }); 
    } 
    
    // 启动指标发布线程 
    let running_clone_metrics = running.clone(); 
    let server_clone_metrics = pubsub_clone.clone(); 
    let db_metrics_clone = db_shared.clone(); 
    
    let _metrics_thread = thread::spawn(move || { 
        let interval = Duration::from_millis(4000); 
        let db = db_metrics_clone; 
        
        while *running_clone_metrics.lock().unwrap() { 
            // 获取真实的数据库指标 
            let metrics = db.lock().unwrap().metrics.snapshot(); 
            let metrics_data = metrics.to_json(); 
            
            match server_clone_metrics.lock().unwrap().publish(METRICS_TOPIC_ID, metrics_data.as_bytes()) { 
                // Ok(_) => println!("Published METRICS: {}", metrics_data), 
                Ok(_) => {}, 
                Err(e) => println!("Failed to publish METRICS: {:?}", e), 
            } 
            thread::sleep(interval); 
        } 
    }); 
    
    // 启动健康状态发布线程 
    let running_clone_health = running.clone(); 
    let server_clone_health = pubsub_clone.clone(); 
    let db_health_clone = db_shared.clone(); 
    
    let _health_thread = thread::spawn(move || { 
        let interval = Duration::from_millis(5000); 
        let db = db_health_clone; 
        
        while *running_clone_health.lock().unwrap() { 
            // 获取真实的健康状态 
            let health = db.lock().unwrap().health_check(); 
            let health_data = health.to_json(); 
            
            match server_clone_health.lock().unwrap().publish(HEALTH_STATUS_TOPIC_ID, health_data.as_bytes()) { 
                // Ok(_) => println!("Published HEALTH_STATUS: {}", health_data.trim()), 
                Ok(_) => {}, 
                Err(e) => println!("Failed to publish HEALTH_STATUS: {:?}", e), 
            } 
            thread::sleep(interval); 
        } 
    }); 
    
    // 运行1分钟后自动停止 
    thread::sleep(Duration::from_secs(60)); 
    
    // 停止所有线程 
    *running.lock().unwrap() = false; 
    
    println!("RemDB Server stopped!"); 
}