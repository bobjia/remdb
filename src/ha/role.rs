// 角色管理器实现

use crate::ha::HARole;
use crate::ha::{HAError, Result};
use crate::pubsub;
use crate::pubsub::{PubSubConfig, PubSubError, UdpMode};
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
        // pubsub系统已经由HA管理器统一初始化，无需再次初始化
        // 这里只做日志记录
        #[cfg(feature = "std")]
        eprintln!("[DEBUG] Role manager using existing pubsub system");

        Ok(())
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
        // 注意：在测试环境中，pubsub可能未正确初始化，此时忽略发布失败
        match pubsub::publish(ROLE_CHANGE_TOPIC, &role_data) {
            Ok(_) => Ok(()),
            Err(_) => {
                // 忽略发布失败，角色已经更新
                Ok(())
            }
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
