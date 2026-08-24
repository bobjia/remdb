use crate::try_lock;

// 基于UDP的高可靠数据订阅与发布模块

pub mod crc32;
pub mod protocol;
pub mod publisher;
pub mod subscriber;
pub mod topics;
pub mod ttl_ringbuffer;
pub mod udp;

use crate::pubsub::topics::*;
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use protocol::ProtocolFrame;

// 公共错误类型
#[derive(Debug, PartialEq, Eq)]
pub enum PubSubError {
    // 初始化错误
    InitFailed,
    // 网络错误
    NetworkError,
    // 无效参数
    InvalidParameter,
    // 超出资源限制
    ResourceExhausted,
    // 无效帧格式
    InvalidFrameFormat,
    // CRC校验失败
    CrcCheckFailed,
    // 主题不存在
    TopicNotFound,
    // 订阅不存在
    SubscriptionNotFound,
    // 不支持的操作
    UnsupportedOperation,
}

impl fmt::Display for PubSubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PubSubError::InitFailed => write!(f, "PubSub initialization failed"),
            PubSubError::NetworkError => write!(f, "Network error"),
            PubSubError::InvalidParameter => write!(f, "Invalid parameter"),
            PubSubError::ResourceExhausted => write!(f, "Resource exhausted"),
            PubSubError::InvalidFrameFormat => write!(f, "Invalid frame format"),
            PubSubError::CrcCheckFailed => write!(f, "CRC check failed"),
            PubSubError::TopicNotFound => write!(f, "Topic not found"),
            PubSubError::SubscriptionNotFound => write!(f, "Subscription not found"),
            PubSubError::UnsupportedOperation => write!(f, "Unsupported operation"),
        }
    }
}

// 公共结果类型
pub type Result<T> = core::result::Result<T, PubSubError>;

// 传输模式枚举
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UdpMode {
    // 单播模式
    Unicast,
    // 广播模式
    Broadcast,
    // 组播模式
    Multicast,
}

// 订阅回调类型
type PubSubCallback = fn(topic_id: u16, data: &[u8]) -> bool;

// 订阅ID类型
type SubscriptionId = usize;

// 通配符主题ID常量（用于订阅所有主题）
pub const WILDCARD_TOPIC_ID: u16 = 0xFFFF;

// 发布/订阅配置
#[derive(Debug, Clone)]
pub struct PubSubConfig {
    // UDP配置
    pub udp_mode: UdpMode,
    pub multicast_addr: Option<std::net::IpAddr>,
    pub port: u16,

    // 资源配置
    pub max_topics: usize,
    pub max_subscribers_per_topic: usize,
    pub buffer_size: usize,

    // 可靠性配置
    pub enable_nack: bool,
    pub retransmit_timeout: core::time::Duration,
    pub max_retransmits: usize,
    pub heartbeat_interval: core::time::Duration,

    // 内存池配置
    pub frame_pool_size: usize,
}

impl Default for PubSubConfig {
    fn default() -> Self {
        Self {
            udp_mode: UdpMode::Unicast,
            multicast_addr: None,
            port: 5555,
            max_topics: 32,
            max_subscribers_per_topic: 16,
            buffer_size: 4096,
            enable_nack: true,
            retransmit_timeout: core::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: core::time::Duration::from_secs(10),
            frame_pool_size: 128,
        }
    }
}

// 全局发布/订阅实例
static mut PUB_SUB_INSTANCE: Option<PubSub> = None;
// 原子锁标志，用于线程安全的初始化
static PUB_SUB_INIT_LOCK: AtomicBool = AtomicBool::new(false);

// 发布/订阅系统核心结构体
pub struct PubSub {
    config: PubSubConfig,
    subscribers: subscriber::SubscriberManager,
    publisher: publisher::Publisher,
    udp_socket: udp::UdpSocket,
    is_running: bool,
}

