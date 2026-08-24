#![cfg(feature = "pubsub")]

use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    println!("Starting PubSub Test Client...");

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

    // 定义心跳回调
    let heartbeat_callback = |_topic_id: u16, data: &[u8]| -> bool {
        if data == b"heartbeat" {
            println!("Received heartbeat from server");
        }
        true
    };

    // 订阅心跳主题
    pubsub
        .subscribe(0, heartbeat_callback)
        .expect("Failed to subscribe to heartbeat topic");

    // 定义WAL日志回调（用于所有WAL相关主题）
    let wal_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        let topic_name = match topic_id {
            1 => "WAL_INSERT",
            2 => "WAL_UPDATE",
            3 => "WAL_DELETE",
            4 => "WAL_TIMESERIES_INSERT",
            5 => "WAL_COMMIT",
            6 => "WAL_ABORT",
            7 => "WAL_CHECKPOINT",
            8 => "WAL_ALL",
            _ => "UNKNOWN_WAL_TOPIC",
        };
        println!("Received {}: {}", topic_name, msg);
        true
    };

    // 订阅所有WAL主题
    pubsub
        .subscribe(1, wal_callback)
        .expect("Failed to subscribe to WAL_INSERT topic");
    pubsub
        .subscribe(2, wal_callback)
        .expect("Failed to subscribe to WAL_UPDATE topic");
    pubsub
        .subscribe(3, wal_callback)
        .expect("Failed to subscribe to WAL_DELETE topic");
    pubsub
        .subscribe(4, wal_callback)
        .expect("Failed to subscribe to WAL_TIMESERIES_INSERT topic");
    pubsub
        .subscribe(5, wal_callback)
        .expect("Failed to subscribe to WAL_COMMIT topic");
    pubsub
        .subscribe(6, wal_callback)
        .expect("Failed to subscribe to WAL_ABORT topic");
    pubsub
        .subscribe(7, wal_callback)
        .expect("Failed to subscribe to WAL_CHECKPOINT topic");
    pubsub
        .subscribe(8, wal_callback)
        .expect("Failed to subscribe to WAL_ALL topic");

    // 定义表内容回调
    let table_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received TABLE_CONTENT (ID: {}): {}", topic_id, msg);
        true
    };

    // 订阅表内容主题
    pubsub
        .subscribe(12, table_callback)
        .expect("Failed to subscribe to table.test_table topic");

    // 定义表创建/删除事件回调
    let tables_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received TABLES (ID: {}): {}", topic_id, msg);
        true
    };

    // 订阅表创建/删除事件主题
    pubsub
        .subscribe(9, tables_callback)
        .expect("Failed to subscribe to TABLES topic");

    // 定义指标回调
    let metrics_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received METRICS (ID: {}): {}", topic_id, msg);
        true
    };

    // 订阅指标主题
    pubsub
        .subscribe(10, metrics_callback)
        .expect("Failed to subscribe to METRICS topic");

    // 定义健康状态回调
    let health_callback = |topic_id: u16, data: &[u8]| -> bool {
        let msg = String::from_utf8_lossy(data);
        println!("Received HEALTH_STATUS (ID: {}): {}", topic_id, msg);
        true
    };

    // 订阅健康状态主题
    pubsub
        .subscribe(11, health_callback)
        .expect("Failed to subscribe to HEALTH_STATUS topic");

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
    println!("Subscribed to topics:");
    println!("- HEARTBEAT (ID: 0)");
    println!("- WAL_INSERT (ID: 1)");
    println!("- WAL_UPDATE (ID: 2)");
    println!("- WAL_DELETE (ID: 3)");
    println!("- WAL_TIMESERIES_INSERT (ID: 4)");
    println!("- WAL_COMMIT (ID: 5)");
    println!("- WAL_ABORT (ID: 6)");
    println!("- WAL_CHECKPOINT (ID: 7)");
    println!("- WAL_ALL (ID: 8)");
    println!("- TABLES (ID: 9)");
    println!("- METRICS (ID: 10)");
    println!("- HEALTH_STATUS (ID: 11)");
    println!("- table.test_table (ID: 12)");
    println!("Client is running and receiving messages...");

    // 运行1分钟后自动停止
    thread::sleep(Duration::from_secs(60));

    println!("PubSub test client stopped!");
}
