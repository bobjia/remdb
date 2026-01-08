use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

fn main() {
    println!("Starting PubSub Test Client...");
    
    // 创建发布/订阅配置
    let config = PubSubConfig {
        udp_mode: UdpMode::Multicast,
        multicast_addr: Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(224, 0, 0, 1))),
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
    
    // 定义心跳回调（不捕获变量）
    let heartbeat_callback = |_topic_id: u16, data: &[u8]| -> bool {
        if data == b"heartbeat" {
            println!("Received heartbeat from server");
        }
        true
    };
    
    // 订阅心跳主题
    pubsub.subscribe(0, heartbeat_callback).expect("Failed to subscribe to heartbeat topic");
    
    // 定义WAL日志回调
    let wal_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received WAL log: topic={}, data={}", topic_id, msg);
        true
    };
    
    // 订阅WAL日志主题
    pubsub.subscribe(1, wal_callback).expect("Failed to subscribe to WAL log topic");
    
    // 定义表内容回调
    let table_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received table content: topic={}, data={}", topic_id, msg);
        true
    };
    
    // 订阅表内容主题
    pubsub.subscribe(2, table_callback).expect("Failed to subscribe to table content topic");
    
    // 获取实际绑定的端口
    let actual_port = pubsub.get_actual_port().expect("Failed to get actual port");
    
    // 启动接收循环线程
    let pubsub_clone = Arc::new(Mutex::new(pubsub));
    
    let _receive_thread = thread::spawn(move || {
        let mut pubsub = pubsub_clone.lock().unwrap();
        pubsub.receive_loop();
    });
    
    println!("PubSub test client started successfully!");
    println!("Listening for messages on UDP port {}", actual_port);
    println!("Subscribed to topics: heartbeat(0), wal_log(1), table_content(2)");
    println!("Client is running and receiving messages...");
    
    // 运行2分钟后自动停止
    thread::sleep(Duration::from_secs(120));
    
    println!("PubSub test client stopped!");
}