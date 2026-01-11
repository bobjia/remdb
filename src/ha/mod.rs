// 嵌入式高可用主从复制模块

pub mod manager;
pub mod replication;
pub mod heartbeat;
pub mod role;

use core::fmt;
use alloc::vec::Vec;

// HA角色
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum HARole {
    /// 主节点
    Master,
    /// 从节点
    Slave,
    /// 自动模式（通过集群协商确定角色）
    Auto,
}

// 复制模式
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ReplicationMode {
    /// 同步模式：等待至少一个从节点确认后才返回
    Sync,
    /// 异步模式：立即返回，异步复制
    Async,
}

/// HA配置结构体
#[derive(Copy, Clone, Debug)]
pub struct HAConfig {
    /// 节点ID，唯一标识集群中的节点
    pub node_id: u32,
    /// HA角色
    pub ha_role: HARole,
    /// 复制模式
    pub replication_mode: ReplicationMode,
    /// 心跳间隔（毫秒）
    pub heartbeat_interval_ms: u64,
    /// 故障检测时间（毫秒）
    pub failure_detection_ms: u64,
    /// 同步超时时间（毫秒）
    pub sync_timeout_ms: u64,
    /// 主节点地址（从节点使用）
    pub master_address: Option<&'static str>,
    /// 主节点端口（从节点使用）
    pub master_port: Option<u16>,
    /// 复制端口（用于WAL日志复制和数据同步）
    pub replication_port: u16,
    /// 心跳端口（用于节点间心跳检测）
    pub heartbeat_port: u16,
}

// HA相关错误类型
#[derive(Debug, PartialEq, Eq)]
pub enum HAError {
    // 初始化错误
    InitFailed,
    // 网络错误
    NetworkError,
    // 无效参数
    InvalidParameter,
    // 角色冲突
    RoleConflict,
    // 同步失败
    SyncFailed,
    // 心跳超时
    HeartbeatTimeout,
    // 复制错误
    ReplicationError,
    // 不支持的操作
    UnsupportedOperation,
}

impl fmt::Display for HAError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HAError::InitFailed => write!(f, "HA initialization failed"),
            HAError::NetworkError => write!(f, "Network error"),
            HAError::InvalidParameter => write!(f, "Invalid parameter"),
            HAError::RoleConflict => write!(f, "Role conflict"),
            HAError::SyncFailed => write!(f, "Sync failed"),
            HAError::HeartbeatTimeout => write!(f, "Heartbeat timeout"),
            HAError::ReplicationError => write!(f, "Replication error"),
            HAError::UnsupportedOperation => write!(f, "Unsupported operation"),
        }
    }
}

// HA结果类型
pub type Result<T> = core::result::Result<T, HAError>;

// 全局HA管理器实例
static mut HA_MANAGER: Option<manager::HAManager> = None;

/// 初始化全局HA管理器
pub fn init(config: &'static crate::config::DbConfig) -> Result<()> {
    unsafe {
        if HA_MANAGER.is_some() {
            return Err(HAError::InitFailed);
        }
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: ha::init: Creating HAManager instance", file!(), line!());
        let mut ha_manager = manager::HAManager::new(config)?;
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: ha::init: Calling ha_manager.init()", file!(), line!());
        ha_manager.init()?;
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: ha::init: Storing HAManager to global static", file!(), line!());
        HA_MANAGER = Some(ha_manager);
        
        #[cfg(feature = "std")]
        println!("[DEBUG] {}:{}: ha::init: HA initialization completed", file!(), line!());
        
        Ok(())
    }
}

/// 获取全局HA管理器
pub fn get_ha_manager() -> Option<&'static mut manager::HAManager> {
    unsafe {
        HA_MANAGER.as_mut()
    }
}

/// 关闭全局HA管理器
pub fn shutdown() -> Result<()> {
    unsafe {
        if let Some(ref mut manager) = HA_MANAGER {
            manager.shutdown()?;
            HA_MANAGER = None;
        }
        Ok(())
    }
}