impl PubSub {
    /// 创建新的发布/订阅实例
    pub fn new(config: PubSubConfig) -> Result<Self> {
        // 创建UDP套接字
        let udp_socket = udp::UdpSocket::new(
            config.udp_mode,
            config.multicast_addr,
            config.port,
            config.buffer_size,
        )?;

        // 创建订阅者管理器
        let subscribers = subscriber::SubscriberManager::new(
            config.max_topics,
            config.max_subscribers_per_topic,
        )?;

        // 创建发布者
        let publisher = publisher::Publisher::new(
            config.enable_nack,
            config.retransmit_timeout,
            config.max_retransmits,
        )?;

        Ok(Self {
            config,
            subscribers,
            publisher,
            udp_socket,
            is_running: false,
        })
    }

    /// 获取实际使用的端口
    pub fn get_actual_port(&self) -> Result<u16> {
        self.udp_socket.get_port()
    }

    /// 初始化发布/订阅系统
    pub fn init(&mut self) -> Result<()> {
        // 初始化UDP套接字
        self.udp_socket.init()?;

        // 标记为运行中
        self.is_running = true;

        Ok(())
    }

    /// 启动接收线程（仅POSIX平台）
    #[cfg(feature = "posix")]
    pub fn start_receiver(&mut self) -> Result<()> {
        if !self.is_running {
            return Err(PubSubError::InitFailed);
        }

        // 使用Arc<Mutex<>>包装PubSub实例，确保线程安全
        let pubsub = std::sync::Arc::new(std::sync::Mutex::new(self.clone()));

        // 启动接收线程
        std::thread::spawn(move || {
            let mut pubsub = try_lock!(pubsub);
            pubsub.receive_loop();
        });

        Ok(())
    }

