// pubsub_example.rs
// 发布/订阅功能示例
#![cfg(feature = "pubsub")]

use std::time::Duration;
use remdb::pubsub::{PubSub, PubSubConfig, UdpMode};

fn main() {
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
        heartbeat_interval: Duration::from_secs(10),
        frame_pool_size: 128,
    };
    
    // 创建发布/订阅实例
    let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
    
    // 初始化
    pubsub.init().expect("Failed to initialize PubSub");
    
    // 定义订阅回调
    let callback = |topic_id: u16, data: &[u8]| -> bool {
        println!("Received data on topic {}: {:?}", topic_id, String::from_utf8_lossy(data));
        true
    };
    
    // 订阅主题
    let subscription_id = pubsub.subscribe(0, callback).expect("Failed to subscribe");
    println!("Subscribed to topic 0 with subscription ID: {}", subscription_id);
    
    // 发布数据
    for i in 0..5 {
        // 使用 let 绑定创建更长生命周期的值
        let msg = format!("Message {}", i);
        let data = msg.as_bytes();
        
        println!("Publishing message {} on topic 0", i);
        pubsub.publish(0, data).expect("Failed to publish");
        
        // 模拟延迟
        std::thread::sleep(Duration::from_millis(500));
    }
    
    // 等待一段时间，接收发布的数据
    std::thread::sleep(Duration::from_secs(1));
    
    // 取消订阅
    pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
    println!("Unsubscribed from topic 0");
    
    // 等待一段时间，确保所有操作完成
    std::thread::sleep(Duration::from_millis(500));
    
    println!("PubSub example completed");
}
