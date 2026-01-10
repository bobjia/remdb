// 心跳监视器实现

use crate::ha::{Result, HAError};
use crate::pubsub;
use crate::pubsub::{PubSubConfig, UdpMode, PubSubError};
use crate::ha::HARole;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::time::Duration;

// 心跳主题ID
const HEARTBEAT_TOPIC: u16 = 3;

/// 心跳数据包结构
#[repr(C, packed)]
pub struct HeartbeatPacket {
    /// 节点ID
    node_id: u64,
    /// 时间戳（毫秒）
    timestamp: u64,
    /// 节点角色
    role: u8,
    /// CRC校验值
    crc32: u32,
}

// 注意：移除了静态指针，改用其他方式处理回调

/// 心跳监视器
pub struct HeartbeatMonitor {
    /// 节点ID
    node_id: u64,
    /// 节点角色
    role: HARole,
    /// 心跳间隔（毫秒）
    heartbeat_interval: u64,
    /// 故障检测时间（毫秒）
    failure_detection_time: u64,
    /// 最后收到心跳的时间
    last_heartbeat_time: AtomicU64,
    /// 主节点是否存活
    master_alive: AtomicBool,
    /// 是否初始化
    is_initialized: bool,
    /// 接收循环是否运行
    receiver_running: AtomicBool,
    /// 发送循环是否运行
    sender_running: AtomicBool,
}

impl HeartbeatPacket {
    /// 创建新的心跳数据包
    pub fn new(node_id: u64, role: HARole) -> Self {
        let timestamp = crate::platform::get_timestamp_us() / 1000; // 转换为毫秒
        let role_u8 = match role {
            HARole::Master => 1,
            HARole::Slave => 2,
            HARole::Auto => 3,
        };
        
        let mut packet = Self {
            node_id,
            timestamp,
            role: role_u8,
            crc32: 0,
        };
        
        // 计算CRC校验值
        packet.update_crc();
        
        packet
    }
    
    /// 更新CRC校验值
    pub fn update_crc(&mut self) {
        // 先将CRC字段置为0
        self.crc32 = 0;
        
        // 计算CRC值
        let bytes = unsafe { 
            core::slice::from_raw_parts(
                self as *const _ as *const u8,
                core::mem::size_of::<Self>()
            ) 
        };
        
        self.crc32 = crate::pubsub::crc32::calculate_crc32(bytes);
    }
    
    /// 验证CRC校验值
    pub fn verify_crc(&self) -> bool {
        // 先创建一个临时数据包，将CRC字段置为0
        let mut temp_packet = Self {
            node_id: self.node_id,
            timestamp: self.timestamp,
            role: self.role,
            crc32: 0,
        };
        
        // 计算临时数据包的CRC值
        temp_packet.update_crc();
        
        // 比较计算出的CRC值与原始数据包的CRC值
        temp_packet.crc32 == self.crc32
    }
    
    /// 转换为字节数组
    pub fn to_bytes(&self) -> &[u8] {
        unsafe { 
            core::slice::from_raw_parts(
                self as *const _ as *const u8,
                core::mem::size_of::<Self>()
            ) 
        }
    }
    
    /// 从字节数组解析
    pub fn from_bytes(bytes: &[u8]) -> Option<&Self> {
        if bytes.len() != core::mem::size_of::<Self>() {
            return None;
        }
        
        unsafe {
            Some(&*(bytes.as_ptr() as *const Self))
        }
    }
    
    /// 获取节点ID
    pub fn node_id(&self) -> u64 {
        self.node_id
    }
    
    /// 获取时间戳
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    
    /// 获取节点角色
    pub fn role(&self) -> HARole {
        match self.role {
            1 => HARole::Master,
            2 => HARole::Slave,
            3 => HARole::Auto,
            _ => HARole::Auto,
        }
    }
}

impl HeartbeatMonitor {
    /// 创建新的心跳监视器
    pub fn new(heartbeat_interval: u64, failure_detection_time: u64) -> Result<Self> {
        Ok(Self {
            node_id: 0, // 默认为0，后续可以设置
            role: HARole::Auto,
            heartbeat_interval,
            failure_detection_time,
            last_heartbeat_time: AtomicU64::new(crate::platform::get_timestamp_us() / 1000),
            master_alive: AtomicBool::new(true),
            is_initialized: false,
            receiver_running: AtomicBool::new(false),
            sender_running: AtomicBool::new(false),
        })
    }
    
    /// 设置节点ID
    pub fn set_node_id(&mut self, node_id: u64) {
        self.node_id = node_id;
    }
    
    /// 设置节点角色
    pub fn set_role(&mut self, role: HARole) {
        self.role = role;
    }
    
    /// 初始化心跳监视器
    pub fn init(&self) -> Result<()> {
        // 初始化pubsub系统（如果尚未初始化）
        self.init_pubsub()?;
        
        Ok(())
    }
    
