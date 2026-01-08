// 发布/订阅模块集成测试

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;
    
    use remdb::pubsub::{PubSub, PubSubConfig, UdpMode, Result};
    
    #[test]
    fn test_pubsub_basic() {
        // 创建发布/订阅配置（使用广播模式）
        let config = PubSubConfig {
            udp_mode: UdpMode::Broadcast,
            multicast_addr: None,
            port: 5555,
            max_topics: 32,
            max_subscribers_per_topic: 16,
            buffer_size: 4096,
            enable_nack: true,
            retransmit_timeout: std::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: std::time::Duration::from_secs(10),
            frame_pool_size: 128,
        };
        
        // 创建发布/订阅实例
        let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
        
        // 初始化
        pubsub.init().expect("Failed to initialize PubSub");
        
        // 定义测试回调函数
        fn test_callback(_topic_id: u16, data: &[u8]) -> bool {
            assert_eq!(data, b"test data");
            true
        }
        
        // 订阅主题
        let subscription_id = pubsub.subscribe(0, test_callback).expect("Failed to subscribe");
        assert!(subscription_id > 0);
        
        // 发布数据
        pubsub.publish(0, b"test data").expect("Failed to publish");
        
        // 注意：由于是异步通信，这里无法直接测试回调是否被调用
        // 实际测试需要更复杂的异步测试框架
        
        // 取消订阅
        pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
    }
    
    #[test]
    fn test_pubsub_multicast() {
        // 创建发布/订阅配置（组播模式）
        let multicast_addr = IpAddr::from_str("224.0.0.1").unwrap();
        
        let config = PubSubConfig {
            udp_mode: UdpMode::Multicast,
            multicast_addr: Some(multicast_addr),
            port: 5556,
            max_topics: 32,
            max_subscribers_per_topic: 16,
            buffer_size: 4096,
            enable_nack: true,
            retransmit_timeout: std::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: std::time::Duration::from_secs(10),
            frame_pool_size: 128,
        };
        
        // 创建发布/订阅实例
        let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
        
        // 初始化
        pubsub.init().expect("Failed to initialize PubSub");
        
        // 订阅主题
        let callback = |_topic_id: u16, _data: &[u8]| -> bool {
            true
        };
        
        let subscription_id = pubsub.subscribe(1, callback).expect("Failed to subscribe");
        
        // 发布数据
        pubsub.publish(1, b"multicast test").expect("Failed to publish multicast");
        
        // 取消订阅
        pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
    }
    
    #[test]
    fn test_pubsub_broadcast() {
        // 创建发布/订阅配置（广播模式）
        let config = PubSubConfig {
            udp_mode: UdpMode::Broadcast,
            multicast_addr: None,
            port: 6667, // 使用不同的端口
            max_topics: 32,
            max_subscribers_per_topic: 16,
            buffer_size: 4096,
            enable_nack: true,
            retransmit_timeout: std::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: std::time::Duration::from_secs(10),
            frame_pool_size: 128,
        };
        
        // 创建发布/订阅实例
        let mut pubsub = PubSub::new(config).expect("Failed to create PubSub instance");
        
        // 初始化
        pubsub.init().expect("Failed to initialize PubSub");
        
        // 订阅主题
        let callback = |_topic_id: u16, _data: &[u8]| -> bool {
            true
        };
        
        let subscription_id = pubsub.subscribe(2, callback).expect("Failed to subscribe");
        
        // 发布数据
        pubsub.publish(2, b"broadcast test").expect("Failed to publish broadcast");
        
        // 取消订阅
        pubsub.unsubscribe(subscription_id).expect("Failed to unsubscribe");
        
        // 关闭实例
        pubsub.shutdown().expect("Failed to shutdown PubSub");
    }
}