    /// 克隆PubSub实例
    pub fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            subscribers: self.subscribers.clone(),
            publisher: self.publisher.clone(),
            udp_socket: self.udp_socket.clone(),
            is_running: self.is_running,
        }
    }

    /// 接收循环（外部可调用）
    pub fn receive_loop(&mut self) {
        // 分配接收缓冲区
        let mut buf = vec![0; self.config.buffer_size];

        // 分配重传帧缓冲区
        let mut retransmit_frames = alloc::vec::Vec::new();

        loop {
            // 检查待重传的数据
            retransmit_frames.clear();
            if let Ok(frames) = self.publisher.check_timeouts() {
                retransmit_frames.extend(frames);
            }

            // 发送待重传的数据
            for frame in &retransmit_frames {
                let bytes = frame.to_bytes();
                if self.udp_socket.send(&bytes).is_err() {
                    // 发送失败，记录错误或重试
                }
            }

            // 接收数据
            match self.udp_socket.recv(&mut buf) {
                Ok(len) if len > 0 => {
                    // 处理接收到的数据
                    self.handle_received_data(&buf[..len]);
                }
                Err(_) => {
                    // 接收错误，继续循环
                    continue;
                }
                _ => {
                    // 接收到0字节，继续循环
                    continue;
                }
            }
        }
    }

    /// 处理接收到的数据
    fn handle_received_data(&mut self, data: &[u8]) {
        // 解析协议帧
        match ProtocolFrame::from_bytes(data) {
            Ok(frame) => {
                // 处理不同类型的帧
                match frame.frame_type() {
                    protocol::FrameType::Data => {
                        // 处理数据帧
                        self.handle_data_frame(frame);
                    }
                    protocol::FrameType::Nack => {
                        // 处理NACK帧
                        self.handle_nack_frame(frame);
                    }
                    protocol::FrameType::Heartbeat => {
                        // 处理心跳帧
                        self.handle_heartbeat_frame(frame);
                    }
                }
            }
            Err(e) => {
                // 解析错误，记录或忽略
                match e {
                    PubSubError::CrcCheckFailed => {
                        // CRC校验失败，可能需要发送NACK
                        // 这里简化处理，直接忽略
                    }
                    _ => {
                        // 其他错误，忽略
                    }
                }
            }
        }
    }

    /// 处理数据帧
    fn handle_data_frame(&mut self, frame: protocol::ProtocolFrame) {
        // 获取帧信息
        let topic_id = frame.topic_id();
        let seq_num = frame.seq_num();
        let payload = frame.payload();

        // 检查序列号
        if let Some(missing_seq_num) = self.subscribers.check_seq_num(topic_id, seq_num) {
            // 序列号不一致，生成NACK帧
            let nack_frame = protocol::ProtocolFrame::new_nack_frame(missing_seq_num, topic_id);
            let nack_bytes = nack_frame.to_bytes();
            // 发送NACK帧
            if self.udp_socket.send(&nack_bytes).is_err() {
                // 发送失败，记录错误
            }
        } else {
            // 序列号正确，将数据分发给订阅者
            if let Err(_e) = self.subscribers.handle_data(topic_id, payload) {
                // 处理分发错误
            }
        }
    }

    /// 处理NACK帧
    fn handle_nack_frame(&mut self, frame: protocol::ProtocolFrame) {
        // 获取帧信息
        let topic_id = frame.topic_id();
        let seq_num = frame.seq_num();

        // 处理NACK，获取需要重传的帧
        match self.publisher.handle_nack(seq_num, topic_id) {
            Ok(frames) => {
                // 发送重传帧
                for frame in frames {
                    let bytes = frame.to_bytes();
                    if self.udp_socket.send(&bytes).is_err() {
                        // 发送失败，记录错误
                    }
                }
            }
            Err(_) => {
                // 处理错误
            }
        }
    }

    /// 处理心跳帧
    fn handle_heartbeat_frame(&mut self, _frame: protocol::ProtocolFrame) {
        // 心跳帧处理：更新订阅者活跃状态
        // 这里简化处理，直接调用清理方法
        let heartbeat_timeout = self.config.heartbeat_interval.as_millis() as u64 * 3;
        if let Err(_e) = self.subscribers.cleanup_inactive(heartbeat_timeout) {
            // 处理清理错误
        }
    }

    /// 订阅主题
    pub fn subscribe(&mut self, topic_id: u16, callback: PubSubCallback) -> Result<SubscriptionId> {
        self.subscribers.subscribe(topic_id, callback)
    }

    /// 取消订阅
    pub fn unsubscribe(&mut self, subscription_id: SubscriptionId) -> Result<()> {
        self.subscribers.unsubscribe(subscription_id)
    }

    /// 发布数据
    pub fn publish(&mut self, topic_id: u16, data: &[u8]) -> Result<()> {
        // 生成协议帧
        let frame = self.publisher.create_frame(topic_id, data)?;

        // 将帧转换为字节数组
        let bytes = frame.to_bytes();

        // 发送数据
        self.udp_socket.send(&bytes)?;

        Ok(())
    }

    /// 注册主题名称到ID的映射
    pub fn register_topic(&mut self, topic_name: &'static str, topic_id: u16) -> Result<()> {
        self.subscribers.register_topic(topic_name, topic_id)
    }

    /// 根据主题名称获取ID
    pub fn get_topic_id(&self, topic_name: &str) -> Option<u16> {
        self.subscribers.get_topic_id(topic_name)
    }

    /// 根据ID获取主题名称
    pub fn get_topic_name(&self, topic_id: u16) -> Option<&'static str> {
        self.subscribers.get_topic_name(topic_id)
    }

    /// 停止发布/订阅系统
    pub fn shutdown(&mut self) -> Result<()> {
        // 关闭UDP套接字
        self.udp_socket.close()?;

        // 标记为停止
        self.is_running = false;

        Ok(())
    }
}

/// 初始化全局发布/订阅实例
pub fn init(config: PubSubConfig) -> Result<()> {
    unsafe {
        // 使用原子交换实现自旋锁，确保线程安全初始化
        while PUB_SUB_INIT_LOCK.swap(true, Ordering::Acquire) {
            // 自旋等待锁释放
            core::hint::spin_loop();
        }

        // 确保锁在函数结束时释放
        struct LockGuard;
        impl Drop for LockGuard {
            fn drop(&mut self) {
                PUB_SUB_INIT_LOCK.store(false, Ordering::Release);
            }
        }
        let _guard = LockGuard;

        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if (*pubsub_ptr).is_some() {
            // 如果已经初始化，直接返回成功
            return Ok(());
        }

        let mut pubsub = PubSub::new(config)?;
        pubsub.init()?;

        // Register all predefined topics
        register_predefined_topics(&mut pubsub)?;

        *pubsub_ptr = Some(pubsub);

        Ok(())
    }
}