    /// 初始化主节点
    pub fn init_master(&self) -> Result<()> {
        // 主节点：定期发送心跳
        self.start_heartbeat_sender()?;
        
        Ok(())
    }
    
    /// 初始化从节点
    pub fn init_slave(&self) -> Result<()> {
        // 从节点：接收主节点心跳
        self.start_heartbeat_receiver()?;
        
        // 启动心跳检查
        self.start_heartbeat_check()?;
        
        Ok(())
    }
    
    /// 初始化pubsub系统
    fn init_pubsub(&self) -> Result<()> {
        // pubsub系统可能已经由ReplicationManager初始化
        // 这里尝试初始化，如果失败则忽略
        let pubsub_config = PubSubConfig {
            udp_mode: UdpMode::Unicast,
            multicast_addr: None,
            port: 5557, // 使用专门的心跳端口
            max_topics: 4,
            max_subscribers_per_topic: 8,
            buffer_size: 4096,
            enable_nack: true,
            retransmit_timeout: core::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: core::time::Duration::from_secs(10),
            frame_pool_size: 128,
        };
        
        // 尝试初始化pubsub，如果失败则忽略（测试环境可能没有网络）
        let _ = pubsub::init(pubsub_config);
        
        Ok(())
    }
    
    /// 启动心跳发送器
    fn start_heartbeat_sender(&self) -> Result<()> {
        // 检查是否已经在运行
        if self.sender_running.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        // 标记为运行中
        self.sender_running.store(true, Ordering::Relaxed);
        
        // 注意：暂时简化设计，移除线程相关逻辑
        // 实际应用中，心跳发送应该由外部定时器或主循环定期调用
        
        Ok(())
    }
    
    /// 启动心跳接收器
    fn start_heartbeat_receiver(&self) -> Result<()> {
        // 检查是否已经在运行
        if self.receiver_running.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        // 标记为运行中
        self.receiver_running.store(true, Ordering::Relaxed);
        
        // 注意：暂时简化设计，不使用pubsub订阅
        // 实际应用中需要实现心跳接收逻辑
        
        Ok(())
    }
    
    /// 启动心跳检查
    fn start_heartbeat_check(&self) -> Result<()> {
        // 注意：暂时简化设计，移除线程相关逻辑
        // 实际应用中，心跳检查应该由外部定时器或主循环定期调用
        
        Ok(())
    }
    
    /// 处理接收到的心跳
    fn handle_heartbeat(&self, data: &[u8]) {
        // 解析心跳数据包
        if let Some(packet) = HeartbeatPacket::from_bytes(data) {
            // 验证CRC校验值
            if !packet.verify_crc() {
                return;
            }
            
            // 更新最后心跳时间
            let now = crate::platform::get_timestamp_us() / 1000; // 转换为毫秒
            self.last_heartbeat_time.store(now, Ordering::Relaxed);
            self.master_alive.store(true, Ordering::Relaxed);
        }
    }
    
    /// 检查心跳状态
    pub fn check_status(&self) -> Result<()> {
        // 只有从节点需要检查心跳
        if self.role != HARole::Slave {
            return Ok(());
        }
        
        // 检查最后心跳时间
        let now = crate::platform::get_timestamp_us() / 1000; // 转换为毫秒
        let last_heartbeat = self.last_heartbeat_time.load(Ordering::Relaxed);
        
        // 如果超过故障检测时间，触发故障转移
        if now - last_heartbeat > self.failure_detection_time {
            self.master_alive.store(false, Ordering::Relaxed);
            
            // 触发故障转移
            // 注意：这里需要调用角色管理器进行角色切换
            // 由于当前设计中HeartbeatMonitor没有引用RoleManager，
            // 这里只设置状态，由HA管理器定期检查
            return Err(HAError::HeartbeatTimeout);
        }
        
        Ok(())
    }
    
    /// 发送心跳
    fn send_heartbeat(&self) -> Result<()> {
        // 构建心跳数据包
        let packet = HeartbeatPacket::new(self.node_id, self.role);
        
        // 发布心跳消息
        match pubsub::publish(HEARTBEAT_TOPIC, packet.to_bytes()) {
            Ok(_) => Ok(()),
            Err(_) => Err(HAError::NetworkError),
        }
    }
    
    /// 关闭心跳监视器
    pub fn shutdown(&self) -> Result<()> {
        // 停止发送器
        self.sender_running.store(false, Ordering::Relaxed);
        
        // 停止接收器
        self.receiver_running.store(false, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// 检查主节点是否存活
    pub fn is_master_alive(&self) -> bool {
        self.master_alive.load(Ordering::Relaxed)
    }
    
    /// 获取最后心跳时间
    pub fn get_last_heartbeat_time(&self) -> u64 {
        self.last_heartbeat_time.load(Ordering::Relaxed)
    }
}

