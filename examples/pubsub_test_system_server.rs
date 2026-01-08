use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

// 主题ID定义
const WAL_LOG_TOPIC_ID: u16 = 1;
const TABLE_CONTENT_TOPIC_ID: u16 = 2;

fn main() {
    println!("Starting PubSub Test Server...");
    
    // 创建发布/订阅配置
    let config = PubSubConfig {
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
    let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
    pubsub.init().expect("Failed to initialize PubSub");
    
    // 注册主题
    pubsub.register_topic("wal_log", WAL_LOG_TOPIC_ID).expect("Failed to register WAL log topic");
    pubsub.register_topic("table_content", TABLE_CONTENT_TOPIC_ID).expect("Failed to register table content topic");
    
    println!("PubSub test server started successfully!");
    println!("Listening on UDP port 5555");
    println!("Topics available:");
    println!("- WAL_LOG_TOPIC (ID: {}) - Published every 1 second", WAL_LOG_TOPIC_ID);
    println!("- TABLE_CONTENT_TOPIC (ID: {}) - Published every 2 seconds", TABLE_CONTENT_TOPIC_ID);
    println!("- HEARTBEAT - Sent every 5 seconds");
    
    // 启动心跳发送线程
    let pubsub_clone = Arc::new(Mutex::new(pubsub));
    let running = Arc::new(Mutex::new(true));
    let running_clone = running.clone();
    let server_clone = pubsub_clone.clone();
    
    let _heartbeat_thread = thread::spawn(move || {
        let mut interval = Duration::from_secs(5);
        while *running_clone.lock().unwrap() {
            // 发送心跳帧
            match server_clone.lock().unwrap().publish(0, b"heartbeat") {
                Ok(_) => println!("Heartbeat sent"),
                Err(e) => println!("Failed to send heartbeat: {:?}", e),
            }
            thread::sleep(interval);
        }
    });
    
    // 启动WAL日志模拟发布线程
    let running_clone_wal = running.clone();
    let server_clone_wal = pubsub_clone.clone();
    
    let _wal_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(1000);
        let mut log_id = 0;
        while *running_clone_wal.lock().unwrap() {
            let wal_data = format!("WAL_LOG_{}: Operation=INSERT, Table=test_table, ID={}, Data={}", log_id, log_id, format!("test_data_{}", log_id));
            match server_clone_wal.lock().unwrap().publish(WAL_LOG_TOPIC_ID, wal_data.as_bytes()) {
                Ok(_) => println!("Published WAL log: {}", wal_data),
                Err(e) => println!("Failed to publish WAL log: {:?}", e),
            }
            log_id += 1;
            thread::sleep(interval);
        }
    });
    
    // 启动表内容变更模拟发布线程
    let running_clone_table = running.clone();
    let server_clone_table = pubsub_clone.clone();
    
    let _table_thread = thread::spawn(move || {
        let mut interval = Duration::from_millis(2000);
        let mut record_id = 0;
        while *running_clone_table.lock().unwrap() {
            let table_data = format!("TABLE_CONTENT_{}: Table=test_table, ID={}, Column1=value_{}, Column2={}", record_id, record_id, record_id, record_id * 2);
            match server_clone_table.lock().unwrap().publish(TABLE_CONTENT_TOPIC_ID, table_data.as_bytes()) {
                Ok(_) => println!("Published table content: {}", table_data),
                Err(e) => println!("Failed to publish table content: {:?}", e),
            }
            record_id += 1;
            thread::sleep(interval);
        }
    });
    
    // 运行10分钟后自动停止
    thread::sleep(Duration::from_secs(600));
    
    // 停止所有线程
    *running.lock().unwrap() = false;
    
    println!("PubSub test server stopped!");
}