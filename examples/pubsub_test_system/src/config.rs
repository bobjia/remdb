use std::time::Duration;
use crate::logger::LogLevel;

// 服务器配置结构体
#[derive(Debug, Default)]
pub struct ServerConfig {
    // 网络配置
    pub udp_port: u16,
    pub max_topics: usize,
    pub max_subscribers_per_topic: usize,
    pub buffer_size: usize,
    
    // 可靠性配置
    pub enable_nack: bool,
    pub retransmit_timeout_ms: u64,
    pub max_retransmits: usize,
    
    // 心跳配置
    pub heartbeat_interval_secs: u64,
    
    // 发布频率配置
    pub wal_publish_interval_ms: u64,
    pub table_content_publish_interval_ms: u64,
    
    // 日志配置
    pub log_level: LogLevel,
}

// 客户端配置结构体
#[derive(Debug, Default)]
pub struct ClientConfig {
    // 网络配置
    pub udp_port: u16,
    pub buffer_size: usize,
    
    // 心跳配置
    pub heartbeat_timeout_secs: u64,
    
    // 日志配置
    pub log_level: LogLevel,
}

// 测试配置结构体
#[derive(Debug, Default)]
pub struct TestConfig {
    // 测试时长配置
    pub heartbeat_test_duration_secs: u64,
    pub basic_flow_test_duration_secs: u64,
    pub wal_log_test_duration_secs: u64,
    pub table_content_test_duration_secs: u64,
    
    // 消息配置
    pub test_message_size: usize,
    pub message_send_interval_ms: u64,
    
    // 连接配置
    pub num_connections: usize,
    
    // 日志配置
    pub log_level: LogLevel,
}

// 获取默认服务器配置
pub fn default_server_config() -> ServerConfig {
    ServerConfig {
        udp_port: 5555,
        max_topics: 32,
        max_subscribers_per_topic: 16,
        buffer_size: 4096,
        enable_nack: true,
        retransmit_timeout_ms: 100,
        max_retransmits: 3,
        heartbeat_interval_secs: 5,
        wal_publish_interval_ms: 1000,
        table_content_publish_interval_ms: 2000,
        log_level: LogLevel::Info,
    }
}

// 获取默认客户端配置
pub fn default_client_config() -> ClientConfig {
    ClientConfig {
        udp_port: 5555,
        buffer_size: 4096,
        heartbeat_timeout_secs: 15,
        log_level: LogLevel::Info,
    }
}

// 获取默认测试配置
pub fn default_test_config() -> TestConfig {
    TestConfig {
        heartbeat_test_duration_secs: 10,
        basic_flow_test_duration_secs: 5,
        wal_log_test_duration_secs: 5,
        table_content_test_duration_secs: 6,
        test_message_size: 64,
        message_send_interval_ms: 500,
        num_connections: 1,
        log_level: LogLevel::Info,
    }
}
