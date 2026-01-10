// 演示pubsub通配符订阅功能

use remdb::pubsub::{self, PubSubConfig, WILDCARD_TOPIC_ID};
use std::thread::sleep;
use std::time::Duration;

// 回调函数：打印收到的消息
// 注意：避免在回调中调用pubsub::get_topic_name，因为会导致可变引用冲突
fn print_callback(topic_id: u16, data: &[u8]) -> bool {
    // 直接使用topic_id，避免调用pubsub API导致可变引用冲突
    println!("Callback received: topic_id={}, data={:?}", topic_id, data);
    true // 继续订阅
}

fn main() {
    // 1. 初始化pubsub系统 - 使用广播模式避免单播目标地址问题
    let config = PubSubConfig {
        udp_mode: remdb::pubsub::UdpMode::Broadcast, // 广播模式避免单播目标地址问题
        port: 5555,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        ..Default::default() // 必须是最后一个字段，后面不能有逗号
    };
    pubsub::init(config).unwrap();
    
    // 2. 注册主题名称到ID的映射（使用未被预定义主题占用的ID，预定义主题占用了1-13）
    pubsub::register_topic("table1", 14).unwrap();
    pubsub::register_topic("table2", 15).unwrap();
    pubsub::register_topic("table3", 16).unwrap();
    
    // 3. 订阅特定主题
    let sub1 = pubsub::subscribe(14, print_callback).unwrap();
    println!("Subscribed to table1 with ID {}", sub1);
    
    let sub2 = pubsub::subscribe(15, print_callback).unwrap();
    println!("Subscribed to table2 with ID {}", sub2);
    
    // 4. 订阅所有主题（使用通配符）
    let sub_wildcard = pubsub::subscribe(WILDCARD_TOPIC_ID, print_callback).unwrap();
    println!("Subscribed to all topics with ID {}", sub_wildcard);
    
    // 5. 发布消息到不同主题
    sleep(Duration::from_millis(100));
    println!("\nPublishing to table1...");
    pubsub::publish(14, b"Hello from table1").unwrap();
    
    sleep(Duration::from_millis(100));
    println!("\nPublishing to table2...");
    pubsub::publish(15, b"Hello from table2").unwrap();
    
    sleep(Duration::from_millis(100));
    println!("\nPublishing to table3...");
    pubsub::publish(16, b"Hello from table3").unwrap();
    
    // 6. 等待消息处理
    sleep(Duration::from_millis(100));
    
    // 7. 取消订阅
    pubsub::unsubscribe(sub1).unwrap();
    pubsub::unsubscribe(sub2).unwrap();
    pubsub::unsubscribe(sub_wildcard).unwrap();
    
    // 8. 停止pubsub系统
    pubsub::shutdown().unwrap();
    
    println!("\nPubSub system shutdown");
}
