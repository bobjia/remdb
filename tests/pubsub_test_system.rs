#![cfg(feature = "pubsub")]

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

// 主题ID定义
const WAL_LOG_TOPIC_ID: u16 = 1;
const TABLE_CONTENT_TOPIC_ID: u16 = 2;

// 全局变量用于存储测试结果
static mut TEST_RESULTS: Vec<(String, bool, String)> = Vec::new();

// 心跳机制测试
#[test]
fn test_heartbeat_mechanism() {
    println!("\n=== Running Heartbeat Mechanism Test ===");
    
    // 启动服务器
    let server_config = PubSubConfig {
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
    
    let mut server = PubSub::new(server_config).expect("Failed to create server PubSub instance");
    server.init().expect("Failed to initialize server PubSub");
    
    // 注册主题
    server.register_topic("wal_log", WAL_LOG_TOPIC_ID).expect("Failed to register WAL log topic");
    server.register_topic("table_content", TABLE_CONTENT_TOPIC_ID).expect("Failed to register table content topic");
    
    // 启动心跳发送线程
    let server_clone = Arc::new(Mutex::new(server));
    let running = Arc::new(Mutex::new(true));
    let running_clone = running.clone();
    let server_clone_thread = server_clone.clone();
    
    let _heartbeat_thread = thread::spawn(move || {
        while *running_clone.lock().unwrap() {
            server_clone_thread.lock().unwrap().publish(0, b"heartbeat").expect("Failed to send heartbeat");
            thread::sleep(Duration::from_secs(1));
        }
    });
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let client_config = PubSubConfig {
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
    
    let mut client = PubSub::new(client_config).expect("Failed to create client PubSub instance");
    client.init().expect("Failed to initialize client PubSub");
    
    // 订阅心跳主题
    client.subscribe(0, |_topic_id: u16, data: &[u8]| -> bool {
        if data == b"heartbeat" {
            println!("Received heartbeat from server");
        }
        true
    }).expect("Failed to subscribe to heartbeat topic");
    
    // 等待一段时间，确保心跳正常
    thread::sleep(Duration::from_secs(3));
    
    // 停止心跳线程
    *running.lock().unwrap() = false;
    
    println!("Heartbeat Mechanism Test completed");
}

// 发布-订阅基础流程测试
#[test]
fn test_pubsub_basic_flow() {
    println!("\n=== Running Pub-Sub Basic Flow Test ===");
    
    // 启动服务器
    let server_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5556,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut server = PubSub::new(server_config).expect("Failed to create server PubSub instance");
    server.init().expect("Failed to initialize server PubSub");
    
    // 启动客户端
    let client_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5556,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut client = PubSub::new(client_config).expect("Failed to create client PubSub instance");
    client.init().expect("Failed to initialize client PubSub");
    
    // 订阅主题
    client.subscribe(1, |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        println!("Received message: topic={}, data={}", topic_id, msg);
        true
    }).expect("Failed to subscribe");
    
    // 发送测试消息
    client.publish(1, b"test_message_123").expect("Failed to publish");
    
    // 等待消息接收
    thread::sleep(Duration::from_secs(2));
    
    println!("Pub-Sub Basic Flow Test completed");
}

// WAL日志主题专项测试
#[test]
fn test_wal_log_topic() {
    println!("\n=== Running WAL Log Topic Test ===");
    
    // 启动服务器
    let server_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5557,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut server = PubSub::new(server_config).expect("Failed to create server PubSub instance");
    server.init().expect("Failed to initialize server PubSub");
    
    // 注册主题
    server.register_topic("wal_log", WAL_LOG_TOPIC_ID).expect("Failed to register WAL log topic");
    
    // 启动WAL日志模拟发布线程
    let server_clone = Arc::new(Mutex::new(server));
    let running = Arc::new(Mutex::new(true));
    let running_clone = running.clone();
    let server_clone_thread = server_clone.clone();
    
    let _wal_thread = thread::spawn(move || {
        let mut log_id = 0;
        while *running_clone.lock().unwrap() {
            let wal_data = format!("WAL_LOG_{}: Operation=INSERT, Table=test_table, ID={}, Data={}", log_id, log_id, format!("test_data_{}", log_id));
            server_clone_thread.lock().unwrap().publish(WAL_LOG_TOPIC_ID, wal_data.as_bytes()).expect("Failed to publish WAL log");
            log_id += 1;
            thread::sleep(Duration::from_millis(1000));
        }
    });
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let client_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5557,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut client = PubSub::new(client_config).expect("Failed to create client PubSub instance");
    client.init().expect("Failed to initialize client PubSub");
    
    // 订阅WAL日志主题
    client.subscribe(WAL_LOG_TOPIC_ID, |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        println!("Received WAL log: topic={}, data={}", topic_id, msg);
        true
    }).expect("Failed to subscribe to WAL log topic");
    
    // 等待接收几个WAL日志消息
    thread::sleep(Duration::from_secs(5));
    
    // 停止WAL日志发布线程
    *running.lock().unwrap() = false;
    
    println!("WAL Log Topic Test completed");
}

// 表内容主题专项测试
#[test]
fn test_table_content_topic() {
    println!("\n=== Running Table Content Topic Test ===");
    
    // 启动服务器
    let server_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5558,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut server = PubSub::new(server_config).expect("Failed to create server PubSub instance");
    server.init().expect("Failed to initialize server PubSub");
    
    // 注册主题
    server.register_topic("table_content", TABLE_CONTENT_TOPIC_ID).expect("Failed to register table content topic");
    
    // 启动表内容变更模拟发布线程
    let server_clone = Arc::new(Mutex::new(server));
    let running = Arc::new(Mutex::new(true));
    let running_clone = running.clone();
    let server_clone_thread = server_clone.clone();
    
    let _table_thread = thread::spawn(move || {
        let mut record_id = 0;
        while *running_clone.lock().unwrap() {
            let table_data = format!("TABLE_CONTENT_{}: Table=test_table, ID={}, Column1=value_{}, Column2={}", record_id, record_id, record_id, record_id * 2);
            server_clone_thread.lock().unwrap().publish(TABLE_CONTENT_TOPIC_ID, table_data.as_bytes()).expect("Failed to publish table content");
            record_id += 1;
            thread::sleep(Duration::from_millis(2000));
        }
    });
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let client_config = PubSubConfig {
        udp_mode: UdpMode::Broadcast,
        multicast_addr: None,
        port: 5558,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout: Duration::from_millis(100),
        max_retransmits: 3,
        heartbeat_interval: Duration::from_secs(5),
        frame_pool_size: 128,
    };
    
    let mut client = PubSub::new(client_config).expect("Failed to create client PubSub instance");
    client.init().expect("Failed to initialize client PubSub");
    
    // 订阅表内容主题
    client.subscribe(TABLE_CONTENT_TOPIC_ID, |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        println!("Received table content: topic={}, data={}", topic_id, msg);
        true
    }).expect("Failed to subscribe to table content topic");
    
    // 等待接收几个表内容消息
    thread::sleep(Duration::from_secs(6));
    
    // 停止表内容发布线程
    *running.lock().unwrap() = false;
    
    println!("Table Content Topic Test completed");
}