/// Register all predefined topics
fn register_predefined_topics(pubsub: &mut PubSub) -> Result<()> {
    // Register WAL topics
    let wal_topics = get_all_wal_topics();
    for (i, topic) in wal_topics.iter().enumerate() {
        pubsub.register_topic(topic, i as u16 + 1)?;
    }

    // Register core topics (start from 11 to avoid overlap with WAL topics)
    let core_topics = get_core_topics();
    for (i, topic) in core_topics.iter().enumerate() {
        pubsub.register_topic(topic, i as u16 + 11)?;
    }

    Ok(())
}

/// 订阅主题
pub fn subscribe(topic_id: u16, callback: PubSubCallback) -> Result<SubscriptionId> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if let Some(ref mut pubsub) = *pubsub_ptr {
            pubsub.subscribe(topic_id, callback)
        } else {
            Err(PubSubError::InitFailed)
        }
    }
}

/// 取消订阅
pub fn unsubscribe(subscription_id: SubscriptionId) -> Result<()> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if let Some(ref mut pubsub) = *pubsub_ptr {
            pubsub.unsubscribe(subscription_id)
        } else {
            Err(PubSubError::InitFailed)
        }
    }
}

/// 发布数据
pub fn publish(topic_id: u16, data: &[u8]) -> Result<()> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if let Some(ref mut pubsub) = *pubsub_ptr {
            pubsub.publish(topic_id, data)
        } else {
            Err(PubSubError::InitFailed)
        }
    }
}

/// 启动接收线程
#[cfg(feature = "posix")]
pub fn start_receiver() -> Result<()> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if let Some(ref mut pubsub) = *pubsub_ptr {
            pubsub.start_receiver()
        } else {
            Err(PubSubError::InitFailed)
        }
    }
}

/// 注册主题名称到ID的映射（全局实例）
pub fn register_topic(topic_name: &'static str, topic_id: u16) -> Result<()> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        if let Some(ref mut pubsub) = *pubsub_ptr {
            pubsub.register_topic(topic_name, topic_id)
        } else {
            Err(PubSubError::InitFailed)
        }
    }
}

/// 根据主题名称获取ID（全局实例）
pub fn get_topic_id(topic_name: &str) -> Option<u16> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of!(PUB_SUB_INSTANCE);
        if let Some(ref pubsub) = *pubsub_ptr {
            pubsub.get_topic_id(topic_name)
        } else {
            None
        }
    }
}

/// 根据ID获取主题名称（全局实例）
pub fn get_topic_name(topic_id: u16) -> Option<&'static str> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of!(PUB_SUB_INSTANCE);
        if let Some(ref pubsub) = *pubsub_ptr {
            pubsub.get_topic_name(topic_id)
        } else {
            None
        }
    }
}

/// 获取全局pubsub实例（用于内部使用）
pub(crate) fn get_global_pubsub() -> Option<&'static mut PubSub> {
    unsafe {
        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        (*pubsub_ptr).as_mut()
    }
}

/// 停止发布/订阅系统
pub fn shutdown() -> Result<()> {
    unsafe {
        // 使用原子交换实现自旋锁，确保线程安全关闭
        while PUB_SUB_INIT_LOCK.swap(true, Ordering::Acquire) {
            // 自旋等待锁释放
            core::hint::spin_loop();
        }

        // 确保锁在函数结束时释放
        struct LockGuard;
        impl Drop for LockGuard {
            fn drop(&mut self) {
                PUB_SUB_INIT_LOCK.store(false, Ordering::Release);
            }
        }
        let _guard = LockGuard;

        let pubsub_ptr = core::ptr::addr_of_mut!(PUB_SUB_INSTANCE);
        // Only shutdown if we have an instance
        if (*pubsub_ptr).is_some() {
            // First get a mutable reference to the instance
            let pubsub = (*pubsub_ptr).as_mut().unwrap();
            // Call shutdown on the instance
            let result = pubsub.shutdown();
            // Always clear the instance after shutdown, regardless of result
            // This prevents the instance from being used again after shutdown
            *pubsub_ptr = None;
            result
        } else {
            // If instance is already None, return Ok to avoid errors in tests
            Ok(())
        }
    }
}
