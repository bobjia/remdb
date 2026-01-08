use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::{client::PubSubClient, server::{PubSubServer, WAL_LOG_TOPIC_ID, TABLE_CONTENT_TOPIC_ID}};

// 测试结果结构体
pub struct TestResult {
    pub test_name: String,
    pub passed: bool,
    pub message: String,
    pub duration: Duration,
}

impl TestResult {
    pub fn new(test_name: &str, passed: bool, message: &str, duration: Duration) -> Self {
        Self {
            test_name: test_name.to_string(),
            passed,
            message: message.to_string(),
            duration,
        }
    }
}

// 心跳机制测试
pub fn test_heartbeat_mechanism() -> TestResult {
    let start_time = std::time::Instant::now();
    println!("\n=== Running Heartbeat Mechanism Test ===");
    
    // 启动服务器
    let mut server = PubSubServer::new();
    server.start();
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let mut client = PubSubClient::new();
    client.start();
    
    // 等待客户端启动
    thread::sleep(Duration::from_secs(1));
    
    // 等待一段时间，检查连接状态
    thread::sleep(Duration::from_secs(6)); // 超过心跳间隔时间
    
    let connected = client.is_connected();
    
    // 停止客户端和服务器
    client.stop();
    server.stop();
    
    let duration = start_time.elapsed();
    
    if connected {
        TestResult::new(
            "Heartbeat Mechanism Test",
            true,
            "Client successfully detected server heartbeat and maintained connection",
            duration,
        )
    } else {
        TestResult::new(
            "Heartbeat Mechanism Test",
            false,
            "Client failed to detect server heartbeat",
            duration,
        )
    }
}

// 发布-订阅基础流程测试
pub fn test_pubsub_basic_flow() -> TestResult {
    let start_time = std::time::Instant::now();
    println!("\n=== Running Pub-Sub Basic Flow Test ===");
    
    // 启动服务器
    let mut server = PubSubServer::new();
    server.start();
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let mut client = PubSubClient::new();
    client.start();
    
    // 等待客户端启动
    thread::sleep(Duration::from_secs(1));
    
    // 用于存储接收到的消息
    let received_messages = Arc::new(Mutex::new(Vec::new()));
    
    // 定义消息回调
    let received_messages_clone = received_messages.clone();
    let callback = move |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        received_messages_clone.lock().unwrap().push((topic_id, msg));
        println!("Received message: topic={}, data={}", topic_id, msg);
        true
    };
    
    // 订阅主题
    let subscription_id = client.subscribe(1, callback);
    
    // 发送测试消息
    client.publish(1, b"test_message_123");
    
    // 等待消息接收
    thread::sleep(Duration::from_secs(2));
    
    // 检查是否接收到消息
    let messages = received_messages.lock().unwrap();
    let found = messages.iter().any(|(topic_id, msg)| *topic_id == 1 && msg == "test_message_123");
    
    // 取消订阅
    client.unsubscribe(subscription_id);
    
    // 停止客户端和服务器
    client.stop();
    server.stop();
    
    let duration = start_time.elapsed();
    
    if found {
        TestResult::new(
            "Pub-Sub Basic Flow Test",
            true,
            "Message successfully published and received",
            duration,
        )
    } else {
        TestResult::new(
            "Pub-Sub Basic Flow Test",
            false,
            "Message not received",
            duration,
        )
    }
}

