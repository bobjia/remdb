// 心跳监视器实现

use crate::ha::{Result, HAError};
use crate::pubsub;
use crate::pubsub::{PubSubConfig, UdpMode, PubSubError};
use crate::ha::HARole;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::time::Duration;
use core::ptr::NonNull;

// 注意：移除全局静态变量，避免线程安全问题
// 改为使用其他方式处理心跳发送

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

/// 心跳接收回调函数 - 简化实现，不依赖全局静态变量
fn handle_heartbeat_callback(topic_id: u16, data: &[u8]) -> bool {
    if topic_id != HEARTBEAT_TOPIC {
        return false;
    }
    
    #[cfg(feature = "std")]
    println!("[DEBUG] {}:{}: Heartbeat callback received data, len: {}", file!(), line!(), data.len());
    
    // 解析并处理心跳数据包
    if let Some(packet) = HeartbeatPacket::from_bytes(data) {
        // 验证CRC校验值
        if !packet.verify_crc() {
            #[cfg(feature = "std")]
            println!("[DEBUG] {}:{}: Heartbeat CRC check failed", file!(), line!());
            return true;
        }
        
        // 安全访问字段，避免未对齐访问
        let node_id = packet.node_id();
        let role = packet.role();
        let timestamp = packet.timestamp();
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Received heartbeat, node_id: {}, role: {:?}, timestamp: {}", 
                 file!(), line!(), node_id, role, timestamp);
    }
    
    true
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
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Heartbeat monitor initialized, role: {:?}, node_id: {}", 
                 file!(), line!(), self.role, self.node_id);
        
        // 初始化pubsub系统
        self.init_pubsub()?;
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Heartbeat monitor pubsub initialized", file!(), line!());
        
        Ok(())
    }
    
    /// 初始化主节点
    pub fn init_master(&self) -> Result<()> {
        // 主节点：定期发送心跳
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Initializing master node, starting heartbeat sender", file!(), line!());
        
        self.start_heartbeat_sender()?;
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Master node heartbeat sender started", file!(), line!());
        
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
        // pubsub系统已经由HA管理器统一初始化，无需再次初始化
        // 这里只做日志记录
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Heartbeat using existing pubsub system", file!(), line!());
        
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
        
        // 获取需要的值
        let heartbeat_interval = self.heartbeat_interval;
        let node_id = self.node_id;
        let role = self.role;
        
        // 启动后台线程定期发送心跳
        #[cfg(feature = "std")]
        {
            std::thread::spawn(move || {
                let interval = std::time::Duration::from_millis(heartbeat_interval);
                
                loop {
                    // 构建心跳数据包
                    let packet = HeartbeatPacket::new(node_id, role);
                    
                    // 复制数据到临时缓冲区，确保publish函数使用完数据之前不会释放内存
                    let mut buffer = [0u8; core::mem::size_of::<HeartbeatPacket>()];
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            packet.to_bytes().as_ptr(),
                            buffer.as_mut_ptr(),
                            buffer.len()
                        );
                    }
                    
                    // 发布心跳消息
                    match pubsub::publish(HEARTBEAT_TOPIC, &buffer) {
                        Ok(_) => {
                            // 发送成功，记录日志
                            let timestamp = packet.timestamp();
                            println!("[DEBUG] src/ha/heartbeat.rs: Heartbeat sent, node_id: {}, role: {:?}, timestamp: {}", 
                                     node_id, role, timestamp);
                        },
                        Err(e) => {
                            // 发送失败，记录错误但继续运行
                            println!("[Heartbeat] Failed to send heartbeat: {:?}", e);
                        },
                    }
                    
                    // 等待心跳间隔
                    std::thread::sleep(interval);
                }
            });
        }
        
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
        
        // 订阅心跳主题
        match pubsub::subscribe(HEARTBEAT_TOPIC, handle_heartbeat_callback) {
            Ok(_) => {
                #[cfg(feature = "std")]
                println!("[DEBUG] {}:{}: Successfully subscribed to heartbeat topic", file!(), line!());
            },
            Err(e) => {
                #[cfg(feature = "std")]
                println!("[DEBUG] {}:{}: Failed to subscribe to heartbeat topic: {:?}", file!(), line!(), e);
                return Err(HAError::NetworkError);
            }
        }
        
        Ok(())
    }
    
    /// 启动心跳检查
    fn start_heartbeat_check(&self) -> Result<()> {
        // 注意：简化设计，移除线程相关逻辑
        // 实际应用中，心跳检查应该由外部定时器或主循环定期调用
        
        Ok(())
    }
    
    /// 处理接收到的心跳
    fn handle_heartbeat(&self, data: &[u8]) {
        // 解析心跳数据包
        if let Some(packet) = HeartbeatPacket::from_bytes(data) {
            // 验证CRC校验值
            if !packet.verify_crc() {
                #[cfg(feature = "std")]
                println!("[DEBUG] {}:{}: Heartbeat CRC check failed", file!(), line!());
                return;
            }
            
            // 安全访问字段，避免未对齐访问
            let node_id = packet.node_id();
            let role = packet.role();
            let timestamp = packet.timestamp();
            
            #[cfg(feature = "std")]
            println!("[DEBUG] {}:{}: Received heartbeat, node_id: {}, role: {:?}, timestamp: {}", 
                     file!(), line!(), node_id, role, timestamp);
            
            // 更新最后心跳时间
            let now = crate::platform::get_timestamp_us() / 1000; // 转换为毫秒
            self.last_heartbeat_time.store(now, Ordering::Relaxed);
            self.master_alive.store(true, Ordering::Relaxed);
            
            #[cfg(feature = "std")]
            println!("[DEBUG] {}:{}: Updated master alive status to true", file!(), line!());
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
        let mut packet = HeartbeatPacket::new(self.node_id, self.role);
        
        // 安全访问字段，避免未对齐访问
        let timestamp = packet.timestamp();
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: Sending heartbeat, node_id: {}, role: {:?}, timestamp: {}", 
                 file!(), line!(), self.node_id, self.role, timestamp);
        
        // 复制数据到临时缓冲区，确保publish函数使用完数据之前不会释放内存
        let mut buffer = [0u8; core::mem::size_of::<HeartbeatPacket>()];
        unsafe {
            core::ptr::copy_nonoverlapping(
                packet.to_bytes().as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len()
            );
        }
        
        // 发布心跳消息
        match pubsub::publish(HEARTBEAT_TOPIC, &buffer) {
            Ok(_) => {
                #[cfg(feature = "std")]
                println!("[DEBUG] {}:{}: Heartbeat sent successfully", file!(), line!());
                Ok(())
            },
            Err(e) => {
                #[cfg(feature = "std")]
                println!("[DEBUG] {}:{}: Failed to send heartbeat: {:?}", file!(), line!(), e);
                Err(HAError::NetworkError)
            },
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

