// 角色管理器实现

use crate::config::HARole;
use crate::ha::{Result, HAError};
use crate::pubsub;
use crate::pubsub::{PubSubConfig, UdpMode, PubSubError};
use core::sync::atomic::{AtomicU8, Ordering};

// 角色变更主题ID
const ROLE_CHANGE_TOPIC: u16 = 4;

/// 角色管理器
pub struct RoleManager {
    /// 当前角色（原子操作，确保线程安全）
    current_role: AtomicU8,
    /// 自旋锁
    lock: u32,
    /// 是否初始化
    is_initialized: bool,
}

impl RoleManager {
    /// 创建新的角色管理器
    pub fn new(initial_role: HARole) -> Result<Self> {
        Ok(Self {
            current_role: AtomicU8::new(initial_role as u8),
            lock: 0,
            is_initialized: false,
        })
    }
    
    /// 初始化角色管理器
    pub fn init(&self) -> Result<()> {
        // 初始化pubsub系统（如果尚未初始化）
        self.init_pubsub()?;
        
        Ok(())
    }
    
    /// 初始化pubsub系统
    fn init_pubsub(&self) -> Result<()> {
        // pubsub系统可能已经由其他组件初始化
        // 这里尝试初始化，如果失败则忽略
        let pubsub_config = PubSubConfig {
            udp_mode: UdpMode::Unicast,
            multicast_addr: None,
            port: 5558, // 使用专门的角色变更端口
            max_topics: 2,
            max_subscribers_per_topic: 8,
            buffer_size: 2048,
            enable_nack: true,
            retransmit_timeout: core::time::Duration::from_millis(100),
            max_retransmits: 3,
            heartbeat_interval: core::time::Duration::from_secs(10),
            frame_pool_size: 64,
        };
        
        match pubsub::init(pubsub_config) {
            Ok(_) => Ok(()),
            Err(PubSubError::InitFailed) => Ok(()), // 已初始化
            Err(_) => Err(HAError::NetworkError),
        }
    }
    
    /// 获取当前角色
    pub fn get_role(&self) -> HARole {
        match self.current_role.load(Ordering::Relaxed) {
            0 => HARole::Master,
            1 => HARole::Slave,
            2 => HARole::Auto,
            _ => HARole::Auto, // 默认值
        }
    }
    
    /// 设置角色
    pub fn set_role(&self, role: HARole) -> Result<()> {
        // 检查角色是否变化
        let current_role = self.get_role();
        if current_role == role {
            return Ok(());
        }
        
        // 更新角色
        self.current_role.store(role as u8, Ordering::Relaxed);
        
        // 发布角色变更通知
        self.publish_role_change(role)?;
        
        Ok(())
    }
    
    /// 发布角色变更通知
    fn publish_role_change(&self, role: HARole) -> Result<()> {
        // 构建角色变更数据
        let role_data = [role as u8; 1];
        
        // 发布角色变更消息
        match pubsub::publish(ROLE_CHANGE_TOPIC, &role_data) {
            Ok(_) => Ok(()),
            Err(_) => Err(HAError::NetworkError),
        }
    }
    
    /// 订阅角色变更通知
    pub fn subscribe_role_change(&self, _callback: fn(role: HARole) -> bool) -> Result<()> {
        // TODO: 实现角色变更订阅
        // 注意：由于pubsub::subscribe只接受函数指针，不接受闭包，
        // 这里需要实现静态回调函数，或者重新设计pubsub接口
        Ok(())
    }
    
    /// 关闭角色管理器
    pub fn shutdown(&self) -> Result<()> {
        // 关闭相关资源
        Ok(())
    }
    
    /// 检查角色是否为主节点
    pub fn is_master(&self) -> bool {
        self.get_role() == HARole::Master
    }
    
    /// 检查角色是否为从节点
    pub fn is_slave(&self) -> bool {
        self.get_role() == HARole::Slave
    }
    
    /// 检查角色是否为自动模式
    pub fn is_auto(&self) -> bool {
        self.get_role() == HARole::Auto
    }
}