// WAL日志主题专项测试
pub fn test_wal_log_topic() -> TestResult {
    let start_time = std::time::Instant::now();
    println!("\n=== Running WAL Log Topic Test ===");
    
    // 启动服务器
    let mut server = PubSubServer::new();
    server.start();
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let mut client = PubSubClient::new();
    client.start();
    
    // 等待客户端启动
    thread::sleep(Duration::from_secs(1));
    
    // 用于存储接收到的WAL日志
    let wal_messages = Arc::new(Mutex::new(Vec::new()));
    
    // 定义WAL日志回调
    let wal_messages_clone = wal_messages.clone();
    let callback = move |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        wal_messages_clone.lock().unwrap().push((topic_id, msg));
        println!("Received WAL log: topic={}, data={}", topic_id, msg);
        true
    };
    
    // 订阅WAL日志主题
    let subscription_id = client.subscribe(WAL_LOG_TOPIC_ID, callback);
    
    // 等待接收几个WAL日志消息
    thread::sleep(Duration::from_secs(5));
    
    // 检查是否接收到WAL日志消息
    let messages = wal_messages.lock().unwrap();
    let wal_log_count = messages.len();
    let valid_wal_logs = messages.iter().filter(|(topic_id, msg)| *topic_id == WAL_LOG_TOPIC_ID && msg.starts_with("WAL_LOG_")).count();
    
    // 取消订阅
    client.unsubscribe(subscription_id);
    
    // 停止客户端和服务器
    client.stop();
    server.stop();
    
    let duration = start_time.elapsed();
    
    if wal_log_count >= 3 && valid_wal_logs == wal_log_count {
        TestResult::new(
            "WAL Log Topic Test",
            true,
            format!("Successfully received {} valid WAL log messages", wal_log_count),
            duration,
        )
    } else {
        TestResult::new(
            "WAL Log Topic Test",
            false,
            format!("Failed to receive valid WAL log messages. Received: {}, Valid: {}", wal_log_count, valid_wal_logs),
            duration,
        )
    }
}

// 表内容主题专项测试
pub fn test_table_content_topic() -> TestResult {
    let start_time = std::time::Instant::now();
    println!("\n=== Running Table Content Topic Test ===");
    
    // 启动服务器
    let mut server = PubSubServer::new();
    server.start();
    
    // 等待服务器启动
    thread::sleep(Duration::from_secs(1));
    
    // 启动客户端
    let mut client = PubSubClient::new();
    client.start();
    
    // 等待客户端启动
    thread::sleep(Duration::from_secs(1));
    
    // 用于存储接收到的表内容消息
    let table_messages = Arc::new(Mutex::new(Vec::new()));
    
    // 定义表内容回调
    let table_messages_clone = table_messages.clone();
    let callback = move |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data).to_string();
        table_messages_clone.lock().unwrap().push((topic_id, msg));
        println!("Received table content: topic={}, data={}", topic_id, msg);
        true
    };
    
    // 订阅表内容主题
    let subscription_id = client.subscribe(TABLE_CONTENT_TOPIC_ID, callback);
    
    // 等待接收几个表内容消息
    thread::sleep(Duration::from_secs(6)); // 表内容主题每2秒发布一次，应该能收到3个消息
    
    // 检查是否接收到表内容消息
    let messages = table_messages.lock().unwrap();
    let table_content_count = messages.len();
    let valid_table_contents = messages.iter().filter(|(topic_id, msg)| *topic_id == TABLE_CONTENT_TOPIC_ID && msg.starts_with("TABLE_CONTENT_")).count();
    
    // 取消订阅
    client.unsubscribe(subscription_id);
    
    // 停止客户端和服务器
    client.stop();
    server.stop();
    
    let duration = start_time.elapsed();
    
    if table_content_count >= 2 && valid_table_contents == table_content_count {
        TestResult::new(
            "Table Content Topic Test",
            true,
            format!("Successfully received {} valid table content messages", table_content_count),
            duration,
        )
    } else {
        TestResult::new(
            "Table Content Topic Test",
            false,
            format!("Failed to receive valid table content messages. Received: {}, Valid: {}", table_content_count, valid_table_contents),
            duration,
        )
    }
}

// 运行所有测试
pub fn run_all_tests() {
    let mut results = Vec::new();
    
    // 运行所有测试
    results.push(test_heartbeat_mechanism());
    results.push(test_pubsub_basic_flow());
    results.push(test_wal_log_topic());
    results.push(test_table_content_topic());
    
    // 生成测试报告
    println!("\n=== Test Results Summary ===");
    println!("{:<40} {:<10} {:<60} {:<20}", "Test Name", "Status", "Message", "Duration");
    println!("{}", "-".repeat(140));
    
    let mut passed = 0;
    let mut failed = 0;
    
    for result in &results {
        let status = if result.passed { "PASSED" } else { "FAILED" };
        println!("{:<40} {:<10} {:<60} {:<20?}", result.test_name, status, result.message, result.duration);
        
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
        }
    }
    
    println!("{}", "-".repeat(140));
    println!("Total Tests: {}, Passed: {}, Failed: {}", results.len(), passed, failed);
    println!("Test Coverage: {:.1}%", (passed as f64 / results.len() as f64) * 100.0);
}
